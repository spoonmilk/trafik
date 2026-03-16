use crate::{Binding, Safety, VarId};

/// Congestion control function call
#[derive(Debug, Clone)]
pub struct Program {
    pub name: &'static str,
    pub bindings: Vec<Binding>,
    // The variable whose value is the output of this program.
    pub result: VarId,
}

/// Summary produced by placement analysis.
#[derive(Debug, Clone)]
pub struct PlacementReport {
    pub weight: u32,
    // n values exchanged between userspcae and kernel
    pub boundary_values: usize,
    pub place: Placement,
}

/// Placement recommendation
/// Split will require sub-program split placement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    FullKernel,
    FullUserspace,
    Split,
}

impl Program {
    pub fn placement_report(&self) -> PlacementReport {
        let mut weight = 0u32;

        accumulate_weights(&self.bindings, &mut weight);

        let boundary_values = count_boundary_crossings(&self.bindings);

        let recommendation = if weight == 0 {
            // If nothing is user-only, fully codegen to eBPF
            Placement::FullKernel
        } else {
            // Idk, cost modeling stuff here
            todo!()
        };

        PlacementReport {
            weight,
            boundary_values,
            place: recommendation,
        }
    }
}

fn accumulate_weights(bindings: &[Binding], user: &mut u32) {
    for b in bindings {
        match b {
            Binding::Let { safety, cost, .. } => {
                if safety == &Safety::UserOnly {
                    *user += cost
                }
            }
            Binding::If {
                then_bindings,
                else_bindings,
                ..
            } => {
                accumulate_weights(then_bindings, user);
                accumulate_weights(else_bindings, user);
            }
            Binding::For { body, .. } => {
                accumulate_weights(body, user);
            }
        }
    }
}

/// Count userspace/kernel crossings
fn count_boundary_crossings(bindings: &[Binding]) -> usize {
    use std::collections::HashMap;

    let mut def_safety: HashMap<VarId, Safety> = HashMap::new();
    collect_def_safety(bindings, &mut def_safety);

    let mut crossings = 0usize;
    check_crossings(bindings, &def_safety, &mut crossings);
    crossings
}

fn collect_def_safety(bindings: &[Binding], map: &mut std::collections::HashMap<VarId, Safety>) {
    for b in bindings {
        match b {
            Binding::Let { var, safety, .. } => {
                map.insert(*var, *safety);
            }
            Binding::If {
                then_bindings,
                else_bindings,
                ..
            } => {
                collect_def_safety(then_bindings, map);
                collect_def_safety(else_bindings, map);
            }
            Binding::For { body, .. } => collect_def_safety(body, map),
        }
    }
}

fn check_crossings(
    bindings: &[Binding],
    def_safety: &std::collections::HashMap<VarId, Safety>,
    count: &mut usize,
) {
    for b in bindings {
        match b {
            Binding::Let {
                safety: use_safety,
                expr,
                ..
            } => {
                let used_vars = vars_in_expr(expr);
                for v in used_vars {
                    if let Some(def_safety) = def_safety.get(&v)
                        && def_safety != use_safety
                    {
                        *count += 1;
                    }
                }
            }
            Binding::If {
                then_bindings,
                else_bindings,
                ..
            } => {
                check_crossings(then_bindings, def_safety, count);
                check_crossings(else_bindings, def_safety, count);
            }
            Binding::For { body, .. } => check_crossings(body, def_safety, count),
        }
    }
}

fn vars_in_expr(expr: &crate::expr::Expr) -> Vec<VarId> {
    use crate::expr::Expr;
    match expr {
        Expr::Var(v) => vec![*v],
        Expr::Const(_) => vec![],
        Expr::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
        Expr::Call { args, .. } => args.clone(),
        Expr::FieldGet { object, .. } => vec![*object],
    }
}
