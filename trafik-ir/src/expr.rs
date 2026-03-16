use crate::VarId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Const {
    U64(u64),
    I64(i64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Var(VarId),
    Const(Const),
    BinOp {
        op: BinOp,
        lhs: VarId,
        rhs: VarId,
    },
    // Call to a named function with positional arguments.
    Call {
        func: &'static str,
        args: Vec<VarId>,
    },
    // Get from a kernel field (e.g. cwnd)
    FieldGet {
        object: VarId,
        field: &'static str,
    },
}
