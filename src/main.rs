use anyhow::{Context, Result};
use chibidec::{binary::Binary, disassemble};

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: {} <binary_path>", args[0]);
        std::process::exit(1);
    }

    let binary_path = &args[1];
    let binary_data = std::fs::read(binary_path).context("failed to read binary file")?;
    let binary = Binary::parse(&binary_data).context("failed to parse binary")?;

    println!("{:x?}", binary.sections());
    println!("{:x?}", binary.symbols());

    let text_section = binary
        .section_by_name("__text")
        .context("failed to find __text section")?;

    let text_insns = disassemble::disassemble(&text_section.data, text_section.addr)
        .context("failed to disassemble __text section")?;
    for insn in text_insns {
        println!("{:x?}", insn);
    }

    Ok(())
}
