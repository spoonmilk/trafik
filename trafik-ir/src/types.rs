/// Scalar types expressible in eBPF
/// Floating point ops or non-supported types routed through UserOnly
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Bool,
    // Pointer into kernel memory (e.g tcp sockets, struct ops members, etc.)
    KernelPtr(&'static str),
}
