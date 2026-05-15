use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::graph::NodeIndex;

use crate::{
    binary::Symbol,
    cfg_structuring::EdgeLabel,
    graph::IndexedGraphViewMut,
    llir::{
        BranchTarget, Instruction, InstructionOrTerminator, LinearProgram, Terminator,
        TerminatorKind, icfg::Icfg,
    },
};

pub fn recover_icfg(program: &LinearProgram, symbols: &[Symbol]) -> Icfg {
    CfgRecovery::new().recover(program, symbols)
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

    fn recover(mut self, program: &LinearProgram, symbols: &[Symbol]) -> Icfg {
        self.construct_blocks(program.items(), symbols);
        self.construct_edges();
        self.label_blocks(symbols);
        self.graph
    }

    fn construct_blocks(&mut self, items: &[InstructionOrTerminator], symbols: &[Symbol]) {
        let addr_to_index = build_addr_to_index(items);

        let starts = find_block_start_addrs(items, symbols);
        let mut worklist: VecDeque<_> = starts.iter().copied().collect();
        while let Some(start_addr) = worklist.pop_front() {
            let start_idx = addr_to_index
                .get(&start_addr)
                .expect("block start address must correspond to an instruction index");

            let mut idx = *start_idx;
            let mut insns = Vec::new();
            while let Some(item) = items.get(idx) {
                let insn = match item {
                    InstructionOrTerminator::Instruction(insn) => insn,
                    InstructionOrTerminator::Terminator(term) => {
                        self.add_block(insns, term.clone());
                        break;
                    }
                };

                insns.push(insn.clone());

                let next_addr = insn.next_addr();
                if starts.contains(&next_addr) {
                    let term = Terminator::new(
                        item.addr(),
                        TerminatorKind::Branch {
                            target: BranchTarget::Imm(next_addr),
                        },
                    );
                    self.add_block(insns, term);
                    break;
                }
                idx += 1;
            }
        }
    }

    fn construct_edges(&mut self) {
        let mut edges = Vec::new();
        for block in self.graph.blocks() {
            match &block.terminator().kind() {
                TerminatorKind::Branch {
                    target: BranchTarget::Imm(target),
                } => {
                    edges.push((block.start(), *target, EdgeLabel::Unconditional));
                }
                TerminatorKind::ConditionalBranch {
                    target: BranchTarget::Imm(target),
                    ..
                } => {
                    edges.push((block.start(), *target, EdgeLabel::TrueBranch));

                    let next_addr = block.terminator().next_addr();
                    if self.addr_to_node.contains_key(&next_addr) {
                        edges.push((block.start(), next_addr, EdgeLabel::FalseBranch));
                    }
                }
                TerminatorKind::Ret => {}
            }
        }

        for (from, to, edge) in edges {
            self.add_edge(from, to, edge);
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

    fn add_block(&mut self, insns: Vec<Instruction>, term: Terminator) {
        let start = insns
            .first()
            .map(|insn| insn.addr())
            .unwrap_or_else(|| term.addr());
        let block = self.graph.add_block(insns, term);
        self.addr_to_node.insert(start, block);
    }

    fn add_edge(&mut self, from_addr: u64, to_addr: u64, label: EdgeLabel) {
        let from_node = self.addr_to_node[&from_addr];
        let to_node = self.addr_to_node[&to_addr];
        self.graph.graph_mut().add_edge(from_node, to_node, label);
    }
}

fn build_addr_to_index(items: &[InstructionOrTerminator]) -> HashMap<u64, usize> {
    let mut addr_to_index = HashMap::new();
    for (idx, item) in items.iter().enumerate() {
        addr_to_index.entry(item.addr()).or_insert(idx);
    }
    addr_to_index
}

fn find_block_start_addrs(insns: &[InstructionOrTerminator], symbols: &[Symbol]) -> HashSet<u64> {
    let mut starts = HashSet::new();

    for symbol in symbols {
        starts.insert(symbol.addr());
    }

    // 最初の命令
    if let Some(first_insn) = insns.first() {
        starts.insert(first_insn.addr());
    }

    for (idx, insn) in insns.iter().enumerate() {
        let Some(term) = insn.terminator() else {
            continue;
        };

        // 分岐命令の分岐先
        if let Some(BranchTarget::Imm(target)) = term.branch_target() {
            starts.insert(*target);
        }

        // 条件分岐命令の次
        if let Some(next_insn) = insns.get(idx + 1) {
            assert!(next_insn.addr() != insn.addr());
            starts.insert(next_insn.addr());
        }
    }

    starts
}
