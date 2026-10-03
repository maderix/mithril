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

impl BinOp {
    /// Declaration order is the opcode order (the index).
    pub const ALL: [BinOp; 11] = [
        BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div, BinOp::FloorDiv, BinOp::Mod,
        BinOp::Shl, BinOp::Shr, BinOp::BitAnd, BinOp::BitOr, BinOp::BitXor,
    ];
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

impl CmpOp {
    /// Declaration order is the opcode order (16 + index).
    pub const ALL: [CmpOp; 6] = [CmpOp::Lt, CmpOp::Le, CmpOp::Gt, CmpOp::Ge, CmpOp::Eq, CmpOp::Ne];
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
    /// unary minus (`-e`; a literal operand is folded by the parser)
    Neg(Box<Expr>),
    IfExp(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    Tuple(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Lambda(Vec<String>, Box<Expr>),
}

impl Expr {
    pub fn kids(&self) -> Vec<&Expr> {
        match self {
            Self::Bin(_, a, b) | Self::Cmp(_, a, b) | Self::Bool2(_, a, b) | Self::Index(a, b) => vec![a, b],
            Self::Not(a) | Self::Neg(a) | Self::Lambda(_, a) => vec![a],
            Self::IfExp(c, a, b) => vec![c, a, b],
            Self::Call(_, xs) | Self::Tuple(xs) => xs.iter().collect(),
            Self::Int(_) | Self::Float(_) | Self::Bool(_) | Self::Var(_) => Vec::new(),
        }
    }

    pub fn any(&self, p: &dyn Fn(&Expr) -> bool) -> bool {
        p(self) || self.kids().into_iter().any(|e| e.any(p))
    }

    pub fn fold<T>(&self, f: &mut dyn FnMut(&Expr, Vec<T>) -> T) -> T {
        let children = self.kids().into_iter().map(|e| e.fold(f)).collect();
        f(self, children)
    }
}

/// A proven fold: carried by `Stmt::For` (`None` out of the parser; reassoc
/// sets it) and copied by desugar onto the loop helper's `CoreFn.fold`.
#[derive(Clone, PartialEq, Debug)]
pub struct FoldInfo {
    pub combiner: Combiner,
    pub proven: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Combiner {
    WrapAdd,
    TupleWrapAdd(usize),
    /// u32-emulation folds (`& 4294967295`-masked): wrapping add mod
    /// 2^32; the join must re-mask its result to the low 32 bits.
    WrapAdd32,
    TupleWrapAdd32(usize),
    /// An index fill: the loop's state is an array written once per
    /// iteration at the loop index (`a = array_set(a, i, e)`), and nothing
    /// in the body reads the array. Writes at distinct indices commute, so
    /// chunks of the range write their parts of one buffer in any order.
    Fill,
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

impl Stmt {
    /// Head expression and nested blocks, in source order. Binders stay on the statement.
    pub fn parts(&self) -> (&Expr, Vec<&[Stmt]>) {
        match self {
            Self::Assign(_, e) | Self::Return(e) | Self::ExprStmt(e) => (e, Vec::new()),
            Self::If(e, a, b) => (e, vec![a, b]),
            Self::While(e, b) | Self::For(_, e, b, _) => (e, vec![b]),
            Self::Match(e, arms) => (e, arms.iter().map(|(_, b)| b.as_slice()).collect()),
        }
    }
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
