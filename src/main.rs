use std::path::Path;

use anyhow::Result;
use chibidec::decompile;

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("usage: {} <binary_path>", args[0]);
        std::process::exit(1);
    }

    let binary_path = Path::new(&args[1]);
    decompile(binary_path)
}
