use std::collections::{HashSet, VecDeque};

use petgraph::{Direction, graph::NodeIndex};

use crate::{
    cfg_recovery::cfg::{Condition, EdgeLabel},
    cfg_structuring::{
        region::{Region, RegionCfg, RegionDominanceView, RegionId},
        scope::{is_in_scope, scoped_neighbors},
    },
    graph::{IndexedGraphView, IndexedGraphViewMut},
};

#[derive(Debug)]
pub struct IfSchema {
    pub head: RegionId,
    pub then_body: Vec<RegionId>,
    pub else_body: Option<Vec<RegionId>>,
    pub join: RegionId,
    pub cond: Option<Condition>,
}

impl IfSchema {
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

    pub fn then_body_last(&self) -> Option<RegionId> {
        self.then_body.last().cloned()
    }

    pub fn else_body_last(&self) -> Option<RegionId> {
        self.else_body.as_ref()?.last().cloned()
    }
}

impl From<IfSchema> for Region {
    fn from(if_schema: IfSchema) -> Self {
        Region::If {
            head: if_schema.head,
            then_br: if_schema.then_body,
            else_br: if_schema.else_body,
            join: if_schema.join,
            cond: if_schema.cond,
        }
    }
}

pub(crate) fn find_if(dominance: &RegionDominanceView<'_>, head: NodeIndex) -> Option<IfSchema> {
    find_if_with_scope(dominance, head, None)
}

pub(crate) fn find_if_in_scope(
    dominance: &RegionDominanceView<'_>,
    head: NodeIndex,
    scope: &HashSet<RegionId>,
) -> Option<IfSchema> {
    find_if_with_scope(dominance, head, Some(scope))
}

fn find_if_with_scope(
    dominance: &RegionDominanceView<'_>,
    head: NodeIndex,
    scope: Option<&HashSet<RegionId>>,
) -> Option<IfSchema> {
    let cfg = dominance.cfg();
    if let Some(scope) = scope {
        let head_region = cfg.key_for_node(head)?;
        if !scope.contains(&head_region) {
            return None;
        }
    }

    let mut succs = scoped_neighbors(cfg, head, Direction::Outgoing, scope);
    let succ1 = succs.next()?;
    let succ2 = succs.next()?;
    if succs.next().is_some() || succ1 == cfg.vexit() || succ2 == cfg.vexit() {
        return None;
    }

    let join = dominance.immediate_post_dominator(head)?;
    let (then_entry, else_entry) = match (succ1 == join, succ2 == join) {
        (false, true) => (succ1, None),
        (true, false) => (succ2, None),
        (false, false) => (succ1, Some(succ2)),
        _ => return None,
    };
    let other_entry = else_entry.unwrap_or(join);

    let head_to_then = cfg
        .edge_label(head, then_entry)
        .expect("missing edge from head to then_entry");
    let head_to_other = cfg
        .edge_label(head, other_entry)
        .expect("missing edge from head to join");
    if !head_to_then.is_branch() || !head_to_other.is_branch() {
        return None;
    }

    let then_nodes = collect_nodes_between(cfg, dominance, head, then_entry, join, scope);
    if then_nodes.is_empty()
        || contains_sess_violation(cfg, head, then_entry, &then_nodes, join, scope)
    {
        return None;
    }

    let else_nodes = match else_entry {
        Some(else_entry) => {
            let else_nodes = collect_nodes_between(cfg, dominance, head, else_entry, join, scope);
            if else_nodes.is_empty()
                || contains_sess_violation(cfg, head, else_entry, &else_nodes, join, scope)
            {
                return None;
            }
            if !then_nodes.is_disjoint(&else_nodes) {
                return None;
            }
            Some(else_nodes)
        }
        None => None,
    };

    Some(IfSchema {
        head: cfg
            .key_for_node(head)
            .expect("missing region node for head"),
        then_body: then_nodes
            .into_iter()
            .map(|node| {
                cfg.key_for_node(node)
                    .expect("missing region node for then node")
            })
            .collect(),
        else_body: else_nodes.map(|nodes| {
            nodes
                .into_iter()
                .map(|node| {
                    cfg.key_for_node(node)
                        .expect("missing region node for else node")
                })
                .collect()
        }),
        join: cfg
            .key_for_node(join)
            .expect("missing region node for join"),
        cond: head_to_then.effective_condition(),
    })
}

pub(crate) fn contract_if(cfg: &mut RegionCfg, if_schema: IfSchema) -> RegionId {
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

    if let Some(then_tail) = if_schema.then_body_last() {
        let then_tail_node = cfg
            .node_for_key(then_tail)
            .expect("missing node for then tail");
        cfg.remove_edge_label(then_tail_node, join_node)
            .expect("missing edge from then tail to join");
    }
    if let Some(else_tail) = if_schema.else_body_last() {
        let else_tail_node = cfg
            .node_for_key(else_tail)
            .expect("missing node for else tail");
        cfg.remove_edge_label(else_tail_node, join_node)
            .expect("missing edge from else tail to join");
    }

    let if_region: Region = if_schema.into();
    let (if_region_id, if_node) = cfg.add_region(if_region);
    cfg.redirect_edges(head_node, if_node, Direction::Incoming);
    cfg.graph_mut()
        .add_edge(if_node, join_node, EdgeLabel::Unconditional);

    for node in nodes {
        if node == join_node {
            continue;
        }
        cfg.remove_node_by_index(node)
            .expect("failed to remove node in if-then");
    }

    if_region_id
}

// headに支配され、かつjoinに後続支配されるノードを探索する
fn collect_nodes_between(
    cfg: &RegionCfg,
    dominance: &RegionDominanceView<'_>,
    head: NodeIndex,
    then_entry: NodeIndex,
    join: NodeIndex,
    scope: Option<&HashSet<RegionId>>,
) -> HashSet<NodeIndex> {
    let mut queue = VecDeque::new();
    let mut visited = HashSet::new();

    queue.push_back(then_entry);
    while let Some(u) = queue.pop_front() {
        if visited.contains(&u) || u == join {
            continue;
        }

        if !is_in_scope(cfg, u, scope)
            || !dominance.dominates(head, u)
            || !dominance.post_dominates(join, u)
        {
            continue;
        }

        visited.insert(u);
        queue.extend(scoped_neighbors(cfg, u, Direction::Outgoing, scope));
    }
    visited
}

// then_nodesがSESSに違反する辺を含むか判定する
fn contains_sess_violation(
    cfg: &RegionCfg,
    head: NodeIndex,
    body_entry: NodeIndex,
    body_nodes: &HashSet<NodeIndex>,
    join: NodeIndex,
    scope: Option<&HashSet<RegionId>>,
) -> bool {
    // then_nodes以外のノードからの入辺が存在してはならない
    // ただし、headからthenへのエッジは存在してもよい
    for &node in body_nodes {
        for pred in scoped_neighbors(cfg, node, Direction::Incoming, scope) {
            if pred == head && node == body_entry {
                continue;
            }
            if !body_nodes.contains(&pred) {
                return true;
            }
        }
    }

    // then_nodes,join以外のノードへの出辺が存在してはならない
    for &node in body_nodes {
        for succ in scoped_neighbors(cfg, node, Direction::Outgoing, scope) {
            if !(body_nodes.contains(&succ) || succ == join) {
                return true;
            }
        }
    }

    // headについて、then_entry以外にthen_nodesへの出辺が存在してはならない
    for succ in scoped_neighbors(cfg, head, Direction::Outgoing, scope) {
        if body_nodes.contains(&succ) && succ != body_entry {
            return true;
        }
    }

    false
}
