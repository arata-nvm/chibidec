use anyhow::{Context, Result, bail};
use capstone::{
    Capstone,
    arch::{self, ArchDetail, BuildsCapstone, DetailsArchInsn, arm64::Arm64OperandType},
};

#[derive(Debug)]
pub struct Instruction {
    pub cs_id: u32,
    pub addr: u64,
    pub len: usize,
    pub mnemonic: String,
    pub operands: Option<Vec<String>>,
    pub imm: Option<u64>,
}

impl Instruction {
    pub fn from_insn(cs: &Capstone, insn: &capstone::Insn) -> Result<Self> {
        let detail = cs.insn_detail(insn)?;
        let ArchDetail::Arm64Detail(detail) = detail.arch_detail() else {
            bail!("unsupported architecture");
        };

        let operands: Vec<_> = detail
            .operands()
            .filter_map(|opr| match opr.op_type {
                Arm64OperandType::Reg(reg_id) => cs.reg_name(reg_id),
                _ => None,
            })
            .collect();

        let imm = detail.operands().find_map(|opr| match opr.op_type {
            Arm64OperandType::Imm(imm) => Some(imm as u64),
            _ => None,
        });

        Ok(Self {
            cs_id: insn.id().0,
            addr: insn.address(),
            len: insn.len(),
            mnemonic: insn.mnemonic().unwrap_or("???").to_string(),
            operands: if !operands.is_empty() {
                Some(operands)
            } else {
                None
            },
            imm,
        })
    }
}

pub fn disassemble(code: &[u8], addr: u64) -> Result<Vec<Instruction>> {
    let cs = Capstone::new()
        .arm64()
        .mode(arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .context("failed to build capstone")?;
    let cs_insns = cs
        .disasm_all(code, addr)
        .context("failed to disassemble code")?;
    cs_insns
        .iter()
        .map(|insn| Instruction::from_insn(&cs, insn))
        .collect()
}
