use std::{
    collections::{BTreeMap, HashSet},
    fmt,
};

use id_arena::{Arena, Id};
use petgraph::graph::NodeIndex;

use crate::graph::IndexedGraph;

pub mod cfg_recovery;
pub mod icfg;
pub mod lifter;
pub mod ssa;

#[derive(Debug, Clone)]
pub struct Function {
    name: String,
    entry: NodeIndex,
    blocks: Arena<Block>,
    cfg: IndexedGraph<BlockId, EdgeKind>,
}

impl Function {
    pub fn new(
        name: String,
        entry: BlockId,
        blocks: Arena<Block>,
        cfg: IndexedGraph<BlockId, EdgeKind>,
    ) -> Self {
        let entry = cfg
            .node_for_key(entry)
            .expect("entry block must exist in CFG");
        Self {
            name,
            entry,
            blocks,
            cfg,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn entry(&self) -> NodeIndex {
        self.entry
    }

    pub fn block(&self, id: BlockId) -> &Block {
        self.blocks.get(id).expect("block id should be valid")
    }

    pub fn block_mut(&mut self, id: BlockId) -> &mut Block {
        self.blocks.get_mut(id).expect("block id should be valid")
    }

    pub fn blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().map(|(_, block)| block)
    }

    pub fn blocks_mut(&mut self) -> impl Iterator<Item = &mut Block> {
        self.blocks.iter_mut().map(|(_, block)| block)
    }

    pub fn cfg(&self) -> &IndexedGraph<BlockId, EdgeKind> {
        &self.cfg
    }

    pub fn block_for_node(&self, node: NodeIndex) -> BlockId {
        self.cfg
            .key_for_node(node)
            .expect("block node should exist")
    }

    pub fn node_for_block(&self, id: BlockId) -> NodeIndex {
        self.cfg
            .node_for_key(id)
            .expect("block id should exist in CFG")
    }
}

impl VarVisitor for Function {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        for block in self.blocks() {
            block.visit_vars(f);
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        for block in self.blocks_mut() {
            block.rewrite_vars(f);
        }
    }
}

pub type BlockId = Id<Block>;

#[derive(Debug, Clone)]
pub struct Block {
    id: BlockId,
    label: Option<String>,
    instructions: Vec<Instruction>,
    terminator: Terminator,
    start: u64,
    end: u64,
    phis: Vec<PhiFunc>,
}

impl Block {
    pub fn new(
        id: BlockId,
        label: Option<String>,
        instructions: Vec<Instruction>,
        terminator: Terminator,
    ) -> Self {
        let start = instructions
            .first()
            .map(|insn| insn.addr)
            .unwrap_or(terminator.addr);
        let end = terminator.addr;
        Self {
            id,
            label,
            instructions,
            terminator,
            start,
            end,
            phis: Vec::new(),
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

    pub fn instructions_mut(&mut self) -> &mut [Instruction] {
        &mut self.instructions
    }

    pub fn terminator(&self) -> &Terminator {
        &self.terminator
    }

    pub fn terminator_mut(&mut self) -> &mut Terminator {
        &mut self.terminator
    }

    pub fn add_phi(&mut self, phi: PhiFunc) {
        self.phis.push(phi);
    }

    pub fn phi_mut(&mut self) -> &mut [PhiFunc] {
        &mut self.phis
    }
}

impl VarVisitor for Block {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        for insn in &self.instructions {
            insn.visit_vars(f);
        }
        self.terminator.visit_vars(f);
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        for insn in &mut self.instructions {
            insn.rewrite_vars(f);
        }
        self.terminator.rewrite_vars(f);
    }
}

#[derive(Debug, Clone)]
pub struct PhiFunc {
    dst: Var,
    args: BTreeMap<BlockId, Value>,
}

impl PhiFunc {
    pub fn new(dst: Var) -> Self {
        Self {
            dst,
            args: BTreeMap::new(),
        }
    }

    pub fn dst(&self) -> Var {
        self.dst
    }

    pub fn set_dst(&mut self, dst: Var) {
        self.dst = dst;
    }

