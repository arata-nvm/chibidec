pub mod asm;
pub mod cycle;
pub mod if_then;
pub mod region;
mod scope;
pub mod seq;

use std::collections::HashMap;

use anyhow::{Context, Result, bail};
use petgraph::{
    Direction,
    graph::NodeIndex,
    visit::{DfsPostOrder, EdgeRef, IntoEdgeReferences},
};

use crate::{
    cfg_recovery::cfg::{Cfg, EdgeLabel},
    cfg_structuring::{
        cycle::{RawLoop, contract_cyclic_region, find_loop, find_smallest_loop},
        if_then::{IfSchema, contract_if, find_if},
        region::{Region, RegionCfg, RegionId, RegionStore},
        seq::{contract_seq, find_seq},
    },
    graph::{IndexedGraphView, IndexedGraphViewMut},
};

#[derive(Debug, Clone)]
pub struct StructuredCfg {
    pub cfg: RegionCfg,
    pub regions: RegionStore,
    pub virtualized_edges: Vec<VirtualizedEdge>,
}

#[derive(Debug, Clone)]
pub struct VirtualizedEdge {
    pub source: RegionId,
    pub target: RegionId,
    pub label: EdgeLabel,
}

pub fn structure_cfg(cfg: &Cfg) -> Result<StructuredCfg> {
    let (mut region_cfg, mut regions) =
        build_region_cfg(cfg).context("failed to create region cfg")?;

    let mut count = 0;
    let mut next_loop_index = 0;
    let mut virtualized_edges = Vec::new();
    while region_cfg.graph().node_count() > 2 {
        let entry = region_cfg.entry().context("failed to find entry region")?;

        let mut progress = false;
        let mut order = DfsPostOrder::new(region_cfg.graph(), entry);
        while let Some(head) = order.next(region_cfg.graph()) {
            count += 1;
            std::fs::write(
                format!("tmp/region_cfg_{}.dot", count),
                region_cfg.dot(&regions),
            )
            .expect("failed to write region cfg dot file");

            match find_reduction(&region_cfg, head, next_loop_index)? {
                Some(Reduction::Cycle {
                    target: _,
                    mut raw_loop,
                }) => {
                    let (structured_loop, tail_edges) = find_loop(&mut region_cfg, &mut raw_loop);
                    next_loop_index += 1;
                    virtualized_edges.extend(tail_edges);

                    let mut body_cfg =
                        create_loop_body_cfg(&region_cfg, &structured_loop, &mut regions)?;
                    let body_region = reduce_loop_body(
                        &mut body_cfg,
                        &mut regions,
                        &mut virtualized_edges,
                        &mut next_loop_index,
                        structured_loop.index,
                    )?;
                    contract_cyclic_region(
                        &mut region_cfg,
                        &mut regions,
                        &structured_loop,
                        body_region,
                    );
                    progress = true;
                    break;
                }
                Some(Reduction::Seq(seq)) => {
                    contract_seq(&mut region_cfg, &mut regions, &seq);
                    progress = true;
                    break;
                }
                Some(Reduction::If(if_schema)) => {
                    contract_if(&mut region_cfg, &mut regions, if_schema);
                    progress = true;
                    break;
                }
                None => {}
            }
        }

        if !progress {
            break;
        }
    }

    Ok(StructuredCfg {
        cfg: region_cfg,
        regions,
        virtualized_edges,
    })
}

enum Reduction {
    Cycle {
        target: NodeIndex,
        raw_loop: RawLoop,
    },
    Seq(Vec<NodeIndex>),
    If(IfSchema),
}

fn find_reduction(
    cfg: &RegionCfg,
    head: NodeIndex,
    loop_index: usize,
) -> Result<Option<Reduction>> {
    {
        let dominance = cfg
            .compute_dominance()
            .context("failed to compute region dominance")?;
        if dominance.has_backedge(head) {
            let raw_loop = find_smallest_loop(&dominance, head, loop_index);
            return Ok(Some(Reduction::Cycle {
                target: head,
                raw_loop,
            }));
        }

        if let Some(seq) = find_seq(cfg, head) {
            return Ok(Some(Reduction::Seq(seq)));
        }

        Ok(find_if(&dominance, head).map(Reduction::If))
    }
}

