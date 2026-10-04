//! Recursive-descent parser with precedence climbing over the token
//! stream produced by `lex`.

use crate::ast::*;
use crate::lex::{TokKind, Token, UNSUPPORTED_KEYWORDS};
use crate::Diag;

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
    /// hidden names made so far (range loops, tuple destructuring): keeps
    /// each one unique
    hidden: usize,
    /// a `**` needs the integer power helper
    pow_used: bool,
}

pub fn parse_module(toks: &[Token]) -> Result<Module, Diag> {
    let mut p = Parser { toks, pos: 0, hidden: 0, pow_used: false };
    p.module()
}

/// `b ** e` for an exponent that is not a small literal (integers; a
/// negative exponent gives 1).
const IPOW: &str = "
def __ipow(b, e):
    r = 1
    while e > 0:
        if e & 1 == 1:
            r = r * b
        b = b * b
        e = e >> 1
    return r
";

/// Every use of a module constant `N` becomes the call `N()`, except in a
/// function that binds `N` itself (Python: assigning a name makes it local);
/// `min`, `max` and `abs` become comparisons where the program defines no
/// function of that name.
fn resolve_globals(m: &mut Module, consts: &[String]) {
    let defined: std::collections::HashSet<String> = m.fns.iter().map(|f| f.name.clone()).collect();
    for f in &mut m.fns {
        let mut local: std::collections::BTreeSet<String> = crate::desugar::assigned_names(&f.body);
        local.extend(f.params.iter().cloned());
        let globals: Vec<&String> = consts.iter().filter(|c| !local.contains(*c)).collect();
        let builtin = |n: &str| !defined.contains(n) && !local.contains(n);
        let (mn, mx, ab) = (builtin("min"), builtin("max"), builtin("abs"));
        for_each_expr(&mut f.body, &mut |e| {
            match e {
                Expr::Var(n) if globals.iter().any(|g| *g == n) => *e = Expr::Call(n.clone(), Vec::new()),
                Expr::Call(n, args) if args.len() >= 2 && ((n == "min" && mn) || (n == "max" && mx)) => {
                    let op = if n == "min" { CmpOp::Le } else { CmpOp::Ge };
                    let mut args = std::mem::take(args).into_iter();
                    let first = args.next().unwrap();
                    *e = args.fold(first, |a, b| {
                        Expr::IfExp(Box::new(Expr::Cmp(op, Box::new(a.clone()), Box::new(b.clone()))), Box::new(a), Box::new(b))
                    });
                }
                Expr::Call(n, args) if args.len() == 1 && n == "abs" && ab => {
                    let x = args.pop().unwrap();
                    let neg = Expr::Neg(Box::new(x.clone()));
                    *e = Expr::IfExp(Box::new(Expr::Cmp(CmpOp::Ge, Box::new(x.clone()), Box::new(Expr::Int(0)))), Box::new(x), Box::new(neg));
                }
                _ => {}
            }
        });
    }
}

/// Apply `f` to every expression in `ss`, innermost first.
fn for_each_expr(ss: &mut [Stmt], f: &mut dyn FnMut(&mut Expr)) {
    fn ex(e: &mut Expr, f: &mut dyn FnMut(&mut Expr)) {
        match e {
            Expr::Bin(_, a, b) | Expr::Cmp(_, a, b) | Expr::Bool2(_, a, b) | Expr::Index(a, b) => {
                ex(a, f);
                ex(b, f);
            }
            Expr::Not(a) | Expr::Neg(a) | Expr::Lambda(_, a) => ex(a, f),
            Expr::IfExp(c, a, b) => {
                ex(c, f);
                ex(a, f);
                ex(b, f);
            }
            Expr::Call(_, xs) | Expr::Tuple(xs) => xs.iter_mut().for_each(|x| ex(x, f)),
            Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Var(_) => {}
        }
        f(e);
    }
    for s in ss {
        match s {
            Stmt::Assign(_, e) | Stmt::Return(e) | Stmt::ExprStmt(e) => ex(e, f),
            Stmt::If(c, a, b) => {
                ex(c, f);
                for_each_expr(a, f);
                for_each_expr(b, f);
            }
            Stmt::While(c, b) | Stmt::For(_, c, b, _) => {
                ex(c, f);
                for_each_expr(b, f);
            }
            Stmt::Match(e, arms) => {
                ex(e, f);
                arms.iter_mut().for_each(|(_, b)| for_each_expr(b, f));
            }
            Stmt::Break | Stmt::Continue => {}
        }
    }
}

