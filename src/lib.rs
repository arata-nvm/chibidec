use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use petgraph::{
    algo::dominators::{self},
    visit::DfsPostOrder,
};

use crate::{
    binary::{Binary, Symbol},
    cfg::{
        BlockId, BlockStore, add_virtual_exit, construct_blocks, construct_graph, dot,
        extract_main, find_block_start_addrs, find_entry_node, has_backedge,
    },
    region::{Region, RegionArena, match_acyclic},
};

pub mod binary;
pub mod cfg;
pub mod disassemble;
pub mod region;

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
    let (mut main_graph, vexit) = add_virtual_exit(main_graph, &mut block_store)
        .context("failed to add virtual exit node")?;

    println!("{}", dot(&main_graph, &block_store));

    let mut region_arena = RegionArena::new();
    let mut block_to_region = HashMap::new();
    for n in main_graph.node_indices() {
        let block_id = main_graph.node_weight(n).unwrap();
        let region_id = region_arena.alloc(Region::Leaf(*block_id));
        block_to_region.insert(*block_id, region_id);
    }

    let mut seq_count = 0;
    loop {
        if main_graph.node_count() <= 1 {
            break;
        }

        let mut progress = false;
        let entry = find_entry_node(&main_graph).context("failed to find entry node")?;

        let mut order = DfsPostOrder::new(&main_graph, entry);
        let dom = dominators::simple_fast(&main_graph, entry);

        while let Some(head) = order.next(&main_graph) {
            if has_backedge(&main_graph, head, vexit, &dom) {
                eprintln!("cycle discovered: {}", head.index());
            } else {
                progress = match_acyclic(
                    &mut main_graph,
                    head,
                    vexit,
                    &mut seq_count,
                    &mut block_store,
                    &mut region_arena,
                    &mut block_to_region,
                    &dom,
                );
                if progress {
                    break;
                }
            }
        }

        if !progress {
            break;
        }
    }

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
