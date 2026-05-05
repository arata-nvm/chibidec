use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    binary::Binary,
    cfg_recovery::recover_cfg,
    cfg_structuring::{asm::render_structured_assembly, structure_cfg},
    disassemble::disassemble,
};

pub mod binary;
pub mod cfg_recovery;
pub mod cfg_structuring;
pub mod disassemble;
pub mod dot;
pub mod graph;

pub fn decompile(binary_path: &Path) -> Result<()> {
    let binary_data = std::fs::read(binary_path).context("failed to read binary file")?;
    let binary = Binary::parse(&binary_data).context("failed to parse binary")?;

    let text_section = binary
        .section_by_name("__text")
        .context("failed to find __text section")?;
    let text_insns = disassemble(text_section.data(), text_section.addr())
        .context("failed to disassemble __text section")?;
    let icfg = recover_cfg(&text_insns, &binary.symbols());
    std::fs::write("tmp/icfg.dot", icfg.dot())?;

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
