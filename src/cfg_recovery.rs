pub mod cfg;
pub mod icfg;

use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::graph::NodeIndex;

use crate::{
    binary::Symbol,
    cfg_recovery::{
        cfg::{Condition, EdgeLabel},
        icfg::Icfg,
    },
    disassemble::{Instruction, InstructionSequence},
    graph::IndexedGraphViewMut,
};

pub fn recover_cfg(insns: &InstructionSequence, symbols: &[Symbol]) -> Icfg {
    CfgRecovery::new().recover(insns, symbols)
}

#[derive(Debug)]
struct CfgRecovery {
    graph: Icfg,
    addr_to_node: HashMap<u64, NodeIndex>,
}

impl CfgRecovery {
    fn new() -> Self {
        Self {
            graph: Icfg::new(),
            addr_to_node: HashMap::new(),
        }
    }

    fn recover(mut self, insns: &InstructionSequence, symbols: &[Symbol]) -> Icfg {
        self.construct_blocks(insns);
        self.construct_edges(insns);
        self.label_blocks(symbols);
        self.graph
    }

    fn construct_blocks(&mut self, insns: &InstructionSequence) {
        let starts = find_block_start_addrs(insns);
        let mut worklist: VecDeque<_> = starts.iter().copied().collect();
        let mut seen = HashSet::new();

        while let Some(start_addr) = worklist.pop_front() {
            if !seen.insert(start_addr) {
                continue;
            }

            let Some(first_insn) = insns.get(start_addr) else {
                continue;
            };

            let mut cur_block_insns = vec![first_insn.clone()];
            while let Some(cur_insn) = cur_block_insns.last() {
                if cur_insn.is_terminator() {
                    break;
                }
                let Some(next_insn) = insns.fallthrough_insn(cur_insn) else {
                    break;
                };
                if starts.contains(&next_insn.addr()) {
                    break;
                }
                cur_block_insns.push(next_insn.clone());
            }

            let last_insn = cur_block_insns
                .last()
                .expect("block must have at least one instruction");
            self.add_block(start_addr, last_insn.addr(), cur_block_insns);
        }
    }

    fn construct_edges(&mut self, insns: &InstructionSequence) {
        let mut edges = Vec::new();
        for block in self.graph.blocks() {
            let term_insn = block.terminator();
            if term_insn.is_ret() {
                continue;
            }

            if let Some(target) = term_insn.conditional_branch_target() {
                let head_cond = Condition::from_block(block);
                edges.push((
                    block.start(),
                    target,
                    EdgeLabel::TrueBranch(head_cond.clone()),
                ));
                if let Some(fallthrough_insn) = insns.fallthrough_insn(term_insn) {
                    edges.push((
                        block.start(),
                        fallthrough_insn.addr(),
                        EdgeLabel::FalseBranch(head_cond.clone()),
                    ));
                }
            } else if let Some(target) = term_insn.unconditional_branch_target() {
                edges.push((block.start(), target, EdgeLabel::Unconditional));
            } else if let Some(fallthrough_insn) = insns.fallthrough_insn(term_insn) {
                edges.push((
                    block.start(),
                    fallthrough_insn.addr(),
                    EdgeLabel::Unconditional,
                ));
            }
        }
        for (start, end, label) in edges {
            self.add_edge(start, end, label);
        }
    }

    fn label_blocks(&mut self, symbols: &[Symbol]) {
        for symbol in symbols {
            if let Some(block) = self
                .graph
                .blocks_mut()
                .find(|block| block.start() == symbol.addr())
            {
                block.set_label(symbol.name());
            }
        }
    }

    fn add_block(&mut self, start: u64, end: u64, insns: Vec<Instruction>) {
        let node = self.graph.add_block(start, end, insns);
        self.addr_to_node.insert(start, node);
    }

    fn add_edge(&mut self, start: u64, end: u64, label: EdgeLabel) {
        let from_node = self.addr_to_node[&start];
        let to_node = self.addr_to_node[&end];
        self.graph.graph_mut().add_edge(from_node, to_node, label);
    }
}

fn find_block_start_addrs(insns: &InstructionSequence) -> HashSet<u64> {
    let mut starts = HashSet::new();

    for (i, insn) in insns.iter().enumerate() {
        // 最初の命令
        if i == 0 {
            starts.insert(insn.addr());
        }

        // 分岐命令の分岐先
        if let Some(target) = insn.branch_target() {
            starts.insert(target);
        }

        // 条件分岐命令の次
        if insn.is_conditional_branch()
            && let Some(next_insn) = insns.fallthrough_insn(insn)
        {
            starts.insert(next_insn.addr());
        }
    }

    starts
}