impl<'a> Parser<'a> {
    fn kind(&self) -> &TokKind {
        &self.toks[self.pos].kind
    }
    fn line(&self) -> u32 {
        self.toks[self.pos].line
    }
    fn bump(&mut self) -> Token {
        let t = self.toks[self.pos].clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn expect(&mut self, k: TokKind) -> Result<(), Diag> {
        if *self.kind() == k {
            self.bump();
            Ok(())
        } else {
            Err(Diag::new(self.line(), format!("expected {:?}, found {:?}", k, self.kind())))
        }
    }
    /// `item (',' item)* ')'`, possibly empty, trailing comma allowed; '(' already consumed.
    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> Result<T, Diag>) -> Result<Vec<T>, Diag> {
        let mut xs = Vec::new();
        while !matches!(self.kind(), TokKind::RParen) {
            xs.push(item(self)?);
            if !matches!(self.kind(), TokKind::Comma) {
                break;
            }
            self.bump();
        }
        self.expect(TokKind::RParen)?;
        Ok(xs)
    }
    /// An indented block: Newline Indent item* Dedent.
    fn block<T>(&mut self, mut item: impl FnMut(&mut Self) -> Result<T, Diag>) -> Result<Vec<T>, Diag> {
        self.expect_newline()?;
        self.expect(TokKind::Indent)?;
        let mut xs = Vec::new();
        while !matches!(self.kind(), TokKind::Dedent) {
            xs.push(item(self)?);
        }
        self.expect(TokKind::Dedent)?;
        Ok(xs)
    }

    fn expect_name(&mut self) -> Result<String, Diag> {
        match self.kind().clone() {
            TokKind::Name(n) => {
                self.bump();
                Ok(n)
            }
            other => Err(Diag::new(self.line(), format!("expected identifier, found {:?}", other))),
        }
    }
    fn expect_newline(&mut self) -> Result<(), Diag> {
        if matches!(self.kind(), TokKind::Newline) {
            self.bump();
            Ok(())
        } else {
            Err(Diag::new(
                self.line(),
                format!("expected end of statement, found {:?}", self.kind()),
            ))
        }
    }
    fn eat_newlines(&mut self) {
        while matches!(self.kind(), TokKind::Newline) {
            self.pos += 1;
        }
    }

    // ---- top level ----

    fn module(&mut self) -> Result<Module, Diag> {
        let mut m = Module::default();
        let mut consts = Vec::new();
        self.eat_newlines();
        while !matches!(self.kind(), TokKind::Eof) {
            match self.kind().clone() {
                TokKind::At => m.datas.push(self.data_def()?),
                TokKind::Def => {
                    m.lines.insert(self.peek_name(), self.line());
                    m.fns.push(self.fn_def()?);
                }
                // a module docstring
                TokKind::Str(_) if self.at(1) == &TokKind::Newline => self.pos += 2,
                // `NAME = expr`: a constant
                TokKind::Name(n) if self.at(1) == &TokKind::Assign => {
                    let line = self.line();
                    self.pos += 2;
                    let e = self.expr_list()?;
                    self.expect_newline()?;
                    m.lines.insert(n.clone(), line);
                    consts.push(FnDef { name: n, params: Vec::new(), body: vec![Stmt::Return(e)] });
                }
                // `if __name__ == "__main__":` runs a script; `main()` is the entry here
                TokKind::If if self.at(1) == &TokKind::Name("__name__".into()) => self.skip_main_guard()?,
                other => {
                    return Err(Diag::new(
                        self.line(),
                        format!("unsupported top-level construct: {:?} (a module holds functions, @data classes and constants)", other),
                    ))
                }
            }
            self.eat_newlines();
        }
        m.fns.extend(consts.iter().cloned());
        resolve_globals(&mut m, &consts.iter().map(|c| c.name.clone()).collect::<Vec<_>>());
        if self.pow_used {
            m.fns.extend(parse_module(&crate::lex::lex(IPOW)?)?.fns);
        }
        Ok(m)
    }

