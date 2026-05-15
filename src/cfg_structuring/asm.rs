use std::fmt::{self, Write};

use id_arena::Arena;

use crate::{
    cfg_structuring::{
        Condition, StructuredCfg, TailKind, VirtualizedEdge,
        cycle::LoopKind,
        region::{Region, RegionId},
    },
    graph::IndexedGraphView,
    llir::{BlockId, Function},
};

pub fn render_structured_llir(structured_cfg: &StructuredCfg, func: &Function) -> String {
    let mut out = String::new();
    write_structured_llir(&mut out, structured_cfg, func)
        .expect("writing to String should not fail");
    out
}

pub fn write_structured_llir(
    out: &mut impl Write,
    structured_cfg: &StructuredCfg,
    func: &Function,
) -> fmt::Result {
    let region_cfg = &structured_cfg.cfg;
    let Some(entry) = region_cfg.entry() else {
        return Ok(());
    };
    if entry == region_cfg.vexit() {
        return Ok(());
    }
    let Some(root) = region_cfg.key_for_node(entry) else {
        return Ok(());
    };

    Renderer::new(out, structured_cfg, func).region(root)
}

struct Renderer<'a, 'w, W: Write> {
    out: &'w mut W,
    regions: &'a Arena<Region>,
    virtualized_edges: &'a [VirtualizedEdge],
    func: &'a Function,
    indent: usize,
}

impl<'a, 'w, W: Write> Renderer<'a, 'w, W> {
    fn new(out: &'w mut W, structured_cfg: &'a StructuredCfg, func: &'a Function) -> Self {
        Self {
            out,
            regions: &structured_cfg.regions,
            virtualized_edges: &structured_cfg.virtualized_edges,
            func,
            indent: 0,
        }
    }

    fn region(&mut self, region_id: RegionId) -> fmt::Result {
        let Some(region) = self.regions.get(region_id) else {
            return Ok(());
        };

        match region {
            Region::Leaf(block) => {
                self.block(*block)?;
                self.virtualized_tail(region_id)
            }
            Region::Seq(seq_regions) => {
                for region in seq_regions {
                    self.region(*region)?;
                }
                Ok(())
            }
            Region::If {
                head,
                then_br,
                else_br,
                cond,
                ..
            } => {
                self.region(*head)?;
                self.line(format_args!("if ({}) {{", display_cond(cond)))?;
                self.indented(|this| {
                    for region in then_br {
                        this.region(*region)?;
                    }
                    Ok(())
                })?;
                if let Some(else_br) = else_br {
                    self.line(format_args!("}} else {{"))?;
                    self.indented(|this| {
                        for region in else_br {
                            this.region(*region)?;
                        }
                        Ok(())
                    })?;
                }
                self.line(format_args!("}}"))
            }
            Region::Loop { kind, meta, body } => {
                let cond = display_cond(&meta.cond);
                match kind {
                    LoopKind::While | LoopKind::NatLoop => {
                        self.line(format_args!("while ({cond}) {{"))?;
                        self.indented(|this| this.region(*body))?;
                        self.line(format_args!("}}"))
                    }
                    LoopKind::DoWhile => {
                        self.line(format_args!("do {{"))?;
                        self.indented(|this| this.region(*body))?;
                        self.line(format_args!("}} while ({cond});"))
                    }
                }
            }
            Region::VirtualExit => Ok(()),
        }
    }

    fn virtualized_tail(&mut self, source: RegionId) -> fmt::Result {
        for edge in self
            .virtualized_edges
            .iter()
            .filter(|edge| edge.source == source)
        {
            match &edge.tail {
                TailKind::Continue => self.line(format_args!("continue;"))?,
                TailKind::Break => self.line(format_args!("break;"))?,
                TailKind::Goto { target } => self.line(format_args!("goto {target:?};"))?,
            }
        }
        Ok(())
    }

    fn block(&mut self, block_id: BlockId) -> fmt::Result {
        let block = self.func.block(block_id);
        for line in block.to_string().lines() {
            self.line(format_args!("{line}"))?;
        }
        Ok(())
    }

    fn indented(&mut self, f: impl FnOnce(&mut Self) -> fmt::Result) -> fmt::Result {
        self.indent += 1;
        let result = f(self);
        self.indent -= 1;
        result
    }

    fn line(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        for _ in 0..self.indent {
            self.out.write_str("    ")?;
        }
        self.out.write_fmt(args)?;
        self.out.write_char('\n')
    }
}

fn display_cond(cond: &Option<Condition>) -> String {
    cond.as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}
