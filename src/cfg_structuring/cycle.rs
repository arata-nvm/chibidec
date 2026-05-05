use std::collections::HashSet;

use petgraph::{
    Direction,
    graph::{EdgeIndex, NodeIndex},
    visit::{Dfs, EdgeRef, NodeFiltered, Reversed, Walker},
};

use crate::{
    cfg_recovery::cfg::{Condition, EdgeLabel, TailKind},
    cfg_structuring::region::{Region, RegionCfg, RegionDominanceView, RegionId},
    graph::{IndexedGraphView, IndexedGraphViewMut},
};

pub fn contract_cyclic_region(
    cfg: &mut RegionCfg,
    structured_loop: &StructuredLoop,
    body: RegionId,
) -> RegionId {
    contract_loop(cfg, structured_loop, body)
}

fn contract_loop(
    cfg: &mut RegionCfg,
    structured_loop: &StructuredLoop,
    body_region: RegionId,
) -> RegionId {
    let loop_ = Region::Loop {
        kind: structured_loop.loop_exit.kind(),
        meta: structured_loop.clone(),
        body: body_region,
    };
    let (loop_region, loop_node) = cfg.add_region(loop_);

    let body_node = cfg
        .node_for_key(body_region)
        .expect("loop body region should have node");
    cfg.redirect_edges(body_node, loop_node, Direction::Incoming);

    if let Some((exit_node, edge_label)) = structured_loop.loop_exit.exit_target_and_label() {
        while let Some(edge) = cfg.graph().find_edge(body_node, exit_node) {
            cfg.graph_mut().remove_edge(edge);
        }
        if cfg.graph().find_edge(loop_node, exit_node).is_none() {
            cfg.graph_mut().add_edge(loop_node, exit_node, edge_label);
        }
    }

    cfg.remove_node_by_index(body_node)
        .expect("loop body node should exist");

    loop_region
}

pub fn find_loop(
    cfg: &mut RegionCfg,
    raw_loop: &mut RawLoop,
) -> (StructuredLoop, Vec<(RegionId, RegionId, EdgeLabel)>) {
    let (single_entry, mut tail_edges) = ensure_single_entry(cfg, raw_loop);
    let (loop_exit, body) = {
        let dominance = cfg
            .compute_dominance()
            .expect("failed to compute region dominance");
        let loop_exit =
            select_single_exit(&dominance, raw_loop).expect("at least one exit should be found");
        let body = match loop_exit.exit_node() {
            Some(exit_node) => build_loop_body(&dominance, single_entry, exit_node),
            None => raw_loop
                .nodes
                .iter()
                .map(|node| cfg.key_for_node(*node).expect("node should have region"))
                .collect(),
        };
        (loop_exit, body)
    };

    let virtualized_tail_edges = virtualize_loop_tails(cfg, raw_loop, &loop_exit, &body);
    tail_edges.extend(virtualized_tail_edges);
    let structured_loop =
        StructuredLoop::build_from_raw_loop(cfg, raw_loop, loop_exit, single_entry, body);
    (structured_loop, tail_edges)
}

pub fn find_smallest_loop(
    dominance: &RegionDominanceView<'_>,
    target: NodeIndex,
    index: usize,
) -> RawLoop {
    let cfg = dominance.cfg();
    let mut loops = Vec::new();
    for head in dominance.backedge_targets(target) {
        let loop_nodes = if target == head {
            HashSet::from([target])
        } else {
            let reversed = Reversed(cfg.graph());
            let without_head = NodeFiltered::from_fn(reversed, |node| node != head);
            let mut loop_nodes: HashSet<_> = Dfs::new(&without_head, target)
                .iter(&without_head)
                .collect();
            loop_nodes.insert(head);
            loop_nodes
        };
        loops.push(RawLoop::new(cfg, index, loop_nodes, head));
    }
    loops
        .into_iter()
        .min_by_key(|loop_| loop_.nodes.len())
        .expect("at least one loop should be found")
}