    /// The token `k` places ahead.
    fn at(&self, k: usize) -> &TokKind {
        &self.toks[(self.pos + k).min(self.toks.len() - 1)].kind
    }

    /// The name after `def`.
    fn peek_name(&self) -> String {
        match self.at(1) {
            TokKind::Name(n) => n.clone(),
            _ => String::new(),
        }
    }

    /// Skip `if __name__ == "__main__":` and its block.
    fn skip_main_guard(&mut self) -> Result<(), Diag> {
        while !matches!(self.kind(), TokKind::Indent | TokKind::Eof) {
            self.bump();
        }
        let mut depth = 0;
        loop {
            match self.bump().kind {
                TokKind::Indent => depth += 1,
                TokKind::Dedent if depth == 1 => return Ok(()),
                TokKind::Dedent => depth -= 1,
                TokKind::Eof => return Ok(()),
                _ => {}
            }
        }
    }

    fn data_def(&mut self) -> Result<DataDef, Diag> {
        let line = self.line();
        self.bump(); // '@'
        let deco = self.expect_name()?;
        if deco != "data" {
            return Err(Diag::new(line, format!("unknown decorator '@{}'", deco)));
        }
        self.expect_newline()?;
        self.expect(TokKind::Class)?;
        let name = self.expect_name()?;
        self.expect(TokKind::Colon)?;
        let ctors = self.block(|p| {
            let cname = p.expect_name()?;
            p.expect(TokKind::Colon)?;
            p.expect(TokKind::LParen)?;
            let binds = p.list(Self::expect_name)?;
            p.expect_newline()?;
            Ok((cname, binds))
        })?;
        Ok(DataDef { name, ctors })
    }

    fn fn_def(&mut self) -> Result<FnDef, Diag> {
        self.bump(); // 'def'
        let name = self.expect_name()?;
        self.expect(TokKind::LParen)?;
        let params = self.list(Self::expect_name)?;
        self.expect(TokKind::Colon)?;
        let body = self.suite()?;
        Ok(FnDef { name, params, body })
    }

    fn suite(&mut self) -> Result<Vec<Stmt>, Diag> {
        Ok(self.block(Self::stmt)?.concat())
    }

    // ---- statements ----

    /// One statement; `for v in range(a, b)` is two (the start bound is
    /// evaluated once, before the loop).
    fn stmt(&mut self) -> Result<Vec<Stmt>, Diag> {
        Ok(vec![match self.kind().clone() {
            TokKind::If => self.if_stmt(),
            TokKind::While => self.while_stmt(),
            TokKind::For => return self.for_stmt(),
            TokKind::Match => self.match_stmt(),
            TokKind::Return => {
                self.bump();
                let e = self.expr_list()?;
                self.expect_newline()?;
                Ok(Stmt::Return(e))
            }
            TokKind::Pass | TokKind::Break | TokKind::Continue => {
                let k = self.bump().kind;
                self.expect_newline()?;
                return Ok(match k {
                    TokKind::Break => vec![Stmt::Break],
                    TokKind::Continue => vec![Stmt::Continue],
                    _ => Vec::new(),
                });
            }
            // a docstring, or any string used as a statement
            TokKind::Str(_) if self.at(1) == &TokKind::Newline => {
                self.pos += 2;
                return Ok(Vec::new());
            }
            TokKind::Name(n) if UNSUPPORTED_KEYWORDS.contains(&n.as_str()) => {
                Err(Diag::new(self.line(), format!("unsupported statement: {}", n)))
            }
            _ => return self.simple_stmt(),
        }?])
    }

