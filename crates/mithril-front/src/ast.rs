//! Surface AST produced by the parser (see shared-interfaces.md).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    FloorDiv,
    Mod,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CmpOp {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoolOp {
    And,
    Or,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Var(String),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Cmp(CmpOp, Box<Expr>, Box<Expr>),
    Bool2(BoolOp, Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    IfExp(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Tuple(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Lambda(Vec<String>, Box<Expr>),
}

/// Binding decision (controller): carried by `Stmt::For`, always `None` out
/// of the parser. Task 4 fills it in; Task 3's desugar copies it onto the
/// generated fold-function's `CoreFn.fold` (converting `Fn(String)` to
/// `Fn(FnId)` via name resolution).
#[derive(Clone, PartialEq, Debug)]
pub struct FoldInfo {
    pub combiner: Combiner,
    pub proven: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Combiner {
    WrapAdd,
    TupleWrapAdd(usize),
    /// Surface level: the combiner function's *name*. `core.rs`/desugar
    /// resolve this to a `FnId` when lowering.
    Fn(String),
}

#[derive(Clone, PartialEq, Debug)]
pub enum Stmt {
    Assign(String, Expr),
    Return(Expr),
    ExprStmt(Expr),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    /// `for v in range(e): body` — `fold` is always `None` from the parser.
    For(String, Expr, Vec<Stmt>, Option<FoldInfo>),
    Match(Expr, Vec<(Pat, Vec<Stmt>)>),
}

/// A `match` case pattern: `Ctor(a, b)` (ctor pattern, `binds` are the
/// bound names) or an integer literal, encoded as `ctor = "__int:<value>"`
/// with `binds = []`.
#[derive(Clone, PartialEq, Debug)]
pub struct Pat {
    pub ctor: String,
    pub binds: Vec<String>,
}

impl Pat {
    pub fn int_lit(v: i64) -> Pat {
        Pat { ctor: format!("__int:{v}"), binds: Vec::new() }
    }

    pub fn as_int_lit(&self) -> Option<i64> {
        self.ctor.strip_prefix("__int:").and_then(|s| s.parse::<i64>().ok())
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct DataDef {
    pub name: String,
    pub ctors: Vec<(String, Vec<String>)>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct FnDef {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Module {
    pub datas: Vec<DataDef>,
    pub fns: Vec<FnDef>,
}