fn build_region_cfg(cfg: &Cfg) -> Result<(RegionCfg, RegionStore)> {
    let mut regions = RegionStore::new();
    let mut region_cfg =
        RegionCfg::with_capacity(cfg.graph().node_count() + 1, cfg.graph().edge_count() + 1);

    // 基本ブロックを領域に変換する
    let mut node_map = HashMap::new();
    for cfg_node in cfg.graph().node_indices() {
        let block = cfg
            .key_for_node(cfg_node)
            .expect("function cfg node without block id");
        let (_, region_node) = region_cfg.add_region(&mut regions, Region::Leaf(block));
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

    region_cfg
        .add_vexit(&mut regions)
        .context("failed to add vexit")?;

    Ok((region_cfg, regions))
}

fn reduce_loop_body(
    cfg: &mut RegionCfg,
    regions: &mut RegionStore,
    virtualized_edges: &mut Vec<VirtualizedEdge>,
    next_loop_index: &mut usize,
    loop_index: usize,
) -> Result<RegionId> {
    while cfg.graph().node_count() > 2 {
        let mut progress = false;
        let entry = cfg.entry().context("failed to find loop body entry")?;
        let mut order = DfsPostOrder::new(cfg.graph(), entry);
        while let Some(head) = order.next(cfg.graph()) {
            let nested_loop = {
                let dominance = cfg
                    .compute_dominance()
                    .context("failed to compute loop body dominance")?;
                if dominance.has_backedge(head) {
                    Some(find_smallest_loop(&dominance, head, *next_loop_index))
                } else {
                    None
                }
            };
            if let Some(mut raw_loop) = nested_loop {
                let (structured_loop, tail_edges) = find_loop(cfg, &mut raw_loop);
                *next_loop_index += 1;
                virtualized_edges.extend(tail_edges);

                let mut body_cfg = create_loop_body_cfg(cfg, &structured_loop, regions)?;
                let body_region = reduce_loop_body(
                    &mut body_cfg,
                    regions,
                    virtualized_edges,
                    next_loop_index,
                    structured_loop.index,
                )?;
                contract_cyclic_region(cfg, regions, &structured_loop, body_region);
                progress = true;
                break;
            }

            if let Some(seq) = find_seq(cfg, head) {
                contract_seq(cfg, regions, &seq);
                progress = true;
                break;
            }

            let if_schema = {
                let dominance = cfg
                    .compute_dominance()
                    .context("failed to compute loop body dominance")?;
                find_if(&dominance, head)
            };
            if let Some(if_schema) = if_schema {
                contract_if(cfg, regions, if_schema);
                progress = true;
                break;
            }
        }

        if !progress {
            bail!("failed to find any acyclic pattern in loop body {loop_index}");
        }
    }

    cfg.graph()
        .node_indices()
        .find(|node| *node != cfg.vexit())
        .and_then(|node| cfg.key_for_node(node))
        .context("loop body should contain exactly one region")
}

fn create_loop_body_cfg(
    main_cfg: &RegionCfg,
    structured_loop: &cycle::StructuredLoop,
    regions: &mut RegionStore,
) -> Result<RegionCfg> {
    let mut body_cfg = RegionCfg::with_capacity(
        structured_loop.body.len() + 1,
        main_cfg.graph().edge_count() + 1,
    );

    for node in main_cfg.graph().node_indices() {
        let Some(region) = main_cfg.key_for_node(node) else {
            continue;
        };
        if structured_loop.body.contains(&region) {
            body_cfg.add_existing_region(region);
        }
    }

    for edge in main_cfg.graph().edge_references() {
        let Some(source_region) = main_cfg.key_for_node(edge.source()) else {
            continue;
        };
        let Some(target_region) = main_cfg.key_for_node(edge.target()) else {
            continue;
        };
        if !structured_loop.body.contains(&source_region)
            || !structured_loop.body.contains(&target_region)
        {
            continue;
        }
        let source = body_cfg
            .node_for_key(source_region)
            .context("body source region should have node")?;
        let target = body_cfg
            .node_for_key(target_region)
            .context("body target region should have node")?;
        body_cfg
            .graph_mut()
            .add_edge(source, target, edge.weight().clone());
    }

    body_cfg
        .add_vexit(regions)
        .context("failed to add loop body vexit")?;

    let entry = body_cfg
        .node_for_key(structured_loop.entry)
        .or_else(|| body_cfg.graph().externals(Direction::Incoming).next())
        .context("loop body cfg should have an entry")?;
    if body_cfg
        .graph()
        .neighbors_directed(entry, Direction::Incoming)
        .next()
        .is_some()
    {
        bail!("loop body entry still has incoming edges");
    }

    Ok(body_cfg)
}
