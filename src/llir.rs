use std::fmt;

use id_arena::{Arena, Id};

use crate::graph::IndexedGraph;

pub mod cfg_recovery;
pub mod icfg;
pub mod lifter;

#[derive(Debug, Clone)]
pub struct Function {
    name: String,
    entry: BlockId,
    blocks: Arena<Block>,
    graph: IndexedGraph<BlockId, EdgeKind>,
}

impl Function {
    pub fn new(
        name: String,
        entry: BlockId,
        blocks: Arena<Block>,
        graph: IndexedGraph<BlockId, EdgeKind>,
    ) -> Self {
        Self {
            name,
            entry,
            blocks,
            graph,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entry(&self) -> BlockId {
        self.entry
    }

    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().map(|(_, block)| block)
    }
}

pub type BlockId = Id<Block>;

#[derive(Debug, Clone)]
pub struct Block {
    id: BlockId,
    label: Option<String>,
    start: u64,
    end: u64,
    instructions: Vec<Instruction>,
    terminator: Terminator,
}

impl Block {
    pub fn new(
        id: BlockId,
        instructions: Vec<Instruction>,
        terminator: Terminator,
        label: Option<String>,
    ) -> Self {
        let start = instructions
            .first()
            .map(|insn| insn.addr)
            .unwrap_or(terminator.addr);
        let end = terminator.addr;
        Self {
            id,
            start,
            end,
            label,
            instructions,
            terminator,
        }
    }

