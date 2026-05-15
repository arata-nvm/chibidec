use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};
use id_arena::{Arena, Id};
use petgraph::{
    Direction,
    algo::dominators::{self, Dominators},
    graph::{EdgeIndex, NodeIndex},
    prelude::StableGraph,
    visit::{EdgeRef, Reversed},
};

use crate::{
    cfg_structuring::{
        Condition, EdgeLabel, branch_condition_for_block,
        cycle::{LoopKind, StructuredLoop},
    },
    dot::export_region_cfg_to_dot,
    graph::{IndexedGraph, IndexedGraphView, IndexedGraphViewMut},
    llir::{BlockId, Function},
};

pub type RegionId = Id<Region>;

#[derive(Clone, Debug)]
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
    Loop {
        kind: LoopKind,
        meta: StructuredLoop,
        body: RegionId,
    },
    VirtualExit,
}

#[derive(Debug, Clone)]
pub struct RegionCfg {
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
            inner: IndexedGraph::with_capacity(nodes, edges),
            vexit: NodeIndex::new(usize::MAX),
        }
    }

    pub(crate) fn add_region(
        &mut self,
        regions: &mut Arena<Region>,
        region: Region,
    ) -> (RegionId, NodeIndex) {
        let region_id = regions.alloc(region);
        let node = self.inner.add_node(region_id);
        (region_id, node)
    }

    pub(crate) fn add_existing_region(&mut self, region_id: RegionId) -> NodeIndex {
        self.inner.add_node(region_id)
    }

    // 出口から仮想的な出口ノードへのエッジを追加し、単一の出口を持つようにする
    pub(crate) fn add_vexit(&mut self, regions: &mut Arena<Region>) -> Result<NodeIndex> {
        let exit_nodes: Vec<_> = self.graph().externals(Direction::Outgoing).collect();
        if exit_nodes.is_empty() {
            bail!("cfg has no exit nodes");
        }

        let (_, vexit) = self.add_region(regions, Region::VirtualExit);
        self.vexit = vexit;

        for exit_node in exit_nodes {
            self.graph_mut()
                .add_edge(exit_node, vexit, EdgeLabel::Unconditional);
        }

        Ok(vexit)
    }

    pub fn graph(&self) -> &StableGraph<RegionId, EdgeLabel> {
        IndexedGraphView::graph(self)
    }

    pub fn vexit(&self) -> NodeIndex {
        self.vexit
    }

    pub fn compute_dominance(&self) -> Option<RegionDominanceView<'_>> {
        Some(RegionDominanceView::compute(
            self,
            self.entry()?,
            self.vexit(),
        ))
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

    pub(crate) fn redirect_edges(
        &mut self,
        target: NodeIndex,
        new_target: NodeIndex,
        dir: Direction,
    ) {
        self.redirect_edges_except(target, new_target, &HashSet::new(), dir);
    }

    pub(crate) fn redirect_edges_except(
        &mut self,
        target: NodeIndex,
        new_target: NodeIndex,
        exclude: &HashSet<NodeIndex>,
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
            .filter(|(_, other)| !exclude.contains(other))
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

    pub(crate) fn edge_condition(
        &self,
        func: &Function,
        regions: &Arena<Region>,
        source: NodeIndex,
        target: NodeIndex,
    ) -> Option<Condition> {
        let edge = self.graph().find_edge(source, target)?;
        self.edge_condition_by_id(func, regions, edge)
    }

    pub(crate) fn edge_condition_by_id(
        &self,
        func: &Function,
        regions: &Arena<Region>,
        edge: EdgeIndex,
    ) -> Option<Condition> {
        let (source, _) = self.graph().edge_endpoints(edge)?;
        let label = self.graph().edge_weight(edge)?;
        let source_region = self.key_for_node(source)?;
        let block = terminating_block(regions, source_region)?;
        let cond = branch_condition_for_block(func, block)?;
        label.apply_condition(&cond)
    }

    pub(crate) fn remove_edge_label(
        &mut self,
        from: NodeIndex,
        to: NodeIndex,
    ) -> Option<EdgeLabel> {
        let edge = self.graph().find_edge(from, to)?;
        self.graph_mut().remove_edge(edge)
    }

    pub fn dot(&self, regions: &Arena<Region>) -> String {
        export_region_cfg_to_dot(regions, self.graph())
    }
}

fn terminating_block(regions: &Arena<Region>, region_id: RegionId) -> Option<BlockId> {
    match regions.get(region_id)? {
        Region::Leaf(block) => Some(*block),
        Region::Seq(seq) => terminating_block(regions, *seq.last()?),
        Region::If { .. } | Region::Loop { .. } | Region::VirtualExit => None,
    }
}

pub struct RegionDominanceView<'cfg> {
    cfg: &'cfg RegionCfg,
    dom: Dominators<NodeIndex>,
    pdom: Dominators<NodeIndex>,
    vexit: NodeIndex,
}

impl<'cfg> RegionDominanceView<'cfg> {
    fn compute(cfg: &'cfg RegionCfg, entry: NodeIndex, vexit: NodeIndex) -> Self {
        let dom = dominators::simple_fast(cfg.graph(), entry);
        let pdom = dominators::simple_fast(Reversed(cfg.graph()), vexit);
        Self {
            cfg,
            dom,
            pdom,
            vexit,
        }
    }

    pub(crate) fn cfg(&self) -> &'cfg RegionCfg {
        self.cfg
    }

    pub fn dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        let Some(mut dominators) = self.dom.dominators(b) else {
            return false;
        };
        dominators.any(|d| d == a)
    }

    pub fn post_dominates(&self, a: NodeIndex, b: NodeIndex) -> bool {
        let Some(mut dominators) = self.pdom.dominators(b) else {
            return false;
        };
        dominators.any(|d| d == a)
    }

    pub fn immediate_post_dominator(&self, node: NodeIndex) -> Option<NodeIndex> {
        self.pdom.immediate_dominator(node)
    }

    pub fn backedge_sources(&self, node: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        self.cfg
            .graph()
            .neighbors_directed(node, Direction::Incoming)
            .filter(move |&pred| self.dominates(node, pred))
    }

    pub fn backedge_targets(&self, node: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
        self.cfg
            .graph()
            .neighbors_directed(node, Direction::Outgoing)
            .filter(move |&succ| self.dominates(succ, node))
    }

    pub fn has_backedge(&self, node: NodeIndex) -> bool {
        if node == self.vexit {
            return false;
        }
        self.backedge_targets(node).next().is_some()
    }
}
