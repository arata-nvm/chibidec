use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};

use crate::{
    binary::{Binary, Symbol},
    cfg::{
        BlockId, BlockStore, add_virtual_exit, construct_blocks, construct_graph, dot,
        extract_main, find_block_start_addrs,
    },
};

pub mod binary;
pub mod cfg;
pub mod disassemble;

pub fn decompile(binary_path: &Path) -> Result<()> {
    let binary_data = std::fs::read(binary_path).context("failed to read binary file")?;
    let binary = Binary::parse(&binary_data).context("failed to parse binary")?;

    let text_section = binary
        .section_by_name("__text")
        .context("failed to find __text section")?;
    let text_insns = disassemble::disassemble(&text_section.data, text_section.addr)
        .context("failed to disassemble __text section")?;

    let symbols = binary.symbols();
    let starts = find_block_start_addrs(&text_insns);
    let addr_to_insn: HashMap<_, _> = text_insns
        .into_iter()
        .map(|insn| (insn.addr, insn))
        .collect();

    let mut block_store = BlockStore::new();
    let block_ids = construct_blocks(&mut block_store, &addr_to_insn, &starts)
        .context("failed to construct blocks")?;
    label_blocks(&mut block_store, &block_ids, &symbols);
    let graph =
        construct_graph(&mut block_store, &addr_to_insn).context("failed to construct graph")?;
    let main_graph = extract_main(&graph, &block_store).context("failed to extract main graph")?;
    let (main_graph, _vexit_node) = add_virtual_exit(main_graph, &mut block_store)
        .context("failed to add virtual exit node")?;

    println!("{}", dot(&main_graph, &block_store));

    Ok(())
}

fn label_blocks(block_store: &mut BlockStore, block_ids: &[BlockId], symbols: &[Symbol]) {
    for block_id in block_ids {
        let block = block_store.get_mut(*block_id);
        if let Some(symbol) = symbols.iter().find(|symbol| symbol.addr == block.start) {
            block.label = Some(symbol.name.clone());
        }
    }
}
