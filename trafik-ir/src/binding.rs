use crate::{Expr, Type, VarId};

/// User safety annotations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Safety {
    KernelSafe,
    UserOnly,
}

/// Atomic step unit through a program, sigma in PL terms
#[derive(Debug, Clone)]
pub enum Binding {
    /// Bind the result of an expression to a variable.
    Let {
        var: VarId,
        // Nonetypes treated as automatic pass failure
        ty: Option<Type>,
        expr: Expr,
        // Bindings to any expression or variable inherit safety from the parent
        // or by operation type
        safety: Safety,
        cost: u32,
    },
    /// Conditional. If cond fails, the whole statement is marked unsafe
    /// if not, branch splits can be independently safe or unsafe.
    If {
        cond: VarId,
        then_bindings: Vec<Binding>,
        else_bindings: Vec<Binding>,
    },
    /// Bounded loop
    For {
        var: VarId,
        // Must be constant or routed through UserOnly
        bound: VarId,
        body: Vec<Binding>,
    },
}
