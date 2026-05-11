use std::fmt;

pub mod lifter;

#[derive(Debug, Clone)]
pub struct LinearFunction {
    pub name: String,
    pub insns: Vec<InstructionOrTerminator>,
}

#[derive(Debug, Clone)]
pub enum InstructionOrTerminator {
    Instruction(Instruction),
    Terminator(Terminator),
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub addr: u64,
    pub kind: InstructionKind,
}

impl From<Instruction> for InstructionOrTerminator {
    fn from(insn: Instruction) -> Self {
        Self::Instruction(insn)
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
    pub addr: u64,
    pub kind: TerminatorKind,
}

impl From<Terminator> for InstructionOrTerminator {
    fn from(term: Terminator) -> Self {
        Self::Terminator(term)
    }
}

#[derive(Debug, Clone)]
pub enum TerminatorKind {
    Ret,
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

impl fmt::Display for LinearFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {} {{", self.name)?;
        for item in &self.insns {
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
        match &self.kind {
            InstructionKind::Assign { dst, src } => write!(f, "{:#x}: {dst} = {src}", self.addr),
            InstructionKind::Store { dst, src } => {
                write!(f, "{:#x}: store [{dst}], {src}", self.addr)
            }
            InstructionKind::Call { target } => write!(f, "{:#x}: call {target}", self.addr),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TerminatorKind::Ret => write!(f, "{:#x}: ret", self.addr),
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
