use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    binary::Binary,
    cfg_recovery::recover_cfg,
    cfg_structuring::{asm::render_structured_assembly, structure_cfg},
    disassemble::{disassemble, disassemble_detailed},
    llir::{cfg_recovery::recover_icfg as recover_llir_icfg, lifter::lift_text},
};

pub mod binary;
pub mod cfg_recovery;
pub mod cfg_structuring;
pub mod disassemble;
pub mod dot;
pub mod graph;
pub mod llir;

pub fn decompile(binary_path: &Path) -> Result<()> {
    let binary_data = std::fs::read(binary_path).context("failed to read binary file")?;
    let binary = Binary::parse(&binary_data).context("failed to parse binary")?;
    std::fs::create_dir_all("tmp").context("failed to create tmp directory")?;

    let text_section = binary
        .section_by_name("__text")
        .context("failed to find __text section")?;
    let text_insns = disassemble(text_section.data(), text_section.addr())
        .context("failed to disassemble __text section")?;
    let icfg = recover_cfg(&text_insns, &binary.symbols());
    std::fs::write("tmp/icfg.dot", icfg.dot())?;

    let detailed_text_insns = disassemble_detailed(text_section.data(), text_section.addr())
        .context("failed to disassemble __text section with details")?;
    let llir = lift_text(&detailed_text_insns);
    std::fs::write("tmp/llir.txt", llir.to_string()).context("failed to write tmp/llir.txt")?;
    let llir_icfg = recover_llir_icfg(&llir, &binary.symbols());
    std::fs::write("tmp/llir_icfg.dot", llir_icfg.dot())
        .context("failed to write tmp/llir_icfg.dot")?;
    let main_llir = llir_icfg
        .extract_function_by_label("_main")
        .context("failed to extract main LLIR function")?;
    std::fs::write("tmp/main.llir.txt", main_llir.to_string())
        .context("failed to write tmp/main.llir.txt")?;

    let main_cfg = icfg
        .extract_function_by_label("_main")
        .context("failed to extract main function cfg")?;
    std::fs::write("tmp/cfg.dot", main_cfg.dot())?;

    let structured_cfg = structure_cfg(&main_cfg).context("failed to structure cfg")?;
    std::fs::write(
        "tmp/structured_cfg.dot",
        structured_cfg.cfg.dot(&structured_cfg.regions),
    )?;
    print!("{}", render_structured_assembly(&structured_cfg, &main_cfg));

    Ok(())
}