    pub fn id(&self) -> BlockId {
        self.id
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn start_addr(&self) -> u64 {
        self.instructions
            .first()
            .map(|insn| insn.addr)
            .unwrap_or(self.terminator.addr)
    }

    pub fn end_addr(&self) -> u64 {
        self.terminator.addr
    }

    pub fn instructions(&self) -> &[Instruction] {
        &self.instructions
    }

    pub fn terminator(&self) -> &Terminator {
        &self.terminator
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Unconditional,
    TrueBranch,
    FalseBranch,
}

#[derive(Debug, Clone)]
pub struct LinearProgram {
    items: Vec<InstructionOrTerminator>,
}

#[derive(Debug, Clone)]
pub enum InstructionOrTerminator {
    Instruction(Instruction),
    Terminator(Terminator),
}

impl InstructionOrTerminator {
    pub fn instruction(&self) -> Option<&Instruction> {
        match self {
            Self::Instruction(insn) => Some(insn),
            Self::Terminator(_) => None,
        }
    }

    pub fn terminator(&self) -> Option<&Terminator> {
        match self {
            Self::Instruction(_) => None,
            Self::Terminator(term) => Some(term),
        }
    }

    pub fn addr(&self) -> u64 {
        match self {
            Self::Instruction(insn) => insn.addr(),
            Self::Terminator(term) => term.addr(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Instruction {
    addr: u64,
    kind: InstructionKind,
}

impl From<Instruction> for InstructionOrTerminator {
    fn from(insn: Instruction) -> Self {
        Self::Instruction(insn)
    }
}

impl Instruction {
    pub fn addr(&self) -> u64 {
        self.addr
    }

    // TODO
    pub fn next_addr(&self) -> u64 {
        self.addr() + 4
    }
}

#[derive(Debug, Clone)]
pub enum InstructionKind {
    Assign { dst: Var, src: Rhs },
    Store { dst: Value, src: Value },
    Call { target: Value },
}

#[derive(Debug, Clone)]
pub struct Terminator {
    addr: u64,
    kind: TerminatorKind,
}

impl Terminator {
    pub fn addr(&self) -> u64 {
        self.addr
    }

    // TODO
    pub fn next_addr(&self) -> u64 {
        self.addr() + 4
    }

    pub fn kind(&self) -> &TerminatorKind {
        &self.kind
    }

    pub fn branch_target(&self) -> Option<&BranchTarget> {
        match &self.kind {
            TerminatorKind::Branch { target } => Some(target),
            TerminatorKind::ConditionalBranch { target, .. } => Some(target),
            _ => None,
        }
    }

    pub fn is_conditional_branch(&self) -> bool {
        matches!(self.kind, TerminatorKind::ConditionalBranch { .. })
    }
}

impl From<Terminator> for InstructionOrTerminator {
    fn from(term: Terminator) -> Self {
        Self::Terminator(term)
    }
}

#[derive(Debug, Clone)]
pub enum TerminatorKind {
    Branch {
        target: BranchTarget,
    },
    ConditionalBranch {
        cond: BranchCondition,
        target: BranchTarget,
    },
    Ret,
}

#[derive(Debug, Clone)]
pub enum BranchCondition {
    NonZero(Value),
    Ge,
}

#[derive(Debug, Clone)]
pub enum BranchTarget {
    Imm(u64),
}

#[derive(Debug, Clone)]
pub enum Rhs {
    BinOp { op: BinOp, lhs: Value, rhs: Value },
    Load { src: Value },
    Copy { src: Value },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Var {
    Reg(Reg),
    Temp(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    X(u8),
    W(u8),
    SP,
    XZR,
    WZR,
}

impl Reg {
    pub fn byte_width(self) -> i64 {
        match self {
            Self::W(_) | Self::WZR => 4,
            Self::X(_) | Self::SP | Self::XZR => 8,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Var(Var),
    Imm(i64),
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {} {{", self.name)?;
        let mut blocks: Vec<_> = self.blocks().collect();
        blocks.sort_by_key(|block| block.start());
        for block in blocks {
            match &block.label {
                Some(label) => writeln!(f, "  {:?} {label}:", block.id())?,
                None => writeln!(f, "  {:?}:", block.id())?,
            }
            for insn in &block.instructions {
                writeln!(f, "    {insn}")?;
            }
            writeln!(f, "    {}", block.terminator)?;
        }
        writeln!(f, "}}")
    }
}

impl fmt::Display for LinearProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "text {{")?;
        for item in &self.items {
            writeln!(f, "  {item}")?;
        }
        writeln!(f, "}}")
    }
}

impl fmt::Display for InstructionOrTerminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instruction(insn) => write!(f, "{insn}"),
            Self::Terminator(term) => write!(f, "{term}"),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}: {}", self.addr, self.kind)
    }
}

impl fmt::Display for InstructionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign { dst, src } => write!(f, "{dst} = {src}"),
            Self::Store { dst, src } => write!(f, "store [{dst}], {src}"),
            Self::Call { target } => write!(f, "call {target}"),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}: {}", self.addr, self.kind)
    }
}

impl fmt::Display for TerminatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Branch { target } => write!(f, "goto {target}"),
            Self::ConditionalBranch { cond, target } => write!(f, "if {cond} goto {target}"),
            Self::Ret => write!(f, "ret"),
        }
    }
}

impl fmt::Display for BranchCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonZero(value) => write!(f, "{value} != 0"),
            Self::Ge => write!(f, ">= 0"),
        }
    }
}

impl fmt::Display for BranchTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Imm(imm) => write!(f, "{imm:#x}"),
        }
    }
}

impl fmt::Display for Rhs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BinOp { op, lhs, rhs } => write!(f, "{op} {lhs}, {rhs}"),
            Self::Load { src } => write!(f, "load [{src}]"),
            Self::Copy { src } => write!(f, "copy {src}"),
        }
    }
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "add"),
            Self::Sub => write!(f, "sub"),
        }
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reg(reg) => write!(f, "{reg}"),
            Self::Temp(idx) => write!(f, "t{idx}"),
        }
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::X(n) => write!(f, "x{n}"),
            Self::W(n) => write!(f, "w{n}"),
            Self::SP => write!(f, "sp"),
            Self::XZR => write!(f, "xzr"),
            Self::WZR => write!(f, "wzr"),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Var(var) => write!(f, "{var}"),
            Self::Imm(imm) => write!(f, "{imm:#x}"),
        }
    }
}
