pub mod binding;
pub mod expr;
pub mod program;
pub mod types;

pub use binding::{Binding, Safety};
pub use expr::{BinOp, Const, Expr};
pub use program::{Placement, PlacementReport, Program};
pub use types::Type;

// All variables given a unique ID
pub type VarId = u32;
