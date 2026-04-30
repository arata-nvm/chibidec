use std::{
    collections::{HashMap, HashSet, VecDeque},
    fmt,
};

use anyhow::{Context, Result, anyhow};
use petgraph::{
    dot::{Config, Dot},
    graph::NodeIndex,
    prelude::StableGraph,
    stable_graph::EdgeReference,
    visit::Dfs,
};

use crate::disassemble::Instruction;

pub type BlockId = u32;

#[derive(Debug, Clone)]
pub struct Block {
    pub id: BlockId,
    pub start: u64,
    pub end: u64,
    pub instructions: Vec<Instruction>,
    pub label: Option<String>,
}

impl Block {
    pub fn new(
        id: BlockId,
        start: u64,
        end: u64,
        instructions: Vec<Instruction>,
        label: Option<String>,
    ) -> Self {
        Self {
            id,
            start,
            end,
            instructions,
            label,
        }
    }

    pub fn terminator(&self) -> Option<&Instruction> {
        self.instructions.last()
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockStore {
    next_id: BlockId,
    blocks: HashMap<BlockId, Block>,
    block_to_node: HashMap<BlockId, Option<NodeIndex>>,
}

impl BlockStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, id: BlockId) -> &Block {
        self.blocks.get(&id).expect("Block not found")
    }

    pub fn get_mut(&mut self, id: BlockId) -> &mut Block {
        self.blocks.get_mut(&id).expect("Block not found")
    }

    pub fn new_block(
        &mut self,
        start: u64,
        end: u64,
        instructions: Vec<Instruction>,
        label: Option<String>,
    ) -> BlockId {
        let id = self.next_id;
        self.next_id += 1;
        let block = Block::new(id, start, end, instructions, label);
        self.blocks.insert(id, block);
        id
    }

    pub fn get_node_index(&self, id: BlockId) -> Option<NodeIndex> {
        *self.block_to_node.get(&id).unwrap_or(&None)
    }

    pub fn add_block_to_graph(&mut self, graph: &mut Cfg, id: BlockId) -> NodeIndex {
        assert!(self.get(id).id == id);
        let node_index = graph.add_node(id);
        self.block_to_node.insert(id, Some(node_index));
        node_index
    }

    pub fn remove_block_from_graph(&mut self, graph: &mut Cfg, id: BlockId) -> Result<()> {
        // 書籍では.getだが、取り除いた方が良さそうなので.removeにした
        let node_index = self
            .block_to_node
            .remove(&id)
            .ok_or_else(|| anyhow!("block not found: {}", id))?;
        if let Some(node_index) = node_index {
            graph.remove_node(node_index);
        }
        Ok(())
    }

    pub fn num_blocks(&self) -> usize {
        self.blocks.len()
    }

    pub fn iter_blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.values()
    }

    pub fn build_initial_cfg(&mut self) -> (Cfg, HashMap<u64, NodeIndex>) {
        let block_count = self.num_blocks();
        let mut graph = Cfg::with_capacity(block_count, block_count * 2);

        let blocks: Vec<_> = self
            .blocks
            .values()
            .map(|block| (block.id, block.start))
            .collect();
        let addr_to_node = blocks
            .into_iter()
            .map(|(block_id, block_start)| {
                let node = self.add_block_to_graph(&mut graph, block_id);
                (block_start, node)
            })
            .collect();

        (graph, addr_to_node)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub op: String,
    pub lhs: String,
    pub rhs: String,
}

impl Condition {
    pub fn new(op: impl Into<String>, lhs: impl Into<String>, rhs: impl Into<String>) -> Self {
        Self {
            op: op.into(),
            lhs: lhs.into(),
            rhs: rhs.into(),
        }
    }

    pub fn from_block(block: &Block) -> Option<Self> {
        let term_insn = block.terminator()?;
        if !term_insn.is_conditional_jump() {
            return None;
        }

        assert!(term_insn.mnemonic == "cbnz");
        let reg = term_insn.operands.as_ref()?.first()?;
        Some(Self::new("!=", reg, "0"))
    }
}

impl fmt::Display for Condition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.lhs, self.op, self.rhs)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EdgeLabel {
    Unconditional,
    TrueBranch(Option<Condition>),
    FalseBranch(Option<Condition>),
    Virtualized,
}

impl EdgeLabel {
    fn color(&self) -> Option<&'static str> {
        match self {
            Self::TrueBranch(_) => Some("green"),
            Self::FalseBranch(_) => Some("red"),
            Self::Unconditional | Self::Virtualized => None,
        }
    }
}

pub type Cfg = StableGraph<BlockId, EdgeLabel>;

pub fn find_block_start_addrs(insns: &[Instruction]) -> HashSet<u64> {
    let mut starts = HashSet::new();

    if let Some(first) = insns.first() {
        starts.insert(first.addr);
    }

    for (i, insn) in insns.iter().enumerate() {
        let Some(target) = insn.imm else {
            continue;
        };

        if insn.is_unconditional_jump() || insn.is_call() {
            starts.insert(target);
            continue;
        }

        if insn.is_conditional_jump() {
            starts.insert(target);
            if let Some(next_insn_addr) = insns.get(i + 1).map(|insn| insn.addr) {
                starts.insert(next_insn_addr);
            }
        }
    }

    starts
}

