use std::fmt;

use id_arena::{Arena, Id};
use petgraph::{graph::NodeIndex, prelude::StableGraph};

use crate::{
    cfg_structuring::region::RegionId,
    disassemble::{ConditionKind, Instruction},
    dot::export_cfg_to_dot,
    graph::{IndexedGraph, IndexedGraphView},
};

pub type BlockId = Id<Block>;

#[derive(Debug, Clone)]
pub struct Block {
    id: BlockId,
    start: u64,
    end: u64,
    instructions: Vec<Instruction>,
    label: Option<String>,
}

impl Block {
    pub fn new(
        id: BlockId,
        start: u64,
        end: u64,
        instructions: Vec<Instruction>,
        label: Option<String>,
    ) -> Self {
        assert!(
            !instructions.is_empty(),
            "block must have at least one instruction"
        );
        Self {
            id,
            start,
            end,
            instructions,
            label,
        }
    }

    pub fn id(&self) -> BlockId {
        self.id
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub fn terminator(&self) -> &Instruction {
        self.instructions
            .last()
            .expect("block must have at least one instruction")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    op: String,
    lhs: String,
    rhs: String,
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
        let term_insn = block.terminator();
        match term_insn.conditional_branch_kind()? {
            ConditionKind::NonZero => Some(Self::new("!=", "TODO", "0")),
            ConditionKind::GreaterOrEqual => Some(Self::new(">", "TODO", "TODO")),
            ConditionKind::LessOrEqual => Some(Self::new("<=", "TODO", "TODO")),
        }
    }

    pub fn negate(&self) -> Self {
        let negated_op = match self.op.as_str() {
            "!=" => "==",
            ">" => "<=",
            "<=" => ">",
            _ => unimplemented!("unsupported condition operator: {}", self.op),
        };
        Self::new(negated_op, self.lhs.clone(), self.rhs.clone())
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
    Virtualized(TailKind),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TailKind {
    Break,
    Continue,
    Goto { target: RegionId },
}

impl EdgeLabel {
    pub fn is_branch(&self) -> bool {
        matches!(self, Self::TrueBranch(_) | Self::FalseBranch(_))
    }

    pub fn effective_condition(&self) -> Option<Condition> {
        match self {
            Self::TrueBranch(cond) => cond.clone(),
            Self::FalseBranch(cond) => cond.clone().map(|c| c.negate()),
            Self::Unconditional | Self::Virtualized(_) => None,
        }
    }

    pub fn color(&self) -> Option<&'static str> {
        match self {
            Self::TrueBranch(_) => Some("green"),
            Self::FalseBranch(_) => Some("red"),
            Self::Unconditional | Self::Virtualized(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cfg {
    #[allow(dead_code)]
    blocks: Arena<Block>,
    inner: IndexedGraph<BlockId, EdgeLabel>,
    entry: NodeIndex,
}

impl IndexedGraphView for Cfg {
    type Key = BlockId;
    type Edge = EdgeLabel;

    fn inner(&self) -> &IndexedGraph<Self::Key, Self::Edge> {
        &self.inner
    }
}

impl Cfg {
    pub fn from_graph(
        blocks: Arena<Block>,
        graph: StableGraph<BlockId, EdgeLabel>,
        entry_block: BlockId,
    ) -> Self {
        let inner = IndexedGraph::from_graph(graph);
        let entry = inner
            .node_for_key(entry_block)
            .expect("entry block must be in the graph");
        Self {
            blocks,
            inner,
            entry,
        }
    }

    pub fn graph(&self) -> &StableGraph<BlockId, EdgeLabel> {
        IndexedGraphView::graph(self)
    }

    pub fn entry(&self) -> NodeIndex {
        self.entry
    }

    pub fn dot(&self) -> String {
        export_cfg_to_dot(&self.blocks, self.graph())
    }
}
