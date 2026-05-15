use std::collections::{HashMap, HashSet};

use petgraph::{
    Direction,
    algo::dominators::{self, Dominators},
    graph::NodeIndex,
    visit::{DfsPostOrder, Walker},
};

use crate::{
    graph::{DominanceFrontier, dominance_frontier},
    llir::{Function, PhiFunc, Place, Value, Var, VarRole, VarVisitor},
};

pub fn construct_ssa(func: &Function) -> Function {
    func.visit_vars(&mut |_, var| {
        assert_eq!(var.version(), 0);
    });

    let mut func = func.clone();

    let df = dominance_frontier(func.cfg().graph(), func.entry());
    let live_in = compute_live_in(&func);
    let phi_sites = compute_phi_sites(&func, &df, &live_in);

    for (var, sites) in phi_sites {
        for site in sites {
            let block = func.block_mut(func.block_for_node(site));
            block.add_phi(PhiFunc::new(var));
        }
    }

    rename_vars(&mut func);

    func
}

fn compute_phi_sites(
    func: &Function,
    df: &DominanceFrontier<NodeIndex>,
    live_in: &HashMap<NodeIndex, HashSet<Var>>,
) -> HashMap<Var, HashSet<NodeIndex>> {
    let mut var_to_phi_sites = HashMap::new();

    let vars: HashSet<_> = func.blocks().flat_map(|block| block.defs()).collect();
    for var in vars {
        let mut phi_sites = HashSet::new();

        let def_nodes: Vec<_> = func
            .blocks()
            .filter(|block| block.defs().contains(&var))
            .map(|block| func.node_for_block(block.id()))
            .collect();
        let mut worklist = def_nodes;
        while let Some(node) = worklist.pop() {
            let Some(df_nodes) = df.get(&node) else {
                continue;
            };
            for df_node in df_nodes {
                let live_here = live_in.get(df_node).is_some_and(|live| live.contains(&var));
                if !phi_sites.contains(df_node) && live_here {
                    phi_sites.insert(*df_node);
                    worklist.push(*df_node);
                }
            }
        }

        if !phi_sites.is_empty() {
            var_to_phi_sites.insert(var, phi_sites);
        }
    }

    var_to_phi_sites
}

fn compute_live_in(func: &Function) -> HashMap<NodeIndex, HashSet<Var>> {
    let block_uses: HashMap<NodeIndex, HashSet<Var>> = func
        .blocks()
        .map(|block| {
            let node = func.node_for_block(block.id());
            (node, block.uses())
        })
        .collect();

    let block_defs: HashMap<NodeIndex, HashSet<Var>> = func
        .blocks()
        .map(|block| {
            let node = func.node_for_block(block.id());
            (node, block.defs())
        })
        .collect();

    let post_order: Vec<_> = DfsPostOrder::new(func.cfg().graph(), func.entry())
        .iter(func.cfg().graph())
        .collect();

    let mut live_in = HashMap::new();
    loop {
        let mut changed = false;

        for &node in &post_order {
            // live_out[n] = U_{s in succ(n)} live_in[s]
            let new_live_out: HashSet<_> = func
                .cfg()
                .graph()
                .neighbors_directed(node, Direction::Outgoing)
                .flat_map(|succ| live_in.get(&succ).cloned().unwrap_or_else(HashSet::new))
                .collect();

            let uses = block_uses
                .get(&node)
                .expect("block uses must exist for node");
            let defs = block_defs
                .get(&node)
                .expect("block defs must exist for node");

            // live_in[n] = use[n] U (live_out[n] - def[n])
            let mut new_live_in = uses.clone();
            new_live_in.extend(new_live_out.difference(defs).copied());

            if live_in.get(&node) != Some(&new_live_in) {
                changed = true;
            }

            live_in.insert(node, new_live_in);
        }

        if !changed {
            break;
        }
    }
    live_in
}

fn rename_vars(func: &mut Function) {
    let dom = dominators::simple_fast(func.cfg().graph(), func.entry());
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
        node: NodeIndex,
        func: &mut Function,
        dom: &Dominators<NodeIndex>,
    ) {
        let mut pushed_places = Vec::new();

        self.rename_phi_defs(node, func, &mut pushed_places);
        self.rename_block_vars(node, func, &mut pushed_places);
        self.add_phi_args_to_successors(node, func);
        self.rename_dominated_children(node, func, dom);

        self.pop_versions(pushed_places);
    }

    fn rename_phi_defs(
        &mut self,
        node: NodeIndex,
        func: &mut Function,
        pushed_places: &mut Vec<Place>,
    ) {
        let block = func.block_mut(func.block_for_node(node));
        for phi in block.phi_mut() {
            let new_dst = self.define_var(phi.dst().place(), pushed_places);
            phi.set_dst(new_dst);
        }
    }

    fn rename_block_vars(
        &mut self,
        node: NodeIndex,
        func: &mut Function,
        pushed_places: &mut Vec<Place>,
    ) {
        let block = func.block_mut(func.block_for_node(node));
        block.rewrite_vars(&mut |r, v| match r {
            VarRole::Use => self.current_var(v.place()),
            VarRole::Def => self.define_var(v.place(), pushed_places),
        });
    }

    fn add_phi_args_to_successors(&self, node: NodeIndex, func: &mut Function) {
        let id = func.block_for_node(node);
        let succs: Vec<_> = func
            .cfg()
            .graph()
            .neighbors_directed(node, Direction::Outgoing)
            .collect();
        for succ in succs {
            let succ_block = func.block_mut(func.block_for_node(succ));
            for phi in succ_block.phi_mut() {
                let arg = self.current_var(phi.dst().place());
                phi.add_arg(id, Value::Var(arg));
            }
        }
    }

    fn rename_dominated_children(
        &mut self,
        node: NodeIndex,
        func: &mut Function,
        dom: &Dominators<NodeIndex>,
    ) {
        let dominated: Vec<_> = dom.immediately_dominated_by(node).collect();
        for succ in dominated {
            self.rename_vars_in_block(succ, func, dom);
        }
    }

    fn pop_versions(&mut self, pushed_places: Vec<Place>) {
        for place in pushed_places.into_iter().rev() {
            let stack = self
                .stack
                .get_mut(&place)
                .expect("version stack must exist for defined place");
            stack
                .pop()
                .expect("version stack must contain pushed version");
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
        self.stack
            .entry(place)
            .or_insert_with(|| vec![0])
            .push(version);
        pushed_places.push(place);
        Var::with_version(place, version)
    }
}
