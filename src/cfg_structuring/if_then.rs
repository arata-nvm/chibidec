use std::collections::{HashSet, VecDeque};

use petgraph::{Direction, algo::dominators::Dominators, graph::NodeIndex, visit::EdgeRef};

use crate::{
    cfg_recovery::cfg::{Condition, EdgeLabel},
    cfg_structuring::region::{Region, RegionCfg, RegionId},
    graph::{IndexedGraphView, IndexedGraphViewMut},
};

pub(crate) fn match_if_then(
    cfg: &mut RegionCfg,
    dom: &Dominators<NodeIndex>,
    pdom: &Dominators<NodeIndex>,
    head: NodeIndex,
) -> bool {
    match find_if_then(cfg, dom, pdom, head) {
        Some(if_schema) => {
            contract_if_then(cfg, if_schema);
            true
        }
        None => false,
    }
}

#[derive(Debug)]
pub struct IfSchema {
    pub head: RegionId,
    pub then_body: Vec<RegionId>,
    pub else_body: Option<Vec<RegionId>>,
    pub join: RegionId,
    pub cond: Option<Condition>,
}

impl IfSchema {
    pub fn new(
        head: RegionId,
        then_body: Vec<RegionId>,
        else_body: Option<Vec<RegionId>>,
        join: RegionId,
        cond: Option<Condition>,
    ) -> Self {
        Self {
            head,
            then_body,
            else_body,
            join,
            cond,
        }
    }

    pub fn all_regions(&self) -> Vec<RegionId> {
        let mut regions = Vec::new();
        regions.push(self.head);
        regions.extend(&self.then_body);
        if let Some(else_nodes) = &self.else_body {
            regions.extend(else_nodes);
        }
        regions.push(self.join);
        regions
    }
}

fn find_if_then(
    cfg: &RegionCfg,
    dom: &Dominators<NodeIndex>,
    pdom: &Dominators<NodeIndex>,
    head: NodeIndex,
) -> Option<IfSchema> {
    let mut succs = cfg.graph().neighbors_directed(head, Direction::Outgoing);
    let succ1 = succs.next()?;
    let succ2 = succs.next()?;
    if succs.next().is_some() || succ1 == cfg.vexit() || succ2 == cfg.vexit() {
        return None;
    }

    let join = pdom.immediate_dominator(head)?;
    let then_entry = match (succ1 == join, succ2 == join) {
        (true, false) => succ2,
        (false, true) => succ1,
        _ => return None,
    };

    let then_nodes = collect_nodes_between(cfg, dom, pdom, head, then_entry, join);
    if then_nodes.is_empty() || contains_sess_violation(cfg, head, then_entry, join, &then_nodes) {
        return None;
    }

    let head_to_join = cfg
        .edge_label(head, join)
        .expect("missing edge from head to join");
    let head_to_then = cfg
        .edge_label(head, then_entry)
        .expect("missing edge from head to then_entry");
    let cond = match (head_to_join, head_to_then) {
        (EdgeLabel::TrueBranch(c1), EdgeLabel::FalseBranch(_)) => c1.clone().map(|c| c.negate()),
        (EdgeLabel::FalseBranch(c1), EdgeLabel::TrueBranch(_)) => c1.clone(),
        _ => {
            return None;
        }
    };

    Some(IfSchema::new(
        cfg.key_for_node(head)
            .expect("missing region node for head"),
        then_nodes
            .into_iter()
            .map(|node| {
                cfg.key_for_node(node)
                    .expect("missing region node for then node")
            })
            .collect(),
        None,
        cfg.key_for_node(join)
            .expect("missing region node for join"),
        cond,
    ))
}

fn contract_if_then(cfg: &mut RegionCfg, if_schema: IfSchema) -> RegionId {
    let head_node = cfg
        .node_for_key(if_schema.head)
        .expect("missing node for if_node");
    let join_node = cfg
        .node_for_key(if_schema.join)
        .expect("missing node for join_node");

    let nodes: Vec<_> = if_schema
        .all_regions()
        .into_iter()
        .map(|key| {
            cfg.node_for_key(key)
                .expect("missing node for if schema node")
        })
        .collect();

    let if_then_region = Region::IfThen {
        head: if_schema.head,
        then_br: if_schema.then_body,
        join: if_schema.join,
        cond: if_schema.cond,
    };
    let (if_then_region_id, if_then_node) = cfg.add_region(if_then_region);

    cfg.redirect_edges(head_node, if_then_node, Direction::Incoming);
    if let Some(label) = cfg.remove_edge_label(head_node, join_node) {
        cfg.graph_mut().add_edge(if_then_node, join_node, label);
    }

    for node in nodes {
        if node == join_node {
            continue;
        }
        cfg.remove_node_by_index(node)
            .expect("failed to remove node in if-then");
    }

    if_then_region_id
}

// headに支配され、かつjoinに後続支配されるノードを探索する
fn collect_nodes_between(
    cfg: &RegionCfg,
    dom: &Dominators<NodeIndex>,
    pdom: &Dominators<NodeIndex>,
    head: NodeIndex,
    then_entry: NodeIndex,
    join: NodeIndex,
) -> HashSet<NodeIndex> {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    queue.push_back(then_entry);
    while let Some(u) = queue.pop_front() {
        if visited.contains(&u) || u == join {
            continue;
        }

        let Some(mut dominators) = dom.dominators(u) else {
            continue;
        };
        if !dominators.any(|d| d == head) {
            continue;
        }

        let Some(mut post_dominators) = pdom.dominators(u) else {
            continue;
        };
        if !post_dominators.any(|d| d == join) {
            continue;
        }

        visited.insert(u);
        for v in cfg.graph().neighbors_directed(u, Direction::Outgoing) {
            queue.push_back(v);
        }
    }
    visited
}

// then_nodesがSESSに違反する辺を含むか判定する
fn contains_sess_violation(
    cfg: &RegionCfg,
    head: NodeIndex,
    then_entry: NodeIndex,
    join: NodeIndex,
    then_nodes: &HashSet<NodeIndex>,
) -> bool {
    // then_nodes以外のノードからの入辺が存在してはならない
    // ただし、headからthenへのエッジは存在してもよい
    for &node in then_nodes {
        for edge in cfg.graph().edges_directed(node, Direction::Incoming) {
            let source = edge.source();
            if source == head && node == then_entry {
                continue;
            }
            if !then_nodes.contains(&source) {
                return true;
            }
        }
    }

    // then_nodes,join以外のノードへの出辺が存在してはならない
    for &node in then_nodes {
        for edge in cfg.graph().edges_directed(node, Direction::Outgoing) {
            let target = edge.target();
            if !(then_nodes.contains(&target) || target == join) {
                return true;
            }
        }
    }

    // headについて、then_entry以外にthen_nodesへの出辺が存在してはならない
    for edge in cfg.graph().edges_directed(head, Direction::Outgoing) {
        let target = edge.target();
        if then_nodes.contains(&target) && target != then_entry {
            return true;
        }
    }

    false
}
