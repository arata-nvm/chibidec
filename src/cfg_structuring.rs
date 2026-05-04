pub mod if_then;
pub mod region;
pub mod seq;

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use petgraph::{
    Direction,
    algo::dominators::{self, Dominators},
    graph::NodeIndex,
    visit::{DfsPostOrder, EdgeRef, IntoEdgeReferences, Reversed},
};

use crate::{
    cfg_recovery::cfg::{Cfg, EdgeLabel},
    cfg_structuring::{
        if_then::{match_if_then, match_if_then_else},
        region::{Region, RegionCfg},
        seq::match_seq,
    },
    graph::{IndexedGraphView, IndexedGraphViewMut},
};

pub fn structure_cfg(cfg: &Cfg) -> Result<RegionCfg> {
    let mut region_cfg = build_region_cfg(cfg).context("failed to create region cfg")?;

    let mut count = 0;
    while region_cfg.graph().node_count() > 2 {
        std::fs::write(format!("tmp/region_cfg_{}.dot", count), region_cfg.dot())
            .expect("failed to write region cfg dot file");
        let entry = region_cfg.entry().context("failed to find entry region")?;
        let dom = dominators::simple_fast(region_cfg.graph(), entry);
        let pdom = dominators::simple_fast(Reversed(region_cfg.graph()), region_cfg.vexit());

        let mut progress = false;
        let mut order = DfsPostOrder::new(region_cfg.graph(), entry);
        while let Some(head) = order.next(region_cfg.graph()) {
            if region_cfg.has_backedge(head, &dom) {
                eprintln!("cycle discovered: {}", head.index());
            } else {
                progress = match_acyclic(&mut region_cfg, &dom, &pdom, head);
                if progress {
                    count += 1;
                    break;
                }
            }
        }

        if !progress {
            break;
        }
    }

    Ok(region_cfg)
}

fn build_region_cfg(cfg: &Cfg) -> Result<RegionCfg> {
    let mut region_cfg =
        RegionCfg::with_capacity(cfg.graph().node_count() + 1, cfg.graph().edge_count() + 1);

    // 基本ブロックを領域に変換する
    let mut node_map = HashMap::new();
    for cfg_node in cfg.graph().node_indices() {
        let block = cfg
            .key_for_node(cfg_node)
            .expect("function cfg node without block id");
        let (_, region_node) = region_cfg.add_region(Region::Leaf(block));
        node_map.insert(cfg_node, region_node);
    }
    for cfg_edge in cfg.graph().edge_references() {
        let source = node_map
            .get(&cfg_edge.source())
            .expect("missing region node for source block");
        let target = node_map
            .get(&cfg_edge.target())
            .expect("missing region node for target block");
        region_cfg
            .graph_mut()
            .add_edge(*source, *target, cfg_edge.weight().clone());
    }

    // 出口から仮想的な出口ノードへのエッジを追加し、単一の出口を持つようにする
    let exit_nodes: Vec<_> = region_cfg.graph().externals(Direction::Outgoing).collect();
    if exit_nodes.is_empty() {
        bail!("cfg has no exit nodes");
    }
    let (_, vexit) = region_cfg.add_region(Region::VirtualExit);
    region_cfg.set_vexit(vexit);

    for exit_node in exit_nodes {
        region_cfg
            .graph_mut()
            .add_edge(exit_node, vexit, EdgeLabel::Virtualized);
    }

    Ok(region_cfg)
}

fn match_acyclic(
    cfg: &mut RegionCfg,
    dom: &Dominators<NodeIndex>,
    pdom: &Dominators<NodeIndex>,
    head: NodeIndex,
) -> bool {
    match_seq(cfg, head)
        || match_if_then_else(cfg, dom, pdom, head)
        || match_if_then(cfg, dom, pdom, head)
}
