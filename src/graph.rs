use std::{collections::HashMap, fmt, hash::Hash};

use anyhow::{Result, anyhow};
use petgraph::{graph::NodeIndex, prelude::StableGraph};

#[derive(Debug, Clone)]
pub struct IndexedGraph<K, E> {
    graph: StableGraph<K, E>,
    key_to_node: HashMap<K, NodeIndex>,
}

impl<K, E> Default for IndexedGraph<K, E> {
    fn default() -> Self {
        Self {
            graph: StableGraph::new(),
            key_to_node: HashMap::new(),
        }
    }
}

impl<K, E> IndexedGraph<K, E>
where
    K: Copy + Eq + Hash + fmt::Debug,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            graph: StableGraph::with_capacity(nodes, edges),
            key_to_node: HashMap::with_capacity(nodes),
        }
    }

    pub fn from_graph(graph: StableGraph<K, E>) -> Self {
        let key_to_node = graph
            .node_indices()
            .filter_map(|node| graph.node_weight(node).copied().map(|key| (key, node)))
            .collect();

        Self { graph, key_to_node }
    }

    pub fn graph(&self) -> &StableGraph<K, E> {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut StableGraph<K, E> {
        &mut self.graph
    }

    pub fn add_node(&mut self, key: K) -> NodeIndex {
        assert!(
            !self.key_to_node.contains_key(&key),
            "key already exists in graph: {key:?}",
        );

        let node = self.graph.add_node(key);
        self.key_to_node.insert(key, node);
        node
    }

    pub fn remove_node(&mut self, key: K) -> Result<()> {
        let node = self
            .key_to_node
            .remove(&key)
            .ok_or_else(|| anyhow!("key not found in graph: {key:?}"))?;
        self.graph
            .remove_node(node)
            .ok_or_else(|| anyhow!("graph node not found for key: {key:?}"))?;
        Ok(())
    }

    pub fn key_for_node(&self, node: NodeIndex) -> Option<K> {
        self.graph.node_weight(node).copied()
    }

    pub fn node_for_key(&self, key: K) -> Option<NodeIndex> {
        self.key_to_node.get(&key).copied()
    }
}

pub trait IndexedGraphView {
    type Key: Copy + Eq + Hash + fmt::Debug;
    type Edge;

    fn inner(&self) -> &IndexedGraph<Self::Key, Self::Edge>;

    fn graph(&self) -> &StableGraph<Self::Key, Self::Edge> {
        self.inner().graph()
    }

    fn key_for_node(&self, node: NodeIndex) -> Option<Self::Key> {
        self.inner().key_for_node(node)
    }

    fn node_for_key(&self, key: Self::Key) -> Option<NodeIndex> {
        self.inner().node_for_key(key)
    }
}

pub trait IndexedGraphViewMut: IndexedGraphView {
    fn inner_mut(&mut self) -> &mut IndexedGraph<Self::Key, Self::Edge>;

    fn graph_mut(&mut self) -> &mut StableGraph<Self::Key, Self::Edge> {
        self.inner_mut().graph_mut()
    }
}
