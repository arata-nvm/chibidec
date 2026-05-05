pub mod cycle;
pub mod if_then;
pub mod region;
mod scope;
pub mod seq;

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail};
use petgraph::{
    graph::NodeIndex,
    visit::{DfsPostOrder, EdgeRef, IntoEdgeReferences},
};

use crate::{
    cfg_recovery::cfg::Cfg,
    cfg_structuring::{
        cycle::{contract_cyclic_region, find_loop, find_smallest_loop},
        if_then::{IfSchema, contract_if, find_if_in_scope, match_if},
        region::{Region, RegionCfg, RegionDominance},
        seq::{contract_seq, find_seq_in_scope, match_seq},
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

                let body_region = reduce_loop_body(&mut region_cfg, structured_loop.body.clone())?;
                contract_cyclic_region(&mut region_cfg, &structured_loop, body_region);
                progress = true;
                break;
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

fn reduce_loop_body(
    cfg: &mut RegionCfg,
    mut scope: HashSet<region::RegionId>,
) -> Result<region::RegionId> {
    while scope.len() > 1 {
        let dominance = cfg
            .compute_dominance()
            .context("failed to compute dominance")?;
        let mut body_nodes: Vec<_> = scope
            .iter()
            .map(|region| {
                cfg.node_for_key(*region)
                    .context("loop body region should have node")
            })
            .collect::<Result<_>>()?;
        body_nodes.sort_by_key(|node| node.index());

        let mut progress = false;
        for head in body_nodes {
            if let Some(seq) = find_seq_in_scope(cfg, head, &scope) {
                let old_regions: Vec<_> = seq
                    .iter()
                    .map(|node| cfg.key_for_node(*node).expect("node should have region"))
                    .collect();
                let new_region = contract_seq(cfg, &seq);
                for region in old_regions {
                    scope.remove(&region);
                }
                scope.insert(new_region);
                progress = true;
                break;
            }

            if let Some(if_schema) = find_if_in_scope(cfg, &dominance, head, &scope) {
                let old_regions = if_regions_removed_from_scope(&if_schema);
                let new_region = contract_if(cfg, if_schema);
                for region in old_regions {
                    scope.remove(&region);
                }
                scope.insert(new_region);
                progress = true;
                break;
            }
        }

        if !progress {
            bail!("failed to find any acyclic pattern in loop");
        }
    }

    scope
        .into_iter()
        .next()
        .context("loop body should contain at least one region")
}

fn if_regions_removed_from_scope(if_schema: &IfSchema) -> Vec<region::RegionId> {
    let mut regions = Vec::new();
    regions.push(if_schema.head);
    regions.extend(if_schema.then_body.iter().copied());
    if let Some(else_body) = &if_schema.else_body {
        regions.extend(else_body.iter().copied());
    }
    regions
}