    /// `e`, or `a, b = e` (tuple destructuring: a hidden name holds the
    /// tuple, each target reads one component), or `e1, e2` as a tuple.
    fn simple_stmt(&mut self) -> Result<Vec<Stmt>, Diag> {
        let line = self.line();
        let e = self.expr_list()?;
        // `x op= e` is `x = x op e`
        if let TokKind::AugAssign(op) = *self.kind() {
            self.bump();
            let rhs = self.expr()?;
            self.expect_newline()?;
            return match e {
                Expr::Var(n) => Ok(vec![Stmt::Assign(n.clone(), Expr::Bin(op, Box::new(Expr::Var(n)), Box::new(rhs)))]),
                _ => Err(Diag::new(line, "augmented assignment needs a plain name on the left")),
            };
        }
        if matches!(self.kind(), TokKind::Assign) {
            self.bump();
            let rhs = self.expr_list()?;
            self.expect_newline()?;
            let name = |t: &Expr| match t {
                Expr::Var(n) => Ok(n.clone()),
                Expr::Index(..) => Err(Diag::new(line, "assignment to an item: arrays are values, write `a = array_set(a, i, v)`")),
                _ => Err(Diag::new(line, "invalid assignment target")),
            };
            return match e {
                Expr::Tuple(targets) if !targets.is_empty() => {
                    self.hidden += 1;
                    // the name carries the count, which `infer` checks
                    let tmp = format!("__tuple{}_{}", self.hidden, targets.len());
                    let mut out = vec![Stmt::Assign(tmp.clone(), rhs)];
                    for (i, t) in targets.iter().enumerate() {
                        let proj = Expr::Index(Box::new(Expr::Var(tmp.clone())), Box::new(Expr::Int(i as i64)));
                        out.push(Stmt::Assign(name(t)?, proj));
                    }
                    Ok(out)
                }
                t => Ok(vec![Stmt::Assign(name(&t)?, rhs)]),
            };
        }
        self.expect_newline()?;
        Ok(vec![Stmt::ExprStmt(e)])
    }

    /// `e` or a bare tuple `e1, e2, ...` (as after `return` or `=`).
    fn expr_list(&mut self) -> Result<Expr, Diag> {
        let first = self.expr()?;
        if !matches!(self.kind(), TokKind::Comma) {
            return Ok(first);
        }
        let mut items = vec![first];
        while matches!(self.kind(), TokKind::Comma) {
            self.bump();
            if matches!(self.kind(), TokKind::Newline | TokKind::Assign) {
                break;
            }
            items.push(self.expr()?);
        }
        Ok(Expr::Tuple(items))
    }

    fn if_stmt(&mut self) -> Result<Stmt, Diag> {
        self.bump(); // 'if' or 'elif'
        let cond = self.expr()?;
        self.expect(TokKind::Colon)?;
        let then = self.suite()?;
        let els = self.elif_or_else()?;
        Ok(Stmt::If(cond, then, els))
    }

    fn elif_or_else(&mut self) -> Result<Vec<Stmt>, Diag> {
        match self.kind() {
            TokKind::Elif => Ok(vec![self.if_stmt()?]),
            TokKind::Else => {
                self.bump();
                self.expect(TokKind::Colon)?;
                self.suite()
            }
            _ => Ok(Vec::new()),
        }
    }

    fn while_stmt(&mut self) -> Result<Stmt, Diag> {
        self.bump(); // 'while'
        let cond = self.expr()?;
        self.expect(TokKind::Colon)?;
        let body = self.suite()?;
        Ok(Stmt::While(cond, body))
    }

