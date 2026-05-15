use std::collections::HashSet;

use id_arena::Arena;
use petgraph::{graph::NodeIndex, prelude::StableGraph, visit::Dfs};

use crate::{
    cfg_structuring::EdgeLabel,
    dot::export_llir_graph_to_dot,
    graph::{IndexedGraph, IndexedGraphView, IndexedGraphViewMut},
    llir::{Block, BlockId, Function, Instruction, Terminator},
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

    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(id)
    }

    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().map(|(_, block)| block)
    }

    pub fn blocks_mut(&mut self) -> impl Iterator<Item = &mut Block> {
        self.blocks.iter_mut().map(|(_, block)| block)
    }

    pub fn add_block(&mut self, insns: Vec<Instruction>, term: Terminator) -> NodeIndex {
        let block = self
            .blocks
            .alloc_with_id(|id| Block::new(id, None, insns, term));
        self.inner.add_node(block)
    }

    pub fn extract_function_by_label(self, label: &str) -> Option<Function> {
        let entry_block = self
            .blocks
            .iter()
            .map(|(_, block)| block)
            .find(|block| block.label() == Some(label))?
            .id();
        self.extract_function(entry_block)
    }

    pub fn extract_function(self, id: BlockId) -> Option<Function> {
        let entry_block = self.blocks.get(id)?;
        let name = entry_block.label().unwrap_or("entry").to_string();
        let entry_node = self.node_for_key(id)?;
        let func_graph = reachable_subgraph(self.graph(), entry_node);
        Some(Function::new(
            name,
            id,
            self.blocks,
            IndexedGraph::from_graph(func_graph),
        ))
    }

    pub fn dot(&self) -> String {
        export_llir_graph_to_dot(&self.blocks, self.graph())
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
