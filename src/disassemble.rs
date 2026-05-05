use std::{collections::HashMap, fmt};

use anyhow::{Context, Result, bail};
use capstone::{
    Capstone,
    arch::{
        self, ArchDetail, BuildsCapstone, DetailsArchInsn,
        arm64::{Arm64CC, Arm64Insn, Arm64InsnDetail, Arm64OperandType},
    },
};

pub fn disassemble(code: &[u8], addr: u64) -> Result<InstructionSequence> {
    let cs = Capstone::new()
        .arm64()
        .mode(arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .context("failed to build capstone")?;
    let cs_insns = cs
        .disasm_all(code, addr)
        .context("failed to disassemble code")?;
    let insns = cs_insns
        .iter()
        .map(|insn| Instruction::from_insn(&cs, insn))
        .collect::<Result<_, _>>()?;
    Ok(InstructionSequence::new(insns))
}

#[derive(Debug)]
pub struct InstructionSequence {
    insns: Vec<Instruction>,
    addr_to_index: HashMap<u64, usize>,
}

impl InstructionSequence {
    pub fn new(insns: Vec<Instruction>) -> Self {
        let addr_to_index = insns
            .iter()
            .enumerate()
            .map(|(i, insn)| (insn.addr(), i))
            .collect();
        Self {
            insns,
            addr_to_index,
        }
    }

    pub fn get(&self, addr: u64) -> Option<&Instruction> {
        self.addr_to_index.get(&addr).map(|&i| &self.insns[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &Instruction> {
        self.insns.iter()
    }

    pub fn fallthrough_insn(&self, insn: &Instruction) -> Option<&Instruction> {
        self.get(insn.next_addr())
    }
}

#[derive(Debug, Clone)]
pub struct Instruction {
    addr: u64,
    len: usize,
    mnemonic: String,
    op_str: String,
    kind: InstructionKind,
}

#[derive(Debug, Clone)]
pub enum InstructionKind {
    ConditionalBranch { target: u64, kind: ConditionKind },
    UnconditionalBranch { target: u64 },
    Call { target: u64 },
    Ret,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionKind {
    NonZero,
    NotEqual,
    Greater,
    GreaterOrEqual,
    LessOrEqual,
}

impl Instruction {
    fn from_insn(cs: &Capstone, insn: &capstone::Insn) -> Result<Self> {
        let detail = cs.insn_detail(insn)?;
        let ArchDetail::Arm64Detail(detail) = detail.arch_detail() else {
            bail!("unsupported architecture");
        };

        let insn_id = Arm64Insn::from(insn.id().0);
        let kind = InstructionKind::from_insn(insn_id, detail)?;

        Ok(Self {
            addr: insn.address(),
            len: insn.len(),
            mnemonic: insn.mnemonic().unwrap_or("???").to_string(),
            op_str: insn.op_str().unwrap_or("").to_string(),
            kind,
        })
    }

    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn mnemonic(&self) -> &str {
        &self.mnemonic
    }

    pub fn op_str(&self) -> &str {
        &self.op_str
    }

    fn next_addr(&self) -> u64 {
        self.addr + self.len as u64
    }

    pub fn branch_target(&self) -> Option<u64> {
        match self.kind {
            InstructionKind::ConditionalBranch { target, .. }
            | InstructionKind::UnconditionalBranch { target }
            | InstructionKind::Call { target } => Some(target),
            _ => None,
        }
    }

    pub fn is_conditional_branch(&self) -> bool {
        matches!(self.kind, InstructionKind::ConditionalBranch { .. })
    }

    pub fn conditional_branch_target(&self) -> Option<u64> {
        match self.kind {
            InstructionKind::ConditionalBranch { target, .. } => Some(target),
            _ => None,
        }
    }

    pub fn conditional_branch_kind(&self) -> Option<ConditionKind> {
        match self.kind {
            InstructionKind::ConditionalBranch { kind, .. } => Some(kind),
            _ => None,
        }
    }

    pub fn is_unconditional_branch(&self) -> bool {
        matches!(self.kind, InstructionKind::UnconditionalBranch { .. })
    }

    pub fn unconditional_branch_target(&self) -> Option<u64> {
        match self.kind {
            InstructionKind::UnconditionalBranch { target } => Some(target),
            _ => None,
        }
    }

    pub fn is_call(&self) -> bool {
        matches!(self.kind, InstructionKind::Call { .. })
    }

    pub fn call_target(&self) -> Option<u64> {
        match self.kind {
            InstructionKind::Call { target } => Some(target),
            _ => None,
        }
    }

    pub fn is_ret(&self) -> bool {
        matches!(self.kind, InstructionKind::Ret)
    }

    pub fn is_terminator(&self) -> bool {
        matches!(
            self.kind,
            InstructionKind::ConditionalBranch { .. }
                | InstructionKind::UnconditionalBranch { .. }
                | InstructionKind::Ret
        )
    }
}

impl InstructionKind {
    fn from_insn(insn: Arm64Insn, detail: Arm64InsnDetail) -> Result<Self> {
        let imm = detail.operands().find_map(|opr| match opr.op_type {
            Arm64OperandType::Imm(imm) => Some(imm as u64),
            _ => None,
        });

        match insn {
            Arm64Insn::ARM64_INS_CBNZ => {
                let Some(target) = imm else {
                    bail!("CBNZ instruction without immediate operand");
                };
                Ok(Self::ConditionalBranch {
                    target,
                    kind: ConditionKind::NonZero,
                })
            }
            Arm64Insn::ARM64_INS_B => {
                let Some(target) = imm else {
                    bail!("B instruction without immediate operand");
                };
                match detail.cc() {
                    Arm64CC::ARM64_CC_INVALID => Ok(Self::UnconditionalBranch { target }),
                    Arm64CC::ARM64_CC_NE => Ok(Self::ConditionalBranch {
                        target,
                        kind: ConditionKind::NotEqual,
                    }),
                    Arm64CC::ARM64_CC_GT => Ok(Self::ConditionalBranch {
                        target,
                        kind: ConditionKind::Greater,
                    }),
                    Arm64CC::ARM64_CC_GE => Ok(Self::ConditionalBranch {
                        target,
                        kind: ConditionKind::GreaterOrEqual,
                    }),
                    Arm64CC::ARM64_CC_LE => Ok(Self::ConditionalBranch {
                        target,
                        kind: ConditionKind::LessOrEqual,
                    }),
                    _ => bail!(
                        "unsupported condition code for B instruction: {:?}",
                        detail.cc()
                    ),
                }
            }
            Arm64Insn::ARM64_INS_BL if let Some(target) = imm => Ok(Self::Call { target }),
            Arm64Insn::ARM64_INS_RET => Ok(Self::Ret),
            _ => Ok(Self::Other),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#x}: {} {}", self.addr, self.mnemonic, self.op_str)
    }
}
