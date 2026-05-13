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

    for (var, sites) in phi_sites {
        for site in sites {
            let block_id = func
                .cfg()
                .key_for_node(site)
                .expect("site block must exist");
            let block = func.block_mut(block_id);
            block.add_phi(PhiFunc::new(var));
        }
    }

    rename_vars(&mut func);

    func
}

fn compute_phi_sites(
    func: &Function,
    df: &DominanceFrontier<NodeIndex>,
) -> HashMap<Var, HashSet<NodeIndex>> {
    let mut var_to_phi_sites = HashMap::new();

    for var in func.vars() {
        let mut phi_sites = HashSet::new();

        let mut worklist: Vec<_> = func
            .find_def(&var)
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

        var_to_phi_sites.insert(var, phi_sites);
    }

    var_to_phi_sites
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
        let saved = self.stack.clone();

        let block = func.block_mut(id);
        for phi in block.phi_mut() {
            let new_dst = self.new_version_of(phi.dst());
            phi.set_dst(new_dst);
        }

        block.rewrite_vars(&mut |r, v| match r {
            VarRole::Use => self.current_version_of(v),
            VarRole::Def => self.new_version_of(v),
        });

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
                let arg = self.current_version_of(phi.dst());
                phi.add_arg(id, Value::Var(arg));
            }
        }

        for succ in dom.immediately_dominated_by(node) {
            let succ_block_id = func
                .cfg()
                .key_for_node(succ)
                .expect("successor block must exist");
            self.rename_vars_in_block(succ_block_id, func, dom);
        }

        let _ = std::mem::replace(&mut self.stack, saved);
    }

    fn current_version_of(&self, var: Var) -> Var {
        let place = var.place();
        let version = self
            .stack
            .get(&place)
            .and_then(|stack| stack.last().copied())
            .unwrap_or(0);
        Var::with_version(place, version)
    }

    fn new_version_of(&mut self, var: Var) -> Var {
        let place = var.place();
        let version = *self.counter.entry(place).or_insert(0) + 1;
        self.counter.insert(place, version);
        self.stack.entry(place).or_insert(vec![0]).push(version);
        Var::with_version(place, version)
    }
}