    fn for_stmt(&mut self) -> Result<Vec<Stmt>, Diag> {
        let line = self.line();
        self.bump(); // 'for'
        let var = self.expect_name()?;
        self.expect(TokKind::In)?;
        let iter_name = self.expect_name()?;
        if iter_name != "range" {
            return Err(Diag::new(
                line,
                format!("unsupported for-loop iterable: {} (only range(..) is supported)", iter_name),
            ));
        }
        self.expect(TokKind::LParen)?;
        let first = self.expr()?;
        // `range(a, b)`: `a` bound once, then a loop over `range(b - a)`
        // whose body first binds the variable to `counter + a`; with a step
        // `s`, over the trip count, binding `counter * s + a`
        let mut more = Vec::new();
        while matches!(self.kind(), TokKind::Comma) && more.len() < 2 {
            self.bump();
            more.push(self.expr()?);
        }
        self.expect(TokKind::RParen)?;
        self.expect(TokKind::Colon)?;
        let mut body = self.suite()?;
        let Some(b) = more.first().cloned() else { return Ok(vec![Stmt::For(var, first, body, None)]) };
        // per-loop names: a nested loop reusing `var` must not reset the
        // outer loop's start
        self.hidden += 1;
        let (ctr, lo) = (format!("__range{}_{var}", self.hidden), format!("__lo{}_{var}", self.hidden));
        let v = |n: &str| Box::new(Expr::Var(n.to_string()));
        let bin = |op, a: Box<Expr>, b: Box<Expr>| Box::new(Expr::Bin(op, a, b));
        let mut out = vec![Stmt::Assign(lo.clone(), first)];
        let Some(step) = more.get(1).cloned() else {
            body.insert(0, Stmt::Assign(var.clone(), Expr::Bin(BinOp::Add, v(&ctr), v(&lo))));
            out.push(Stmt::For(ctr, Expr::Bin(BinOp::Sub, Box::new(b), v(&lo)), body, None));
            return Ok(out);
        };
        let st = format!("__step{}_{var}", self.hidden);
        out.push(Stmt::Assign(st.clone(), step.clone()));
        // trip count: ceil((b - a) / s) for s > 0, ceil((a - b) / -s) for s < 0
        let up = || bin(BinOp::FloorDiv, bin(BinOp::Sub, bin(BinOp::Add, bin(BinOp::Sub, Box::new(b.clone()), v(&lo)), v(&st)), Box::new(Expr::Int(1))), v(&st));
        let down = || bin(BinOp::FloorDiv, bin(BinOp::Sub, bin(BinOp::Sub, v(&lo), Box::new(b.clone())), bin(BinOp::Add, v(&st), Box::new(Expr::Int(1)))), bin(BinOp::Sub, Box::new(Expr::Int(0)), v(&st)));
        let trips = match step {
            Expr::Int(0) => return Err(Diag::new(line, "range() step must not be zero")),
            Expr::Int(k) if k > 0 => *up(),
            Expr::Int(_) => *down(),
            _ => Expr::IfExp(Box::new(Expr::Cmp(CmpOp::Gt, v(&st), Box::new(Expr::Int(0)))), up(), down()),
        };
        body.insert(0, Stmt::Assign(var.clone(), Expr::Bin(BinOp::Add, bin(BinOp::Mul, v(&ctr), v(&st)), v(&lo))));
        out.push(Stmt::For(ctr, trips, body, None));
        Ok(out)
    }

    fn match_stmt(&mut self) -> Result<Stmt, Diag> {
        self.bump(); // 'match'
        let scrut = self.expr()?;
        self.expect(TokKind::Colon)?;
        let cases = self.block(|p| {
            p.expect(TokKind::Case)?;
            let pat = p.pattern()?;
            p.expect(TokKind::Colon)?;
            Ok((pat, p.suite()?))
        })?;
        Ok(Stmt::Match(scrut, cases))
    }

    fn pattern(&mut self) -> Result<Pat, Diag> {
        let line = self.line();
        match self.kind().clone() {
            TokKind::Int(v) => {
                self.bump();
                Ok(Pat::int_lit(v))
            }
            TokKind::Name(n) => {
                self.bump();
                if matches!(self.kind(), TokKind::LParen) {
                    self.bump();
                    Ok(Pat { ctor: n, binds: self.list(Self::expect_name)? })
                } else {
                    Ok(Pat { ctor: n, binds: Vec::new() })
                }
            }
            other => Err(Diag::new(line, format!("invalid match pattern: {:?}", other))),
        }
    }