pub struct RawLoop {
    pub loop_index: usize,
    pub nodes: HashSet<NodeIndex>,
    pub head: NodeIndex,
    pub entries: HashSet<NodeIndex>,
    pub exit_edges: HashSet<EdgeIndex>,
}

impl RawLoop {
    pub fn new(
        cfg: &RegionCfg,
        loop_index: usize,
        nodes: HashSet<NodeIndex>,
        head: NodeIndex,
    ) -> Self {
        let entries = extract_entry_nodes(cfg, &nodes);
        let exit_edges = extract_exit_edges(cfg, &nodes);
        Self {
            loop_index,
            nodes,
            head,
            entries,
            exit_edges,
        }
    }

    // 最もループ外からの入辺が多いノードをエントリとみなす
    pub fn most_likely_entry(&self, cfg: &RegionCfg) -> NodeIndex {
        return *self
            .entries
            .iter()
            .max_by_key(|node| count_edges_from_outside(cfg, **node, &self.nodes))
            .expect("loop should have at least one entry");

        fn count_edges_from_outside(
            cfg: &RegionCfg,
            node: NodeIndex,
            loop_nodes: &HashSet<NodeIndex>,
        ) -> usize {
            cfg.graph()
                .edges_directed(node, Direction::Incoming)
                .filter(|edge| !loop_nodes.contains(&edge.source()))
                .count()
        }
    }

