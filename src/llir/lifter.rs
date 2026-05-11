use capstone::{
    RegId,
    arch::arm64::{Arm64Insn, Arm64Operand, Arm64OperandType, Arm64Reg},
};

use crate::{
    disassemble::MachineInst,
    llir::{
        BinOp, Instruction, InstructionKind, InstructionOrTerminator, LinearFunction, Reg, Rhs,
        Terminator, TerminatorKind, Value, Var,
    },
};

pub fn lift_linear_function(name: impl Into<String>, minsns: &[MachineInst]) -> LinearFunction {
    let mut ctx = LifterCtx::new();
    let insns = minsns
        .iter()
        .flat_map(|insn| lift_inst(&mut ctx, insn))
        .collect();

    LinearFunction {
        name: name.into(),
        insns,
    }
}

#[derive(Debug, Default)]
struct LifterCtx {
    next_temp: u32,
}

impl LifterCtx {
    fn new() -> Self {
        Self::default()
    }

    fn new_temp(&mut self) -> Var {
        let temp = Var::Temp(self.next_temp);
        self.next_temp += 1;
        temp
    }
}

fn lift_inst(ctx: &mut LifterCtx, minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    match minsn.opcode {
        Arm64Insn::ARM64_INS_ADD => lift_binop(minsn, BinOp::Add),
        Arm64Insn::ARM64_INS_ADRP => lift_adrp(minsn),
        Arm64Insn::ARM64_INS_BL => lift_bl(minsn),
        Arm64Insn::ARM64_INS_LDP => lift_ldp(ctx, minsn),
        Arm64Insn::ARM64_INS_LDR => lift_ldr(ctx, minsn),
        Arm64Insn::ARM64_INS_MOV => lift_mov(minsn),
        Arm64Insn::ARM64_INS_RET => lift_ret(minsn),
        Arm64Insn::ARM64_INS_STP => lift_stp(ctx, minsn),
        Arm64Insn::ARM64_INS_STR | Arm64Insn::ARM64_INS_STUR => lift_str(ctx, minsn),
        Arm64Insn::ARM64_INS_SUB => lift_binop(minsn, BinOp::Sub),
        _ => unimplemented!(
            "unsupported instruction at {:#x}: {} {}",
            minsn.addr,
            minsn.mnemonic,
            minsn.op_str
        ),
    }
}

fn lift_binop(minsn: &MachineInst, op: BinOp) -> Vec<InstructionOrTerminator> {
    vec![insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: Var::Reg(opr_reg(minsn, 0)),
            src: Rhs::BinOp {
                op,
                lhs: Value::Var(Var::Reg(opr_reg(minsn, 1))),
                rhs: Value::Imm(opr_imm(minsn, 2)),
            },
        },
    )]
}

fn lift_adrp(minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    vec![insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: Var::Reg(opr_reg(minsn, 0)),
            src: Rhs::Copy {
                src: Value::Imm(opr_imm(minsn, 1)),
            },
        },
    )]
}

fn lift_bl(minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    vec![insn(
        minsn.addr,
        InstructionKind::Call {
            target: Value::Imm(opr_imm(minsn, 0)),
        },
    )]
}

fn lift_ldp(ctx: &mut LifterCtx, minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    reject_writeback(minsn);
    let first = opr_reg(minsn, 0);
    let second = opr_reg(minsn, 1);
    let (first_addr_insns, first_addr) = opr_mem(ctx, minsn, 2, 0);
    let (mut second_addr_insns, second_addr) = opr_mem(ctx, minsn, 2, first.byte_width());

    let mut insns = first_addr_insns;
    insns.push(insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: Var::Reg(first),
            src: Rhs::Load { src: first_addr },
        },
    ));
    insns.append(&mut second_addr_insns);
    insns.push(insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: Var::Reg(second),
            src: Rhs::Load { src: second_addr },
        },
    ));
    insns
}

fn lift_mov(minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    let src = match &operand(minsn, 1).op_type {
        Arm64OperandType::Reg(reg) => Value::Var(Var::Reg((*reg).into())),
        Arm64OperandType::Imm(imm) => Value::Imm(*imm),
        _ => panic!(
            "unsupported mov operand at {:#x}: {} {}",
            minsn.addr, minsn.mnemonic, minsn.op_str
        ),
    };

    vec![insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: Var::Reg(opr_reg(minsn, 0)),
            src: Rhs::Copy { src },
        },
    )]
}

fn lift_ret(minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    vec![term(minsn.addr, TerminatorKind::Ret)]
}

fn lift_str(ctx: &mut LifterCtx, minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    reject_writeback(minsn);
    let (mut addr_insns, addr) = opr_mem(ctx, minsn, 1, 0);
    addr_insns.push(insn(
        minsn.addr,
        InstructionKind::Store {
            dst: addr,
            src: Value::Var(Var::Reg(opr_reg(minsn, 0))),
        },
    ));
    addr_insns
}