pub fn construct_blocks(
    block_store: &mut BlockStore,
    addr_to_insn: &HashMap<u64, Instruction>,
    starts: &HashSet<u64>,
) -> Result<Vec<BlockId>> {
    let mut blocks = Vec::new();
    let mut worklist: VecDeque<_> = starts.iter().copied().collect();
    let mut seen = HashSet::new();

    while let Some(start_addr) = worklist.pop_front() {
        if !seen.insert(start_addr) {
            continue;
        }

        let mut cur_block_insns = match addr_to_insn.get(&start_addr) {
            Some(insn) => vec![insn.clone()],
            None => continue,
        };

        loop {
            let cur_insn = cur_block_insns
                .last()
                .context("current block has no instructions")?;
            if cur_insn.is_terminator() {
                break;
            }

            match get_next_insn(addr_to_insn, cur_insn) {
                Some(next_insn) => {
                    if starts.contains(&next_insn.addr) {
                        break;
                    }
                    cur_block_insns.push(next_insn);
                }
                None => break,
            }
        }

        let end_addr = cur_block_insns
            .last()
            .context("current block has no instructions")?
            .addr;
        let block = block_store.new_block(start_addr, end_addr, cur_block_insns, None);
        blocks.push(block);
    }

    Ok(blocks)
}

pub fn construct_graph(
    block_store: &mut BlockStore,
    addr_to_insn: &HashMap<u64, Instruction>,
) -> Result<Cfg> {
    let (mut graph, addr_to_node) = block_store.build_initial_cfg();
    for block in block_store.iter_blocks() {
        let term_insn = block.terminator().context("block has no terminator")?;
        if term_insn.is_ret() {
            continue;
        }

        let block_idx = addr_to_node[&block.start];
        if term_insn.is_conditional_jump() {
            let head_cond = Condition::from_block(block);
            if let Some(fallthrough_insn) = get_next_insn(addr_to_insn, term_insn) {
                graph.add_edge(
                    block_idx,
                    addr_to_node[&fallthrough_insn.addr],
                    EdgeLabel::FalseBranch(head_cond.clone()),
                );
            }
            if let Some(target_addr) = term_insn.imm {
                graph.add_edge(
                    block_idx,
                    addr_to_node[&target_addr],
                    EdgeLabel::TrueBranch(head_cond.clone()),
                );
            }
        } else if term_insn.is_unconditional_jump() {
            if let Some(target_addr) = term_insn.imm {
                graph.add_edge(
                    block_idx,
                    addr_to_node[&target_addr],
                    EdgeLabel::Unconditional,
                );
            }
        } else if let Some(fallthrough_insn) = get_next_insn(addr_to_insn, term_insn) {
            graph.add_edge(
                block_idx,
                addr_to_node[&fallthrough_insn.addr],
                EdgeLabel::Unconditional,
            );
        }
    }
    Ok(graph)
}

pub fn extract_main(graph: &Cfg, block_store: &BlockStore) -> Result<Cfg> {
    let main_entry_node = graph
        .node_indices()
        .find(|&idx| {
            block_store.get(*graph.node_weight(idx).unwrap()).label == Some("_main".into())
        })
        .context("failed to find main function")?;
    Ok(reachable_subgraph(graph, main_entry_node))
}

fn reachable_subgraph<N: Clone, E: Clone>(
    graph: &StableGraph<N, E>,
    start: NodeIndex,
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

fn get_next_insn(
    addr_to_insn: &HashMap<u64, Instruction>,
    insn: &Instruction,
) -> Option<Instruction> {
    let next_addr = insn.addr + insn.len as u64;
    addr_to_insn.get(&next_addr).cloned()
}

pub fn dot(graph: &Cfg, block_store: &BlockStore) -> String {
    let get_edge_attributes = |_, edge: EdgeReference<'_, EdgeLabel>| match edge.weight().color() {
        Some(color) => format!(r#"color = "{color}""#),
        None => String::new(),
    };
    let get_node_attributes = |_, (_, &block_id)| {
        let block = block_store.get(block_id);
        let mut lines = vec![
            format!(
                "Block(id: {}, label: {:?}) [{:#x}-{:#x}]",
                block.id,
                block.label.as_deref().unwrap_or(""),
                block.start,
                block.end
            ),
            "insns:".to_string(),
        ];
        lines.extend(block.instructions.iter().map(|insn| format!("  {insn}")));
        let label = lines.join("\n");
        format!(r#"label = "{}""#, escape_dot_label(label))
    };
    let dot = Dot::with_attr_getters(
        &graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &get_edge_attributes,
        &get_node_attributes,
    );
    format!("{dot:?}")
}

fn escape_dot_label(label: String) -> String {
    label
        .replace('\\', r"\\")
        .replace('"', r#"\""#)
        .replace('\n', r"\l")
        + r"\l"
}
