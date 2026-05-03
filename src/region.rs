use std::{collections::HashMap, fmt};

use petgraph::{Direction, algo::dominators::Dominators, graph::NodeIndex, visit::EdgeRef};

use crate::cfg::{BlockArena, BlockId, Cfg, Condition, dot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(usize);

impl fmt::Display for RegionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Region{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub enum Region {
    Leaf(BlockId),
    Seq(Vec<RegionId>),
    IfThen {
        head: RegionId,
        then_br: Vec<RegionId>,
        join: RegionId,
        cond: Option<Condition>,
    },
    IfThenElse {
        head: RegionId,
        then_br: Vec<RegionId>,
        else_br: Vec<RegionId>,
        join: RegionId,
        cond: Option<Condition>,
    },
}

#[derive(Default)]
pub struct RegionArena {
    regions: Vec<Region>,
}

impl RegionArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc(&mut self, region: Region) -> RegionId {
        let id = RegionId(self.regions.len());
        self.regions.push(region);
        id
    }

    pub fn get(&self, id: RegionId) -> &Region {
        &self.regions[id.0]
    }

    pub fn attach_seq(
        &mut self,
        seq_block_id: BlockId,
        inner_block_ids: Vec<BlockId>,
        block_to_region: &mut HashMap<BlockId, RegionId>,
    ) -> RegionId {
        let region_ids = self.region_ids_of(block_to_region, &inner_block_ids);
        let region_id = self.alloc(Region::Seq(region_ids));
        block_to_region.insert(seq_block_id, region_id);
        region_id
    }

    fn region_id_of(
        &self,
        block_to_region: &HashMap<BlockId, RegionId>,
        block_id: BlockId,
    ) -> RegionId {
        block_to_region[&block_id]
    }

    fn region_ids_of(
        &self,
        block_to_region: &HashMap<BlockId, RegionId>,
        block_ids: &[BlockId],
    ) -> Vec<RegionId> {
        block_ids
            .iter()
            .map(|&block_id| self.region_id_of(block_to_region, block_id))
            .collect()
    }
}

fn find_seq(cfg: &Cfg, head: NodeIndex, vexit: NodeIndex) -> Option<Vec<NodeIndex>> {
    if head == vexit {
        return None;
    }

    // predがただ1つ存在し、かつpredのsuccがheadだけである場合は、headは中間ノードになる
    if let Some(pred) = neighbor_if_one(cfg, head, Direction::Incoming)
        && degree(cfg, pred, Direction::Outgoing) == 1
    {
        return None;
    }

    let mut chain = vec![head];
    let mut target = head;

    // succがただ1つ存在する場合は、後続ノードを調べる
    while let Some(succ) = neighbor_if_one(cfg, target, Direction::Outgoing) {
        // succが合流点である場合はseqを終了する
        if succ == vexit || degree(cfg, succ, Direction::Incoming) != 1 {
            break;
        }
        chain.push(succ);
        target = succ;
    }

    return (chain.len() > 1).then_some(chain);

    fn degree(cfg: &Cfg, node: NodeIndex, dir: Direction) -> usize {
        cfg.edges_directed(node, dir).count()
    }

    // nodeのdir方向の隣接ノードが1つだけ存在する場合は、そのノードを返す。それ以外の場合はNoneを返す。
    fn neighbor_if_one(cfg: &Cfg, node: NodeIndex, dir: Direction) -> Option<NodeIndex> {
        let mut neighbors = cfg.neighbors_directed(node, dir);
        let next = neighbors.next()?;
        if neighbors.next().is_some() {
            return None;
        }
        Some(next)
    }
}

pub fn match_acyclic(
    cfg: &mut Cfg,
    head: NodeIndex,
    vexit: NodeIndex,
    seq_count: &mut usize,
    block_arena: &mut BlockArena,
    region_arena: &mut RegionArena,
    block_to_region: &mut HashMap<BlockId, RegionId>,
    dom: &Dominators<NodeIndex>,
) -> bool {
    if let Some(seq) = find_seq(cfg, head, vexit) {
        let seq_inners: Vec<_> = seq
            .iter()
            .map(|&node_index| cfg.node_weight(node_index).unwrap())
            .copied()
            .collect();
        let seq_block = contract_seq(cfg, &seq, seq_count, block_arena);
        region_arena.attach_seq(seq_block, seq_inners, block_to_region);
        let _ = std::fs::write("./seq.dot", dot(cfg, block_arena));
        return true;
    }
    false
}

fn contract_seq(
    cfg: &mut Cfg,
    seq: &[NodeIndex],
    seq_count: &mut usize,
    block_arena: &mut BlockArena,
) -> BlockId {
    let seq_head = seq.first().unwrap();
    let seq_tail = seq.last().unwrap();

    let seq_block_id = block_arena.new_block(
        0,
        0,
        Vec::new(),
        Some(format!("contracted seq {}", *seq_count)),
    );
    let seq_node = block_arena.add_block_to_graph(cfg, seq_block_id);

    redirect_edges(cfg, *seq_head, seq_node, Direction::Incoming);
    redirect_edges(cfg, *seq_tail, seq_node, Direction::Outgoing);

    *seq_count += 1;
    remove_nodes(cfg, block_arena, seq, None);
    seq_block_id
}

fn redirect_edges(cfg: &mut Cfg, target: NodeIndex, new_target: NodeIndex, dir: Direction) {
    let edges: Vec<_> = cfg
        .edges_directed(target, dir)
        .map(|e| {
            let other = match dir {
                Direction::Incoming => e.source(),
                Direction::Outgoing => e.target(),
            };
            (e.id(), other)
        })
        .collect();
    for (edge_id, edge_source) in edges {
        let Some(weight) = cfg.remove_edge(edge_id) else {
            continue;
        };
        let (source, target) = match dir {
            Direction::Incoming => (edge_source, new_target),
            Direction::Outgoing => (new_target, edge_source),
        };
        if !cfg.contains_edge(source, target) {
            cfg.add_edge(source, target, weight);
        }
    }
}

fn remove_nodes(
    cfg: &mut Cfg,
    block_arena: &mut BlockArena,
    nodes: &[NodeIndex],
    except: Option<NodeIndex>,
) {
    for &node in nodes {
        if except.is_some_and(|except| node == except) {
            continue;
        }
        let Some(&block_id) = cfg.node_weight(node) else {
            continue;
        };
        if let Err(e) = block_arena.remove_block_from_graph(cfg, block_id) {
            eprintln!("failed to remove block {} from graph: {}", block_id, e);
        }
    }
}
