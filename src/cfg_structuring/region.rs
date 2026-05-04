use anyhow::{Result, anyhow};
use id_arena::{Arena, Id};
use petgraph::{
    Direction, algo::dominators::Dominators, graph::NodeIndex, prelude::StableGraph, visit::EdgeRef,
};

use crate::{
    cfg_recovery::cfg::{BlockId, Condition, EdgeLabel},
    dot::export_region_cfg_to_dot,
    graph::{IndexedGraph, IndexedGraphView, IndexedGraphViewMut},
};

pub type RegionId = Id<Region>;

#[derive(Clone, Debug, PartialEq)]
pub enum Region {
    Leaf(BlockId),
    Seq(Vec<RegionId>),
    If {
        head: RegionId,
        then_br: Vec<RegionId>,
        else_br: Option<Vec<RegionId>>,
        join: RegionId,
        cond: Option<Condition>,
    },
    VirtualExit,
}

#[derive(Debug, Clone)]
pub struct RegionCfg {
    regions: Arena<Region>,
    inner: IndexedGraph<RegionId, EdgeLabel>,
    vexit: NodeIndex,
}

impl IndexedGraphView for RegionCfg {
    type Key = RegionId;
    type Edge = EdgeLabel;

    fn inner(&self) -> &IndexedGraph<Self::Key, Self::Edge> {
        &self.inner
    }
}

impl IndexedGraphViewMut for RegionCfg {
    fn inner_mut(&mut self) -> &mut IndexedGraph<Self::Key, Self::Edge> {
        &mut self.inner
    }
}

impl RegionCfg {
    pub(crate) fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            regions: Arena::new(),
            inner: IndexedGraph::with_capacity(nodes, edges),
            vexit: NodeIndex::new(usize::MAX),
        }
    }

    pub(crate) fn add_region(&mut self, region: Region) -> (RegionId, NodeIndex) {
        let region_id = self.regions.alloc(region);
        let node = self.inner.add_node(region_id);
        (region_id, node)
    }

    pub(crate) fn set_vexit(&mut self, vexit: NodeIndex) {
        self.vexit = vexit;
    }

    pub fn graph(&self) -> &StableGraph<RegionId, EdgeLabel> {
        IndexedGraphView::graph(self)
    }

    pub fn vexit(&self) -> NodeIndex {
        self.vexit
    }

    pub(crate) fn remove_node_by_index(&mut self, node: NodeIndex) -> Result<()> {
        let key = self
            .key_for_node(node)
            .ok_or_else(|| anyhow!("graph node not found for index: {node:?}"))?;
        self.inner.remove_node(key)
    }

    pub(crate) fn entry(&self) -> Option<NodeIndex> {
        self.graph().externals(Direction::Incoming).next()
    }

    pub(crate) fn degree(&self, node: NodeIndex, direction: Direction) -> usize {
        self.graph().edges_directed(node, direction).count()
    }

    pub(crate) fn backedge_targets(
        &self,
        node: NodeIndex,
        dominators: &Dominators<NodeIndex>,
    ) -> impl Iterator<Item = NodeIndex> {
        return self
            .graph()
            .neighbors_directed(node, Direction::Outgoing)
            .filter(move |&succ| dominates(succ, node, dominators));

        fn dominates(
            node1: NodeIndex,
            node2: NodeIndex,
            dominators: &Dominators<NodeIndex>,
        ) -> bool {
            dominators
                .dominators(node2)
                .is_some_and(|mut it| it.any(|d| d == node1))
        }
    }

    pub(crate) fn has_backedge(&self, node: NodeIndex, dominators: &Dominators<NodeIndex>) -> bool {
        if node == self.vexit {
            return false;
        }
        self.backedge_targets(node, dominators).next().is_some()
    }

    // nodeのdir方向の隣接ノードが1つだけ存在する場合は、そのノードを返す。それ以外の場合はNoneを返す。
    pub(crate) fn neighbor_if_one(
        &self,
        node: NodeIndex,
        direction: Direction,
    ) -> Option<NodeIndex> {
        let mut neighbors = self.graph().neighbors_directed(node, direction);
        let (Some(neighbor), None) = (neighbors.next(), neighbors.next()) else {
            return None;
        };
        Some(neighbor)
    }

    pub(crate) fn redirect_edges(
        &mut self,
        target: NodeIndex,
        new_target: NodeIndex,
        dir: Direction,
    ) {
        let edges: Vec<_> = self
            .graph()
            .edges_directed(target, dir)
            .map(|edge| {
                let other = match dir {
                    Direction::Incoming => edge.source(),
                    Direction::Outgoing => edge.target(),
                };
                (edge.id(), other)
            })
            .collect();

        for (edge_id, other) in edges {
            let Some(weight) = self.graph_mut().remove_edge(edge_id) else {
                continue;
            };
            let (source, target) = match dir {
                Direction::Incoming => (other, new_target),
                Direction::Outgoing => (new_target, other),
            };
            if !self.graph().contains_edge(source, target) {
                self.graph_mut().add_edge(source, target, weight);
            }
        }
    }

    pub fn edge_label(&self, from: NodeIndex, to: NodeIndex) -> Option<&EdgeLabel> {
        let edge = self.graph().find_edge(from, to)?;
        self.graph().edge_weight(edge)
    }

    pub fn remove_edge_label(&mut self, from: NodeIndex, to: NodeIndex) -> Option<EdgeLabel> {
        let edge = self.graph().find_edge(from, to)?;
        self.graph_mut().remove_edge(edge)
    }

    pub fn dot(&self) -> String {
        export_region_cfg_to_dot(&self.regions, self.graph())
    }
}
