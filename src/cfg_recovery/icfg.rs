use std::collections::HashSet;

use id_arena::Arena;
use petgraph::graph::NodeIndex;
use petgraph::{prelude::StableGraph, visit::Dfs};

use crate::dot::export_cfg_to_dot;
use crate::{
    cfg_recovery::cfg::{Block, BlockId, Cfg, EdgeLabel},
    disassemble::Instruction,
    graph::{IndexedGraph, IndexedGraphView, IndexedGraphViewMut},
};

#[derive(Debug, Clone)]
pub struct Icfg {
    blocks: Arena<Block>,
    inner: IndexedGraph<BlockId, EdgeLabel>,
}

impl Default for Icfg {
    fn default() -> Self {
        Self {
            blocks: Arena::new(),
            inner: IndexedGraph::new(),
        }
    }
}

impl IndexedGraphView for Icfg {
    type Key = BlockId;
    type Edge = EdgeLabel;

    fn inner(&self) -> &IndexedGraph<Self::Key, Self::Edge> {
        &self.inner
    }
}

impl IndexedGraphViewMut for Icfg {
    fn inner_mut(&mut self) -> &mut IndexedGraph<Self::Key, Self::Edge> {
        &mut self.inner
    }
}

impl Icfg {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().map(|(_, block)| block)
    }

    pub(crate) fn blocks_mut(&mut self) -> impl Iterator<Item = &mut Block> {
        self.blocks.iter_mut().map(|(_, block)| block)
    }

    pub(crate) fn add_block(&mut self, start: u64, end: u64, insns: Vec<Instruction>) -> NodeIndex {
        let block = self
            .blocks
            .alloc_with_id(|id| Block::new(id, start, end, insns, None));
        self.add_node(block)
    }

    pub fn graph(&self) -> &StableGraph<BlockId, EdgeLabel> {
        IndexedGraphView::graph(self)
    }

    pub(crate) fn graph_mut(&mut self) -> &mut StableGraph<BlockId, EdgeLabel> {
        IndexedGraphViewMut::graph_mut(self)
    }

    pub fn extract_function_by_label(self, label: &str) -> Option<Cfg> {
        let entry_block = self
            .blocks
            .iter()
            .map(|(_, block)| block)
            .find(|block| block.label() == Some(label))?
            .id();
        self.extract_function(entry_block)
    }

    pub fn extract_function(self, entry_block: BlockId) -> Option<Cfg> {
        let entry_node = self.node_for_key(entry_block)?;
        let func_graph = reachable_subgraph(self.graph(), entry_node);
        let cfg = Cfg::from_graph(self.blocks, func_graph, entry_block);
        Some(cfg)
    }

    pub fn dot(&self) -> String {
        export_cfg_to_dot(&self.blocks, self.graph())
    }
}

fn reachable_subgraph<N: Clone, E: Clone>(
    graph: &StableGraph<N, E>,
    start: petgraph::graph::NodeIndex,
) -> StableGraph<N, E> {
    assert!(graph.contains_node(start));

    let mut dfs = Dfs::new(graph, start);
    let mut reachable = HashSet::new();

    while let Some(node_index) = dfs.next(graph) {
        reachable.insert(node_index);
    }

    graph.filter_map(
        |node_index, node_weight| {
            reachable
                .contains(&node_index)
                .then_some(node_weight.clone())
        },
        |_, edge_weight| Some(edge_weight.clone()),
    )
}
