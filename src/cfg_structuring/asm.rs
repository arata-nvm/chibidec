use std::fmt::Write;

use crate::{
    cfg_recovery::cfg::{Cfg, Condition},
    cfg_structuring::{
        cycle::LoopKind,
        region::{Region, RegionCfg, RegionId},
    },
    graph::IndexedGraphView,
};

pub fn render_structured_assembly(region_cfg: &RegionCfg, cfg: &Cfg) -> String {
    let mut out = String::new();
    let Some(entry) = region_cfg.entry() else {
        return out;
    };
    if entry == region_cfg.vexit() {
        return out;
    }
    let Some(root) = region_cfg.key_for_node(entry) else {
        return out;
    };

    fmt_region(region_cfg, cfg, root, 0, &mut out);
    out
}

fn fmt_region(
    region_cfg: &RegionCfg,
    cfg: &Cfg,
    region_id: RegionId,
    indent: usize,
    out: &mut String,
) {
    let Some(region) = region_cfg.region(region_id) else {
        return;
    };

    match region {
        Region::Leaf(block_id) => fmt_block(cfg, *block_id, indent, out),
        Region::Seq(regions) => {
            for region in regions {
                fmt_region(region_cfg, cfg, *region, indent, out);
            }
        }
        Region::If {
            head,
            then_br,
            else_br,
            cond,
            ..
        } => {
            fmt_region(region_cfg, cfg, *head, indent, out);
            push_line(out, indent, format_args!("if ({}) {{", display_cond(cond)));
            for region in then_br {
                fmt_region(region_cfg, cfg, *region, indent + 1, out);
            }
            match else_br {
                Some(else_br) => {
                    push_line(out, indent, format_args!("}} else {{"));
                    for region in else_br {
                        fmt_region(region_cfg, cfg, *region, indent + 1, out);
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
                    fmt_region(region_cfg, cfg, *body, indent + 1, out);
                    push_line(out, indent, format_args!("}}"));
                }
                LoopKind::DoWhile => {
                    push_line(out, indent, format_args!("do {{"));
                    fmt_region(region_cfg, cfg, *body, indent + 1, out);
                    push_line(out, indent, format_args!("}} while ({cond});"));
                }
                LoopKind::NatLoop => {
                    push_line(out, indent, format_args!("while ({cond}) {{"));
                    fmt_region(region_cfg, cfg, *body, indent + 1, out);
                    push_line(out, indent, format_args!("}}"));
                }
            }
        }
        Region::VirtualExit => {}
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