    // ---- expressions (precedence climbing, low to high) ----
    // expr -> lambda | ternary
    // ternary -> or_expr ('if' or_expr 'else' expr)?
    // or_expr -> and_expr ('or' and_expr)*
    // and_expr -> not_expr ('and' not_expr)*
    // not_expr -> 'not' not_expr | cmp_expr
    // cmp_expr -> bin_level(0) (cmpop bin_level(0))?      (non-chaining)
    // bin_level(n) -> bin_level(n+1) (op_n bin_level(n+1))*, op_0..5: | ^ & <<>> +- *///%
    // unary -> postfix          (no unary +/- in this grammar)
    // postfix -> atom ('[' expr ']')*
    // atom -> literal | name | call | tuple/paren | lambda

    fn expr(&mut self) -> Result<Expr, Diag> {
        if matches!(self.kind(), TokKind::Lambda) {
            return self.lambda();
        }
        self.ternary()
    }

    fn lambda(&mut self) -> Result<Expr, Diag> {
        self.bump(); // 'lambda'
        let mut params = Vec::new();
        if !matches!(self.kind(), TokKind::Colon) {
            params.push(self.expect_name()?);
            while matches!(self.kind(), TokKind::Comma) {
                self.bump();
                params.push(self.expect_name()?);
            }
        }
        self.expect(TokKind::Colon)?;
        let body = self.expr()?;
        Ok(Expr::Lambda(params, Box::new(body)))
    }

    fn ternary(&mut self) -> Result<Expr, Diag> {
        let e = self.or_expr()?;
        if matches!(self.kind(), TokKind::If) {
            self.bump();
            let cond = self.or_expr()?;
            self.expect(TokKind::Else)?;
            let els = self.expr()?;
            Ok(Expr::IfExp(Box::new(cond), Box::new(e), Box::new(els)))
        } else {
            Ok(e)
        }
    }