    pub fn add_arg(&mut self, block: BlockId, value: Value) {
        self.args.insert(block, value);
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

impl VarVisitor for InstructionOrTerminator {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        match self {
            Self::Instruction(insn) => insn.visit_vars(f),
            Self::Terminator(term) => term.visit_vars(f),
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        match self {
            Self::Instruction(insn) => insn.rewrite_vars(f),
            Self::Terminator(term) => term.rewrite_vars(f),
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

    pub fn kind_mut(&mut self) -> &mut InstructionKind {
        &mut self.kind
    }

    // TODO
    pub fn next_addr(&self) -> u64 {
        self.addr() + 4
    }
}

impl VarVisitor for Instruction {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        match &self.kind {
            InstructionKind::Assign { dst, src } => {
                // NB: visit src before dst, so that in case of dst and src sharing the same variable, it is treated as use before def.
                src.visit_vars(f);
                f(VarRole::Def, *dst);
            }
            InstructionKind::Store { dst, src } => {
                dst.visit_vars(f);
                src.visit_vars(f);
            }
            InstructionKind::Call { target } => target.visit_vars(f),
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        match &mut self.kind {
            InstructionKind::Assign { dst, src } => {
                // NB: same reason as visit_vars
                src.rewrite_vars(f);
                *dst = f(VarRole::Def, *dst);
            }
            InstructionKind::Store { dst, src } => {
                dst.rewrite_vars(f);
                src.rewrite_vars(f);
            }
            InstructionKind::Call { target } => target.rewrite_vars(f),
        }
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

impl From<Terminator> for InstructionOrTerminator {
    fn from(term: Terminator) -> Self {
        Self::Terminator(term)
    }
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

impl VarVisitor for Terminator {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        match &self.kind {
            TerminatorKind::Branch { target: _ } => {}
            TerminatorKind::ConditionalBranch { cond, .. } => cond.visit_vars(f),
            TerminatorKind::Ret => {}
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        match &mut self.kind {
            TerminatorKind::Branch { target: _ } => {}
            TerminatorKind::ConditionalBranch { cond, .. } => cond.rewrite_vars(f),
            TerminatorKind::Ret => {}
        }
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
    Gt,
}

impl VarVisitor for BranchCondition {
    fn visit_vars(&self, mut f: &mut impl FnMut(VarRole, Var)) {
        match self {
            Self::NonZero(value) => value.visit_vars(&mut f),
            Self::Ge | Self::Gt => {}
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        match self {
            Self::NonZero(value) => value.rewrite_vars(f),
            Self::Ge | Self::Gt => {}
        }
    }
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

impl VarVisitor for Rhs {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        match self {
            Self::BinOp { lhs, rhs, .. } => {
                lhs.visit_vars(f);
                rhs.visit_vars(f);
            }
            Self::Load { src } => src.visit_vars(f),
            Self::Copy { src } => src.visit_vars(f),
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        match self {
            Self::BinOp { lhs, rhs, .. } => {
                lhs.rewrite_vars(f);
                rhs.rewrite_vars(f);
            }
            Self::Load { src } => src.rewrite_vars(f),
            Self::Copy { src } => src.rewrite_vars(f),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    SDiv,
}

#[derive(Debug, Clone)]
pub enum Value {
    Var(Var),
    Imm(i64),
}

impl VarVisitor for Value {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var)) {
        match self {
            Self::Var(var) => f(VarRole::Use, *var),
            Self::Imm(_) => {}
        }
    }

    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var) {
        match self {
            Self::Var(var) => {
                *var = f(VarRole::Use, *var);
            }
            Self::Imm(_) => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Var {
    place: Place,
    version: u32,
}

impl Var {
    pub fn from_place(place: Place) -> Self {
        Self::with_version(place, 0)
    }

    pub fn with_version(place: Place, version: u32) -> Self {
        Self { place, version }
    }

    pub fn place(&self) -> Place {
        self.place
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn set_version(&mut self, version: u32) {
        self.version = version;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Place {
    Reg(Reg),
    Temp(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarRole {
    Use,
    Def,
}

trait VarVisitor {
    fn visit_vars(&self, f: &mut impl FnMut(VarRole, Var));
    fn rewrite_vars(&mut self, f: &mut impl FnMut(VarRole, Var) -> Var);

    fn uses(&self) -> HashSet<Var> {
        let mut s = HashSet::new();
        let mut defs = HashSet::new();
        self.visit_vars(&mut |r, v| match r {
            VarRole::Use => {
                if !defs.contains(&v.place()) {
                    s.insert(v);
                }
            }
            VarRole::Def => {
                defs.insert(v.place());
            }
        });
        s
    }

    fn defs(&self) -> HashSet<Var> {
        let mut s = HashSet::new();
        self.visit_vars(&mut |r, v| {
            if r == VarRole::Def {
                s.insert(v);
            }
        });
        s
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {} {{", self.name)?;
        let mut blocks: Vec<_> = self.blocks().collect();
        blocks.sort_by_key(|block| block.start());
        for block in blocks {
            writeln!(f, "{block}")?;
        }
        writeln!(f, "}}")
    }
}

impl fmt::Display for Block {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.label {
            Some(label) => writeln!(f, "block{} {label}:", self.id.index())?,
            None => writeln!(f, "block{}:", self.id.index())?,
        };
        for phi in &self.phis {
            writeln!(f, "  {phi}")?;
        }
        for insn in &self.instructions {
            writeln!(f, "  {insn}")?;
        }
        writeln!(f, "  {}", self.terminator)
    }
}

impl fmt::Display for PhiFunc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args = self
            .args
            .iter()
            .map(|(k, v)| format!("block{}:{v}", k.index()))
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "{} = phi {}", self.dst, args)
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
            Self::Gt => write!(f, "> 0"),
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
            Self::Mul => write!(f, "sdiv"),
            Self::SDiv => write!(f, "sdiv"),
        }
    }
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.place, self.version)
    }
}

impl fmt::Display for Place {
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
