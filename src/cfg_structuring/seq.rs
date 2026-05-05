use std::collections::HashSet;

use petgraph::{Direction, graph::NodeIndex};

use crate::{
    cfg_structuring::{
        region::{RegionCfg, RegionId},
        scope::{is_in_scope, scoped_degree, scoped_neighbor_if_one},
    },
    graph::IndexedGraphView,
};

use super::Region;

pub(crate) fn find_seq(cfg: &RegionCfg, head: NodeIndex) -> Option<Vec<NodeIndex>> {
    find_seq_with_scope(cfg, head, None)
}

pub(crate) fn find_seq_in_scope(
    cfg: &RegionCfg,
    head: NodeIndex,
    scope: &HashSet<RegionId>,
) -> Option<Vec<NodeIndex>> {
    find_seq_with_scope(cfg, head, Some(scope))
}

fn find_seq_with_scope(
    cfg: &RegionCfg,
    head: NodeIndex,
    scope: Option<&HashSet<RegionId>>,
) -> Option<Vec<NodeIndex>> {
    if head == cfg.vexit() || !is_in_scope(cfg, head, scope) {
        return None;
    }

    // predがただ1つ存在し、かつpredのsuccがheadだけである場合は、headは中間ノードになる
    if let Some(pred) = scoped_neighbor_if_one(cfg, head, Direction::Incoming, scope)
        && scoped_degree(cfg, pred, Direction::Outgoing, scope) == 1
    {
        return None;
    }

    let mut chain = vec![head];
    let mut target = head;

    // succがただ1つ存在する場合は、後続ノードを調べる
    while let Some(succ) = scoped_neighbor_if_one(cfg, target, Direction::Outgoing, scope) {
        // succが合流点である場合はseqを終了する
        if succ == cfg.vexit() || scoped_degree(cfg, succ, Direction::Incoming, scope) != 1 {
            break;
        }
        chain.push(succ);
        target = succ;
    }

    (chain.len() > 1).then_some(chain)
}

pub(crate) fn contract_seq(region_cfg: &mut RegionCfg, seq: &[NodeIndex]) -> RegionId {
    let seq_head = seq.first().unwrap();
    let seq_tail = seq.last().unwrap();
    let seq_inners: Vec<_> = seq
        .iter()
        .map(|&node_index| region_cfg.key_for_node(node_index).unwrap())
        .collect();

    let (seq_region, seq_node) = region_cfg.add_region(Region::Seq(seq_inners));

    region_cfg.redirect_edges(*seq_head, seq_node, Direction::Incoming);
    region_cfg.redirect_edges(*seq_tail, seq_node, Direction::Outgoing);

    for node in seq {
        region_cfg
            .remove_node_by_index(*node)
            .expect("failed to remove node in seq");
    }
    seq_region
}