    // 仮想化すべき辺を返す
    pub fn edges_to_virtualize(&self, cfg: &RegionCfg, entry: NodeIndex) -> Vec<EdgeIndex> {
        self.entries
            .iter()
            .filter(|node| **node != entry)
            .flat_map(|node| cfg.graph().edges_directed(*node, Direction::Incoming))
            .filter(|edge| !self.nodes.contains(&edge.source()))
            .map(|edge| edge.id())
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct StructuredLoop {
    pub index: usize,
    pub loop_exit: LoopExit,
    pub head: RegionId,
    pub entry: RegionId,
    pub body: HashSet<RegionId>,
    pub cond: Option<Condition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopKind {
    While,
    DoWhile,
    NatLoop,
}

impl StructuredLoop {
    pub fn new(
        index: usize,
        loop_exit: LoopExit,
        head: RegionId,
        entry: RegionId,
        body: HashSet<RegionId>,
        cond: Option<Condition>,
    ) -> Self {
        Self {
            index,
            loop_exit,
            head,
            entry,
            body,
            cond,
        }
    }

    pub fn build_from_raw_loop(
        cfg: &RegionCfg,
        raw_loop: &RawLoop,
        loop_exit: LoopExit,
        entry: NodeIndex,
        body: HashSet<RegionId>,
    ) -> Self {
        let head_region = cfg
            .key_for_node(raw_loop.head)
            .expect("loop head should have region id");
        let entry_region = cfg
            .key_for_node(entry)
            .expect("loop entry should have region id");
        let cond = match loop_exit.kind() {
            LoopKind::DoWhile => loop_exit
                .exit_edge()
                .and_then(|edge| cfg.graph().edge_weight(edge))
                .and_then(|label| label.effective_condition())
                .map(|cond| cond.negate()),
            LoopKind::While => loop_exit
                .exit_edge()
                .and_then(|edge| cfg.graph().edge_weight(edge))
                .and_then(|label| label.effective_condition())
                .map(|cond| cond.negate()),
            LoopKind::NatLoop => {
                extract_condition(cfg, raw_loop.head, entry, loop_exit.exit_node())
            }
        };
        Self::new(
            raw_loop.loop_index,
            loop_exit,
            head_region,
            entry_region,
            body,
            cond,
        )
    }
}

// nodes以外のノードからの入辺があるノードを抽出
fn extract_entry_nodes(cfg: &RegionCfg, nodes: &HashSet<NodeIndex>) -> HashSet<NodeIndex> {
    nodes
        .iter()
        .copied()
        .filter(|node| {
            cfg.graph()
                .neighbors_directed(*node, Direction::Incoming)
                .any(|pred| !nodes.contains(&pred))
        })
        .collect()
}

// nodes以外のノードへの出辺を抽出
fn extract_exit_edges(cfg: &RegionCfg, nodes: &HashSet<NodeIndex>) -> HashSet<EdgeIndex> {
    nodes
        .iter()
        .copied()
        .flat_map(|node| cfg.graph().edges_directed(node, Direction::Outgoing))
        .filter(|edge| !nodes.contains(&edge.target()))
        .map(|edge| edge.id())
        .collect()
}

fn extract_condition(
    cfg: &RegionCfg,
    head: NodeIndex,
    entry: NodeIndex,
    succ: Option<NodeIndex>,
) -> Option<Condition> {
    if let Some(label) = cfg.edge_label(head, entry) {
        label.effective_condition()
    } else if let Some(label) = cfg.edge_label(head, succ?) {
        label.effective_condition()
    } else {
        None
    }
}

fn ensure_single_entry(
    cfg: &mut RegionCfg,
    raw_loop: &mut RawLoop,
) -> (NodeIndex, Vec<(RegionId, RegionId, EdgeLabel)>) {
    let entry = raw_loop.most_likely_entry(cfg);
    let mut virtualized_edges = Vec::new();
    for edge in raw_loop.edges_to_virtualize(cfg, entry) {
        let (source, target) = cfg.graph().edge_endpoints(edge).expect("edge should exist");
        cfg.graph_mut()
            .remove_edge(edge)
            .expect("edge should be removable");
        let source_region = cfg
            .key_for_node(source)
            .expect("source node should have region id");
        let target_region = cfg
            .key_for_node(target)
            .expect("target node should have region id");
        let new_label = EdgeLabel::Virtualized(TailKind::Goto {
            target: cfg
                .key_for_node(target)
                .expect("target node should have region id"),
        });
        virtualized_edges.push((source_region, target_region, new_label));
    }
    (entry, virtualized_edges)
}

#[derive(Clone, Debug)]
pub struct LoopExit {
    kind: LoopKind,
    exit_node: Option<NodeIndex>,
    exit_edge: Option<EdgeIndex>,
    exit_label: Option<EdgeLabel>,
}

impl LoopExit {
    pub fn kind(&self) -> LoopKind {
        self.kind
    }

    pub fn exit_node(&self) -> Option<NodeIndex> {
        self.exit_node
    }

    pub fn exit_edge(&self) -> Option<EdgeIndex> {
        self.exit_edge
    }

    pub fn exit(&self) -> Option<(NodeIndex, EdgeIndex)> {
        self.exit_node.zip(self.exit_edge)
    }

    pub fn exit_target_and_label(&self) -> Option<(NodeIndex, EdgeLabel)> {
        self.exit_node.zip(self.exit_label.clone())
    }
}

fn select_single_exit(dominance: &RegionDominanceView<'_>, raw_loop: &RawLoop) -> Option<LoopExit> {
    let cfg = dominance.cfg();
    let preferred_succ = dominance.immediate_post_dominator(raw_loop.head);
    let exit_edges = raw_loop.exit_edges.clone();

    // while: headからの出辺がある
    let while_edges: HashSet<_> = raw_loop
        .exit_edges
        .iter()
        .copied()
        .filter(|edge| {
            cfg.graph()
                .edge_endpoints(*edge)
                .is_some_and(|(source, _)| source == raw_loop.head)
        })
        .collect();
    if !while_edges.is_empty() {
        let edge = pick_edge_deterministically(cfg, &while_edges, preferred_succ);
        let (_, succ) = cfg.graph().edge_endpoints(edge).expect("edge should exist");
        return Some(LoopExit {
            kind: LoopKind::While,
            exit_node: Some(succ),
            exit_edge: Some(edge),
            exit_label: cfg.graph().edge_weight(edge).cloned(),
        });
    }

    // do-while: 戻り辺のソースからの出辺がある
    let srcs: HashSet<_> = dominance.backedge_sources(raw_loop.head).collect();
    let do_while_edges: HashSet<_> = raw_loop
        .exit_edges
        .iter()
        .copied()
        .filter(|edge| {
            cfg.graph()
                .edge_endpoints(*edge)
                .is_some_and(|(source, _)| srcs.contains(&source))
        })
        .collect();
    if !do_while_edges.is_empty() {
        let edge = pick_edge_deterministically(cfg, &do_while_edges, preferred_succ);
        let (_, succ) = cfg.graph().edge_endpoints(edge).expect("edge should exist");
        return Some(LoopExit {
            kind: LoopKind::DoWhile,
            exit_node: Some(succ),
            exit_edge: Some(edge),
            exit_label: cfg.graph().edge_weight(edge).cloned(),
        });
    }

    // nat loop
    if !exit_edges.is_empty() {
        let nat_edge = pick_edge_deterministically(cfg, &exit_edges, preferred_succ);
        let (_, succ) = cfg
            .graph()
            .edge_endpoints(nat_edge)
            .expect("edge should exist");
        return Some(LoopExit {
            kind: LoopKind::NatLoop,
            exit_node: Some(succ),
            exit_edge: Some(nat_edge),
            exit_label: cfg.graph().edge_weight(nat_edge).cloned(),
        });
    }

    return Some(LoopExit {
        kind: LoopKind::NatLoop,
        exit_node: None,
        exit_edge: None,
        exit_label: None,
    });

    fn pick_edge_deterministically(
        cfg: &RegionCfg,
        edges: &HashSet<EdgeIndex>,
        preferred_succ: Option<NodeIndex>,
    ) -> EdgeIndex {
        if let Some(succ) = preferred_succ
            && let Some(edge) = edges.iter().find(|edge| {
                cfg.graph()
                    .edge_endpoints(**edge)
                    .is_some_and(|(_, target)| target == succ)
            })
        {
            return *edge;
        }

        *edges
            .iter()
            .min()
            .expect("at least one edge should be present")
    }
}

fn build_loop_body(
    dominance: &RegionDominanceView<'_>,
    head: NodeIndex,
    succ: NodeIndex,
) -> HashSet<RegionId> {
    let cfg = dominance.cfg();
    cfg.graph()
        .node_indices()
        .filter(|&node| dominance.dominates(head, node))
        .filter(|&node| dominance.post_dominates(succ, node) && node != succ)
        .filter(|&node| node != cfg.vexit())
        .map(|node| cfg.key_for_node(node).expect("node should have region id"))
        .collect()
}

fn virtualize_loop_tails(
    cfg: &mut RegionCfg,
    raw_loop: &mut RawLoop,
    loop_exit: &LoopExit,
    body: &HashSet<RegionId>,
) -> Vec<(RegionId, RegionId, EdgeLabel)> {
    let Some((exit_node, exit_edge)) = loop_exit.exit() else {
        return vec![];
    };

    let mut virtualized_edges = Vec::new();

    let mut changed = Vec::new();
    for region in body {
        let node = cfg.node_for_key(*region).expect("region should have node");
        for edge in cfg.graph().edges(node) {
            if edge.id() == exit_edge {
                continue;
            }

            let target_region = cfg
                .key_for_node(edge.target())
                .expect("node should have reigon");
            let label = if edge.target() == raw_loop.head {
                EdgeLabel::Virtualized(TailKind::Continue)
            } else if edge.target() == exit_node {
                EdgeLabel::Virtualized(TailKind::Break)
            } else if body.contains(&target_region) {
                continue;
            } else {
                EdgeLabel::Virtualized(TailKind::Goto {
                    target: target_region,
                })
            };
            changed.push((node, edge.target(), *region, target_region, label));
        }
    }
    for (source, target, source_region, target_region, label) in changed {
        while let Some(edge) = cfg.graph().find_edge(source, target) {
            cfg.graph_mut().remove_edge(edge);
        }
        virtualized_edges.push((source_region, target_region, label))
    }

    virtualized_edges
}
