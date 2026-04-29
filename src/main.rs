use anyhow::{Context, Result};
use chibidec::binary::Binary;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <binary_path>", args[0]);
        std::process::exit(1);
    }

    let binary_path = &args[1];
    let binary_data = std::fs::read(binary_path).context("Failed to read binary file")?;
    let binary = Binary::parse(&binary_data).context("Failed to parse binary")?;

    println!("{:x?}", binary.sections());
    println!("{:x?}", binary.symbols());

    Ok(())
}
