pub mod cycle;
pub mod if_then;
pub mod region;
pub mod seq;

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use petgraph::{
    graph::NodeIndex,
    visit::{DfsPostOrder, EdgeRef, IntoEdgeReferences},
};

use crate::{
    cfg_recovery::cfg::Cfg,
    cfg_structuring::{
        cycle::{contract_cyclic_region, find_loop, find_smallest_loop},
        if_then::match_if,
        region::{Region, RegionCfg, RegionDominance},
        seq::match_seq,
    },
    graph::{IndexedGraphView, IndexedGraphViewMut},
};

pub fn structure_cfg(cfg: &Cfg) -> Result<RegionCfg> {
    let mut region_cfg = build_region_cfg(cfg).context("failed to create region cfg")?;

    let mut count = 0;
    let mut virtualized_edges = Vec::new();
    let mut strucutured_loops = Vec::new();
    while region_cfg.graph().node_count() > 2 {
        let entry = region_cfg.entry().context("failed to find entry region")?;
        let dominance = region_cfg
            .compute_dominance()
            .context("failed to compute region dominance")?;

        let mut progress = false;
        let mut order = DfsPostOrder::new(region_cfg.graph(), entry);
        while let Some(head) = order.next(region_cfg.graph()) {
            count += 1;
            std::fs::write(format!("tmp/region_cfg_{}.dot", count), region_cfg.dot())
                .expect("failed to write region cfg dot file");
            if dominance.has_backedge(&region_cfg, head) {
                eprintln!("cycle discovered: {}", head.index());
                let mut raw_loop =
                    find_smallest_loop(&region_cfg, &dominance, head, strucutured_loops.len());
                let (structured_loop, tail_edges) = find_loop(&mut region_cfg, &mut raw_loop);
                strucutured_loops.push(structured_loop.clone());
                virtualized_edges.extend(tail_edges);

                let mut body_graph = region_cfg.clone();
                for node in body_graph.clone().graph().node_indices() {
                    let region = region_cfg
                        .key_for_node(node)
                        .expect("node should have region");
                    if !structured_loop.body.contains(&region) {
                        body_graph.graph_mut().remove_node(node);
                    }
                }

                let body_vexit = body_graph.add_vexit().context("failed to add vexit")?;

                while body_graph.graph().node_count() > 1 {
                    let body_entry = body_graph.entry().expect("graph should have entry node");
                    let mut order = DfsPostOrder::new(body_graph.graph(), body_entry);
                    let dominance = body_graph
                        .compute_dominance()
                        .expect("failed to compute dominance");

                    while let Some(head) = order.next(body_graph.graph()) {
                        if match_acyclic(&mut body_graph, &dominance, head) {
                            progress = true;
                            break;
                        }
                    }

                    if !progress {
                        bail!("failed to find any acyclic pattern in loop");
                    }
                }

                let loop_region = contract_cyclic_region(&mut region_cfg, &structured_loop);
                body_graph.graph_mut().remove_node(body_vexit);
                let mut body_node_iter = body_graph.graph().node_indices();
                if let (Some(root_node), None) = (body_node_iter.next(), body_node_iter.next()) {
                    let root_region = body_graph
                        .key_for_node(root_node)
                        .expect("node should have region");
                    let Region::Loop { body, .. } = region_cfg
                        .region_mut(loop_region)
                        .expect("region should exist")
                    else {
                        bail!("invalid region");
                    };
                    *body = root_region;
                }
            } else {
                progress = match_acyclic(&mut region_cfg, &dominance, head);
                if progress {
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

    region_cfg.add_vexit().context("failed to add vexit")?;

    Ok(region_cfg)
}

fn match_acyclic(cfg: &mut RegionCfg, dominance: &RegionDominance, head: NodeIndex) -> bool {
    match_seq(cfg, head) || match_if(cfg, dominance, head)
}
