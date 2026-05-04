use id_arena::Arena;
use petgraph::{
    dot::{Config, Dot},
    prelude::StableGraph,
    stable_graph::EdgeReference,
};

use crate::{
    cfg_recovery::cfg::{Block, BlockId, EdgeLabel},
    cfg_structuring::region::{Region, RegionId},
};

pub(crate) fn export_cfg_to_dot(
    blocks: &Arena<Block>,
    graph: &StableGraph<BlockId, EdgeLabel>,
) -> String {
    let get_edge_attributes = |_, edge: EdgeReference<'_, EdgeLabel>| match edge.weight().color() {
        Some(color) => format!(r#"color = "{color}""#),
        None => String::new(),
    };
    let get_node_attributes = |_, (_, &block_id)| {
        let block = blocks.get(block_id).expect("block_id must be valid");
        let mut lines = vec![
            format!(
                "{block_id:?}({:?}) [{:#x}-{:#x}]",
                block.label().unwrap_or_default(),
                block.start(),
                block.end()
            ),
            "insns:".to_string(),
        ];
        lines.extend(block.instructions().iter().map(|insn| format!("  {insn}")));
        let label = lines.join("\n");
        format!(r#"label = "{}""#, escape_dot_label(label))
    };
    let dot = Dot::with_attr_getters(
        graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &get_edge_attributes,
        &get_node_attributes,
    );
    format!("{dot:?}")
}

pub(crate) fn export_region_cfg_to_dot(
    regions: &Arena<Region>,
    graph: &StableGraph<RegionId, EdgeLabel>,
) -> String {
    let get_edge_attributes = |_, edge: EdgeReference<'_, EdgeLabel>| match edge.weight().color() {
        Some(color) => format!(r#"color = "{color}""#),
        None => String::new(),
    };
    let get_node_attributes = |_, (_, &region_id)| {
        let region = regions.get(region_id).expect("region_id must be valid");
        let label = format!("{region_id:?}\n{region:?}");
        format!(r#"label = "{}""#, escape_dot_label(label))
    };
    let dot = Dot::with_attr_getters(
        graph,
        &[Config::EdgeNoLabel, Config::NodeNoLabel],
        &get_edge_attributes,
        &get_node_attributes,
    );
    format!("{dot:?}")
}

pub(crate) fn escape_dot_label(label: String) -> String {
    label
        .replace('\\', r"\\")
        .replace('"', r#"\""#)
        .replace('\n', r"\l")
        + r"\l"
}
