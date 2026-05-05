use std::collections::HashSet;

use petgraph::{Direction, graph::NodeIndex};

use crate::{
    cfg_structuring::region::{RegionCfg, RegionId},
    graph::IndexedGraphView,
};

pub(crate) fn is_in_scope(
    cfg: &RegionCfg,
    node: NodeIndex,
    scope: Option<&HashSet<RegionId>>,
) -> bool {
    let Some(scope) = scope else {
        return true;
    };
    cfg.key_for_node(node).is_some_and(|id| scope.contains(&id))
}

pub(crate) fn scoped_neighbors<'a>(
    cfg: &'a RegionCfg,
    node: NodeIndex,
    direction: Direction,
    scope: Option<&'a HashSet<RegionId>>,
) -> impl Iterator<Item = NodeIndex> + 'a {
    cfg.graph()
        .neighbors_directed(node, direction)
        .filter(move |neighbor| is_in_scope(cfg, *neighbor, scope))
}

pub(crate) fn scoped_neighbor_if_one(
    cfg: &RegionCfg,
    node: NodeIndex,
    direction: Direction,
    scope: Option<&HashSet<RegionId>>,
) -> Option<NodeIndex> {
    let mut neighbors = scoped_neighbors(cfg, node, direction, scope);
    let (Some(neighbor), None) = (neighbors.next(), neighbors.next()) else {
        return None;
    };
    Some(neighbor)
}

pub(crate) fn scoped_degree(
    cfg: &RegionCfg,
    node: NodeIndex,
    direction: Direction,
    scope: Option<&HashSet<RegionId>>,
) -> usize {
    scoped_neighbors(cfg, node, direction, scope).count()
}
