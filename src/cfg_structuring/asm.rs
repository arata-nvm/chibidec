use std::fmt::Write;

use crate::{
    cfg_recovery::cfg::{Cfg, Condition, EdgeLabel, TailKind},
    cfg_structuring::{
        StructuredCfg, VirtualizedEdge,
        cycle::LoopKind,
        region::{Region, RegionId, RegionStore},
    },
    graph::IndexedGraphView,
};

pub fn render_structured_assembly(structured_cfg: &StructuredCfg, cfg: &Cfg) -> String {
    let mut out = String::new();
    let region_cfg = &structured_cfg.cfg;
    let Some(entry) = region_cfg.entry() else {
        return out;
    };
    if entry == region_cfg.vexit() {
        return out;
    }
    let Some(root) = region_cfg.key_for_node(entry) else {
        return out;
    };

    fmt_region(
        &structured_cfg.regions,
        &structured_cfg.virtualized_edges,
        cfg,
        root,
        0,
        &mut out,
    );
    out
}

fn fmt_region(
    regions: &RegionStore,
    virtualized_edges: &[VirtualizedEdge],
    cfg: &Cfg,
    region_id: RegionId,
    indent: usize,
    out: &mut String,
) {
    let Some(region) = regions.get(region_id) else {
        return;
    };

    match region {
        Region::Leaf(block_id) => {
            fmt_block(cfg, *block_id, indent, out);
            fmt_virtualized_tail(virtualized_edges, region_id, indent, out);
        }
        Region::Seq(seq_regions) => {
            for region in seq_regions {
                fmt_region(regions, virtualized_edges, cfg, *region, indent, out);
            }
        }
        Region::If {
            head,
            then_br,
            else_br,
            cond,
            ..
        } => {
            fmt_region(regions, virtualized_edges, cfg, *head, indent, out);
            push_line(out, indent, format_args!("if ({}) {{", display_cond(cond)));
            for region in then_br {
                fmt_region(regions, virtualized_edges, cfg, *region, indent + 1, out);
            }
            match else_br {
                Some(else_br) => {
                    push_line(out, indent, format_args!("}} else {{"));
                    for region in else_br {
                        fmt_region(regions, virtualized_edges, cfg, *region, indent + 1, out);
                    }
                    push_line(out, indent, format_args!("}}"));
                }
                None => push_line(out, indent, format_args!("}}")),
            }
        }
        Region::Loop { kind, meta, body } => {
            let cond = display_cond(&meta.cond);
            match kind {
                LoopKind::While => {
                    push_line(out, indent, format_args!("while ({cond}) {{"));
                    fmt_region(regions, virtualized_edges, cfg, *body, indent + 1, out);
                    push_line(out, indent, format_args!("}}"));
                }
                LoopKind::DoWhile => {
                    push_line(out, indent, format_args!("do {{"));
                    fmt_region(regions, virtualized_edges, cfg, *body, indent + 1, out);
                    push_line(out, indent, format_args!("}} while ({cond});"));
                }
                LoopKind::NatLoop => {
                    push_line(out, indent, format_args!("while ({cond}) {{"));
                    fmt_region(regions, virtualized_edges, cfg, *body, indent + 1, out);
                    push_line(out, indent, format_args!("}}"));
                }
            }
        }
        Region::VirtualExit => {}
    }
}

fn fmt_virtualized_tail(
    virtualized_edges: &[VirtualizedEdge],
    source: RegionId,
    indent: usize,
    out: &mut String,
) {
    for edge in virtualized_edges
        .iter()
        .filter(|edge| edge.source == source)
    {
        let EdgeLabel::Virtualized(tail) = &edge.label else {
            continue;
        };
        match tail {
            TailKind::Continue => push_line(out, indent, format_args!("continue;")),
            TailKind::Break => push_line(out, indent, format_args!("break;")),
            TailKind::Goto { target } => {
                push_line(out, indent, format_args!("goto {target:?};"));
            }
        }
    }
}

fn fmt_block(
    cfg: &Cfg,
    block_id: crate::cfg_recovery::cfg::BlockId,
    indent: usize,
    out: &mut String,
) {
    let Some(block) = cfg.block(block_id) else {
        return;
    };

    push_line(
        out,
        indent,
        format_args!("// {block_id:?} [{:#x}-{:#x}]", block.start(), block.end()),
    );
    for insn in block.instructions() {
        push_line(out, indent, format_args!("{insn}"));
    }
}

fn display_cond(cond: &Option<Condition>) -> String {
    cond.as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(|| "unknown".to_string())
}

fn push_line(out: &mut String, indent: usize, args: std::fmt::Arguments<'_>) {
    for _ in 0..indent {
        out.push_str("    ");
    }
    let _ = out.write_fmt(args);
    out.push('\n');
}