    fn or_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.and_expr()?;
        while matches!(self.kind(), TokKind::Or) {
            self.bump();
            let rhs = self.and_expr()?;
            e = Expr::Bool2(BoolOp::Or, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn and_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.not_expr()?;
        while matches!(self.kind(), TokKind::And) {
            self.bump();
            let rhs = self.not_expr()?;
            e = Expr::Bool2(BoolOp::And, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn not_expr(&mut self) -> Result<Expr, Diag> {
        if matches!(self.kind(), TokKind::Not) {
            self.bump();
            let e = self.not_expr()?;
            return Ok(Expr::Not(Box::new(e)));
        }
        self.cmp_expr()
    }

    /// `a < b < c` is `a < b and b < c` (values are immutable, so `b`
    /// is the same value both times).
    fn cmp_expr(&mut self) -> Result<Expr, Diag> {
        let mut left = self.bin_level(0)?;
        let mut out: Option<Expr> = None;
        loop {
            let op = match self.kind() {
                TokKind::Lt => CmpOp::Lt,
                TokKind::Le => CmpOp::Le,
                TokKind::Gt => CmpOp::Gt,
                TokKind::Ge => CmpOp::Ge,
                TokKind::EqEq => CmpOp::Eq,
                TokKind::NotEq => CmpOp::Ne,
                _ => return Ok(out.unwrap_or(left)),
            };
            self.bump();
            let right = self.bin_level(0)?;
            let c = Expr::Cmp(op, Box::new(left), Box::new(right.clone()));
            out = Some(match out {
                None => c,
                Some(prev) => Expr::Bool2(BoolOp::And, Box::new(prev), Box::new(c)),
            });
            left = right;
        }
    }

    /// Left-associative binary levels, loosest (0: `|`) to tightest (5: `* / // %`).
    fn bin_level(&mut self, lvl: u8) -> Result<Expr, Diag> {
        if lvl == 6 {
            return self.unary();
        }
        let mut e = self.bin_level(lvl + 1)?;
        loop {
            let op = match (lvl, self.kind()) {
                (0, TokKind::Pipe) => BinOp::BitOr,
                (1, TokKind::Caret) => BinOp::BitXor,
                (2, TokKind::Amp) => BinOp::BitAnd,
                (3, TokKind::Shl) => BinOp::Shl,
                (3, TokKind::Shr) => BinOp::Shr,
                (4, TokKind::Plus) => BinOp::Add,
                (4, TokKind::Minus) => BinOp::Sub,
                (5, TokKind::Star) => BinOp::Mul,
                (5, TokKind::Slash) => BinOp::Div,
                (5, TokKind::SlashSlash) => BinOp::FloorDiv,
                (5, TokKind::Percent) => BinOp::Mod,
                _ => return Ok(e),
            };
            self.bump();
            let rhs = self.bin_level(lvl + 1)?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
    }

    fn unary(&mut self) -> Result<Expr, Diag> {
        // unary minus binds tighter than `*` and looser than a call or
        // index (as in Python); a literal operand folds into the literal
        if matches!(self.kind(), TokKind::Minus) {
            self.bump();
            return Ok(match self.unary()? {
                Expr::Int(v) => Expr::Int(v.wrapping_neg()),
                Expr::Float(v) => Expr::Float(-v),
                e => Expr::Neg(Box::new(e)),
            });
        }
        if matches!(self.kind(), TokKind::Plus) {
            return Err(Diag::new(self.line(), "unary plus is not supported"));
        }
        self.power()
    }

    /// `a ** b`, binding tighter than unary minus on its left and right
    /// associative: a small literal exponent is repeated multiplication (on
    /// any number type), any other exponent the integer helper.
    fn power(&mut self) -> Result<Expr, Diag> {
        let base = self.postfix()?;
        if !matches!(self.kind(), TokKind::StarStar) {
            return Ok(base);
        }
        self.bump();
        Ok(match self.unary()? {
            Expr::Int(0) => Expr::Int(1),
            Expr::Int(k) if (1..=8).contains(&k) => {
                (1..k).fold(base.clone(), |acc, _| Expr::Bin(BinOp::Mul, Box::new(acc), Box::new(base.clone())))
            }
            e => {
                self.pow_used = true;
                Expr::Call("__ipow".into(), vec![base, e])
            }
        })
    }

    fn postfix(&mut self) -> Result<Expr, Diag> {
        let mut e = self.atom()?;
        while matches!(self.kind(), TokKind::LBracket) {
            self.bump();
            let idx = self.expr()?;
            self.expect(TokKind::RBracket)?;
            e = Expr::Index(Box::new(e), Box::new(idx));
        }
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr, Diag> {
        let line = self.line();
        let lit = match self.kind() {
            TokKind::Int(v) => Some(Expr::Int(*v)),
            TokKind::Float(v) => Some(Expr::Float(*v)),
            TokKind::True => Some(Expr::Bool(true)),
            TokKind::False => Some(Expr::Bool(false)),
            _ => None,
        };
        if let Some(e) = lit {
            self.bump();
            return Ok(e);
        }
        match self.kind().clone() {
            TokKind::Lambda => self.lambda(),
            TokKind::Name(n) => {
                self.bump();
                if matches!(self.kind(), TokKind::LParen) {
                    self.bump();
                    Ok(Expr::Call(n, self.list(Self::expr)?))
                } else {
                    Ok(Expr::Var(n))
                }
            }
            TokKind::LParen => {
                self.bump();
                if matches!(self.kind(), TokKind::RParen) {
                    self.bump();
                    return Ok(Expr::Tuple(Vec::new()));
                }
                let first = self.expr()?;
                if matches!(self.kind(), TokKind::Comma) {
                    let mut items = vec![first];
                    while matches!(self.kind(), TokKind::Comma) {
                        self.bump();
                        if matches!(self.kind(), TokKind::RParen) {
                            break;
                        }
                        items.push(self.expr()?);
                    }
                    self.expect(TokKind::RParen)?;
                    Ok(Expr::Tuple(items))
                } else {
                    self.expect(TokKind::RParen)?;
                    Ok(first)
                }
            }
            TokKind::Str(_) => Err(Diag::new(line, "strings are not supported: Mithril programs compute numbers, tuples, arrays and @data values")),
            other => Err(Diag::new(line, format!("unexpected token in expression: {:?}", other))),
        }
    }
}
