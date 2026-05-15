use std::collections::{HashMap, HashSet};

use petgraph::{
    Direction,
    algo::dominators::{self, Dominators},
    graph::NodeIndex,
};

use crate::{
    graph::{DominanceFrontier, dominance_frontier},
    llir::{BlockId, Function, PhiFunc, Place, Value, Var, VarRole, VarVisitor},
};

pub fn construct_minimal_ssa(func: &Function) -> Function {
    let mut func = func.clone();

    let entry_node = func
        .cfg()
        .node_for_key(func.entry())
        .expect("entry node must exist");
    let df = dominance_frontier(func.cfg().graph(), entry_node);
    let phi_sites = compute_phi_sites(&func, &df);

    for (place, sites) in phi_sites {
        for site in sites {
            let block_id = func
                .cfg()
                .key_for_node(site)
                .expect("site block must exist");
            let block = func.block_mut(block_id);
            block.add_phi(PhiFunc::new(Var::from_place(place)));
        }
    }

    rename_vars(&mut func);

    func
}

fn compute_phi_sites(
    func: &Function,
    df: &DominanceFrontier<NodeIndex>,
) -> HashMap<Place, HashSet<NodeIndex>> {
    let mut place_to_phi_sites = HashMap::new();

    let places: HashSet<_> = func.vars().into_iter().map(|var| var.place()).collect();
    for place in places {
        let mut phi_sites = HashSet::new();

        let mut worklist: Vec<_> = func
            .find_def_of_place(place)
            .into_iter()
            .map(|block| func.cfg().node_for_key(block).expect("block must exist"))
            .collect();
        while let Some(node) = worklist.pop() {
            let Some(df_nodes) = df.get(&node) else {
                continue;
            };
            for df_node in df_nodes {
                if !phi_sites.contains(df_node) {
                    phi_sites.insert(*df_node);
                    worklist.push(*df_node);
                }
            }
        }

        if !phi_sites.is_empty() {
            place_to_phi_sites.insert(place, phi_sites);
        }
    }

    place_to_phi_sites
}

fn rename_vars(func: &mut Function) {
    let entry_node = func
        .cfg()
        .node_for_key(func.entry())
        .expect("entry node must exist");
    let dom = dominators::simple_fast(func.cfg().graph(), entry_node);
    RenameContext::new().rename_vars_in_block(func.entry(), func, &dom);
}

struct RenameContext {
    counter: HashMap<Place, u32>,
    stack: HashMap<Place, Vec<u32>>,
}

impl RenameContext {
    fn new() -> Self {
        Self {
            counter: HashMap::new(),
            stack: HashMap::new(),
        }
    }

    fn rename_vars_in_block(
        &mut self,
        id: BlockId,
        func: &mut Function,
        dom: &Dominators<NodeIndex>,
    ) {
        let mut pushed_places = Vec::new();

        self.rename_phi_defs(id, func, &mut pushed_places);
        self.rename_block_vars(id, func, &mut pushed_places);
        self.add_phi_args_to_successors(id, func);
        self.rename_dominated_children(id, func, dom);

        self.pop_versions(pushed_places);
    }

    fn rename_phi_defs(&mut self, id: BlockId, func: &mut Function, pushed_places: &mut Vec<Place>) {
        let block = func.block_mut(id);
        for phi in block.phi_mut() {
            let new_dst = self.define_var(phi.dst().place(), pushed_places);
            phi.set_dst(new_dst);
        }
    }

    fn rename_block_vars(&mut self, id: BlockId, func: &mut Function, pushed_places: &mut Vec<Place>) {
        let block = func.block_mut(id);
        block.rewrite_vars(&mut |r, v| match r {
            VarRole::Use => self.current_var(v.place()),
            VarRole::Def => self.define_var(v.place(), pushed_places),
        });
    }

    fn add_phi_args_to_successors(&self, id: BlockId, func: &mut Function) {
        let node = func.cfg().node_for_key(id).expect("block must exist");
        let succs: Vec<_> = func
            .cfg()
            .graph()
            .neighbors_directed(node, Direction::Outgoing)
            .collect();
        for succ in succs {
            let succ_block_id = func
                .cfg()
                .key_for_node(succ)
                .expect("successor block must exist");
            let succ_block = func.block_mut(succ_block_id);
            for phi in succ_block.phi_mut() {
                let arg = self.current_var(phi.dst().place());
                phi.add_arg(id, Value::Var(arg));
            }
        }
    }

    fn rename_dominated_children(
        &mut self,
        id: BlockId,
        func: &mut Function,
        dom: &Dominators<NodeIndex>,
    ) {
        let node = func.cfg().node_for_key(id).expect("block must exist");
        let dominated: Vec<_> = dom.immediately_dominated_by(node).collect();
        for succ in dominated {
            let succ_block_id = func
                .cfg()
                .key_for_node(succ)
                .expect("successor block must exist");
            self.rename_vars_in_block(succ_block_id, func, dom);
        }
    }

    fn pop_versions(&mut self, pushed_places: Vec<Place>) {
        for place in pushed_places.into_iter().rev() {
            let stack = self
                .stack
                .get_mut(&place)
                .expect("version stack must exist for defined place");
            stack.pop().expect("version stack must contain pushed version");
        }
    }

    fn current_var(&self, place: Place) -> Var {
        let version = self
            .stack
            .get(&place)
            .and_then(|stack| stack.last().copied())
            .unwrap_or(0);
        Var::with_version(place, version)
    }

    fn define_var(&mut self, place: Place, pushed_places: &mut Vec<Place>) -> Var {
        let counter = self.counter.entry(place).or_insert(0);
        *counter += 1;
        let version = *counter;
        self.stack.entry(place).or_insert_with(|| vec![0]).push(version);
        pushed_places.push(place);
        Var::with_version(place, version)
    }
}
