use anyhow::{Result, anyhow, bail};
use goblin::{
    Object,
    mach::{Mach, MachO},
};

#[derive(Debug)]
pub struct Binary<'a> {
    binary: MachO<'a>,
}

impl<'a> Binary<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let object = Object::parse(bytes)?;
        let Object::Mach(Mach::Binary(binary)) = object else {
            bail!("unsupported binary format");
        };
        Ok(Self { binary })
    }

    pub fn sections(&self) -> Vec<Section> {
        self.binary
            .segments
            .iter()
            .filter_map(|segment| segment.sections().ok())
            .flatten()
            .map(Section::from)
            .collect()
    }

    pub fn section_by_name(&self, name: &str) -> Result<Section> {
        self.sections()
            .into_iter()
            .find(|section| section.name == name)
            .ok_or_else(|| anyhow!("section not found: {name}"))
    }

    pub fn symbols(&self) -> Vec<Symbol> {
        let mut symbols: Vec<_> = self
            .binary
            .symbols()
            .filter_map(|symbol| symbol.ok())
            .map(Symbol::from)
            .filter(|symbol| symbol.name != "__mh_execute_header" && symbol.addr != 0)
            .collect();
        symbols.sort_by_key(|symbol| symbol.addr);
        symbols
    }
}

#[derive(Debug)]
pub struct Section {
    addr: u64,
    name: String,
    data: Vec<u8>,
}

impl Section {
    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

impl From<(goblin::mach::segment::Section, &[u8])> for Section {
    fn from((section, data): (goblin::mach::segment::Section, &[u8])) -> Self {
        Self {
            addr: section.addr,
            name: section.name().unwrap_or_default().to_string(),
            data: data.to_vec(),
        }
    }
}

#[derive(Debug)]
pub struct Symbol {
    addr: u64,
    name: String,
}

impl Symbol {
    pub fn addr(&self) -> u64 {
        self.addr
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl From<(&str, goblin::mach::symbols::Nlist)> for Symbol {
    fn from((name, symbol): (&str, goblin::mach::symbols::Nlist)) -> Self {
        Self {
            addr: symbol.n_value,
            name: name.to_string(),
        }
    }
}
