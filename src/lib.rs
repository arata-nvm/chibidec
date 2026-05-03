use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    binary::Binary, cfg_recovery::recover_cfg, cfg_structuring::structure_cfg,
    disassemble::disassemble,
};

pub mod binary;
pub mod cfg_recovery;
pub mod cfg_structuring;
pub mod disassemble;
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
    let main_cfg = icfg
        .extract_function_by_label("_main")
        .context("failed to extract main function cfg")?;
    let _structured_cfg = structure_cfg(&main_cfg).context("failed to structure cfg")?;

    // println!(
    //     "{}",
    //     region_cfg_dot(&region_cfg, &region_arena, &block_arena)
    // );

    Ok(())
}