fn lift_ldr(ctx: &mut LifterCtx, minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    reject_writeback(minsn);
    let (mut addr_insns, addr) = opr_mem(ctx, minsn, 1, 0);
    addr_insns.push(insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: Var::Reg(opr_reg(minsn, 0)),
            src: Rhs::Load { src: addr },
        },
    ));
    addr_insns
}

fn lift_stp(ctx: &mut LifterCtx, minsn: &MachineInst) -> Vec<InstructionOrTerminator> {
    reject_writeback(minsn);
    let first = opr_reg(minsn, 0);
    let second = opr_reg(minsn, 1);
    let (first_addr_insns, first_addr) = opr_mem(ctx, minsn, 2, 0);
    let (mut second_addr_insns, second_addr) = opr_mem(ctx, minsn, 2, first.byte_width());

    let mut insns = first_addr_insns;
    insns.push(insn(
        minsn.addr,
        InstructionKind::Store {
            dst: first_addr,
            src: Value::Var(Var::Reg(first)),
        },
    ));
    insns.append(&mut second_addr_insns);
    insns.push(insn(
        minsn.addr,
        InstructionKind::Store {
            dst: second_addr,
            src: Value::Var(Var::Reg(second)),
        },
    ));
    insns
}

fn opr_mem(
    ctx: &mut LifterCtx,
    minsn: &MachineInst,
    idx: usize,
    extra_offset: i64,
) -> (Vec<InstructionOrTerminator>, Value) {
    let Arm64OperandType::Mem(mem) = &operand(minsn, idx).op_type else {
        panic!(
            "expected memory operand {idx} at {:#x}: {} {}",
            minsn.addr, minsn.mnemonic, minsn.op_str
        );
    };
    assert!(
        mem.index().0 == Arm64Reg::ARM64_REG_INVALID as u16,
        "unsupported scaled indexed memory operand at {:#x}: {} {}",
        minsn.addr,
        minsn.mnemonic,
        minsn.op_str
    );

    let temp = ctx.new_temp();
    let addr_insn = insn(
        minsn.addr,
        InstructionKind::Assign {
            dst: temp,
            src: Rhs::BinOp {
                op: BinOp::Add,
                lhs: Value::Var(Var::Reg(mem.base().into())),
                rhs: Value::Imm(mem.disp() as i64 + extra_offset),
            },
        },
    );
    (vec![addr_insn], Value::Var(temp))
}

fn reject_writeback(minsn: &MachineInst) {
    assert!(
        !minsn.writeback,
        "unsupported writeback addressing at {:#x}: {} {}",
        minsn.addr, minsn.mnemonic, minsn.op_str
    );
}

fn insn(addr: u64, kind: InstructionKind) -> InstructionOrTerminator {
    InstructionOrTerminator::Instruction(Instruction { addr, kind })
}

fn term(addr: u64, kind: TerminatorKind) -> InstructionOrTerminator {
    InstructionOrTerminator::Terminator(Terminator { addr, kind })
}

fn operand(minsn: &MachineInst, idx: usize) -> &Arm64Operand {
    match minsn.operands.get(idx) {
        Some(opr) => opr,
        None => {
            panic!(
                "missing operand {idx} at {:#x}: {} {}",
                minsn.addr, minsn.mnemonic, minsn.op_str
            )
        }
    }
}

fn opr_imm(minsn: &MachineInst, idx: usize) -> i64 {
    let Arm64OperandType::Imm(imm) = operand(minsn, idx).op_type else {
        panic!(
            "expected immediate operand {idx} at {:#x}: {} {}",
            minsn.addr, minsn.mnemonic, minsn.op_str
        );
    };
    imm
}

fn opr_reg(minsn: &MachineInst, idx: usize) -> Reg {
    let Arm64OperandType::Reg(reg) = operand(minsn, idx).op_type else {
        panic!(
            "expected register operand {idx} at {:#x}: {} {}",
            minsn.addr, minsn.mnemonic, minsn.op_str
        );
    };
    reg.into()
}

impl From<RegId> for Reg {
    fn from(reg: RegId) -> Self {
        let w0 = Arm64Reg::ARM64_REG_W0;
        let x0 = Arm64Reg::ARM64_REG_X0;

        match reg.0 as u32 {
            Arm64Reg::ARM64_REG_SP | Arm64Reg::ARM64_REG_WSP => Reg::SP,
            Arm64Reg::ARM64_REG_WZR => Reg::WZR,
            Arm64Reg::ARM64_REG_XZR => Reg::XZR,
            Arm64Reg::ARM64_REG_FP => Reg::X(29),
            Arm64Reg::ARM64_REG_LR => Reg::X(30),
            w if (w0..=Arm64Reg::ARM64_REG_W30).contains(&w) => Reg::W((w - w0) as u8),
            x if (x0..=Arm64Reg::ARM64_REG_X28).contains(&x) => Reg::X((x - x0) as u8),
            r => unimplemented!("unsupported register {:#x}", r),
        }
    }
}
