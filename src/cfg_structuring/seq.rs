use petgraph::{Direction, graph::NodeIndex};

use crate::{cfg_structuring::region::RegionId, graph::IndexedGraphView};

use super::Region;
use super::region::RegionCfg;

pub(crate) fn match_seq(region_cfg: &mut RegionCfg, head: NodeIndex) -> bool {
    match find_seq(region_cfg, head) {
        Some(seq) => {
            contract_seq(region_cfg, &seq);
            true
        }
        None => false,
    }
}

fn find_seq(cfg: &RegionCfg, head: NodeIndex) -> Option<Vec<NodeIndex>> {
    if head == cfg.vexit() {
        return None;
    }

    // predがただ1つ存在し、かつpredのsuccがheadだけである場合は、headは中間ノードになる
    if let Some(pred) = cfg.neighbor_if_one(head, Direction::Incoming)
        && cfg.degree(pred, Direction::Outgoing) == 1
    {
        return None;
    }

    let mut chain = vec![head];
    let mut target = head;

    // succがただ1つ存在する場合は、後続ノードを調べる
    while let Some(succ) = cfg.neighbor_if_one(target, Direction::Outgoing) {
        // succが合流点である場合はseqを終了する
        if succ == cfg.vexit() || cfg.degree(succ, Direction::Incoming) != 1 {
            break;
        }
        chain.push(succ);
        target = succ;
    }

    (chain.len() > 1).then_some(chain)
}

fn contract_seq(region_cfg: &mut RegionCfg, seq: &[NodeIndex]) -> RegionId {
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
