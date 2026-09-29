//! Recursive-descent parser with precedence climbing over the token
//! stream produced by `lex`.

use crate::ast::*;
use crate::lex::{TokKind, Token, UNSUPPORTED_KEYWORDS};
use crate::Diag;

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

pub fn parse_module(toks: &[Token]) -> Result<Module, Diag> {
    let mut p = Parser { toks, pos: 0 };
    p.module()
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
    fn names_to_rparen(&mut self) -> Result<Vec<String>, Diag> {
        let mut binds = Vec::new();
        while !matches!(self.kind(), TokKind::RParen) {
            binds.push(self.expect_name()?);
            if matches!(self.kind(), TokKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(TokKind::RParen)?;
        Ok(binds)
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
        self.eat_newlines();
        while !matches!(self.kind(), TokKind::Eof) {
            match self.kind() {
                TokKind::At => m.datas.push(self.data_def()?),
                TokKind::Def => m.fns.push(self.fn_def()?),
                other => {
                    return Err(Diag::new(
                        self.line(),
                        format!("unsupported top-level construct: {:?}", other),
                    ))
                }
            }
            self.eat_newlines();
        }
        Ok(m)
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
        self.expect_newline()?;
        self.expect(TokKind::Indent)?;
        let mut ctors = Vec::new();
        while !matches!(self.kind(), TokKind::Dedent) {
            let cname = self.expect_name()?;
            self.expect(TokKind::Colon)?;
            self.expect(TokKind::LParen)?;
            let binds = self.names_to_rparen()?;
            self.expect_newline()?;
            ctors.push((cname, binds));
        }
        self.expect(TokKind::Dedent)?;
        Ok(DataDef { name, ctors })
    }

    fn fn_def(&mut self) -> Result<FnDef, Diag> {
        self.bump(); // 'def'
        let name = self.expect_name()?;
        self.expect(TokKind::LParen)?;
        let mut params = Vec::new();
        while !matches!(self.kind(), TokKind::RParen) {
            params.push(self.expect_name()?);
            if matches!(self.kind(), TokKind::Comma) {
                self.bump();
            } else {
                break;
            }
        }
        self.expect(TokKind::RParen)?;
        self.expect(TokKind::Colon)?;
        let body = self.suite()?;
        Ok(FnDef { name, params, body })
    }

    fn suite(&mut self) -> Result<Vec<Stmt>, Diag> {
        self.expect_newline()?;
        self.expect(TokKind::Indent)?;
        let mut stmts = Vec::new();
        while !matches!(self.kind(), TokKind::Dedent) {
            stmts.extend(self.stmt()?);
        }
        self.expect(TokKind::Dedent)?;
        Ok(stmts)
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
                let e = self.expr()?;
                self.expect_newline()?;
                Ok(Stmt::Return(e))
            }
            TokKind::Name(n) if UNSUPPORTED_KEYWORDS.contains(&n.as_str()) => {
                Err(Diag::new(self.line(), format!("unsupported statement: {}", n)))
            }
            _ => self.simple_stmt(),
        }?])
    }

    fn simple_stmt(&mut self) -> Result<Stmt, Diag> {
        let line = self.line();
        let e = self.expr()?;
        if matches!(self.kind(), TokKind::Assign) {
            self.bump();
            let rhs = self.expr()?;
            self.expect_newline()?;
            return match e {
                Expr::Var(name) => Ok(Stmt::Assign(name, rhs)),
                _ => Err(Diag::new(line, "invalid assignment target")),
            };
        }
        self.expect_newline()?;
        Ok(Stmt::ExprStmt(e))
    }

    fn if_stmt(&mut self) -> Result<Stmt, Diag> {
        self.bump(); // 'if'
        let cond = self.expr()?;
        self.expect(TokKind::Colon)?;
        let then = self.suite()?;
        let els = self.elif_or_else()?;
        Ok(Stmt::If(cond, then, els))
    }

    fn elif_or_else(&mut self) -> Result<Vec<Stmt>, Diag> {
        match self.kind() {
            TokKind::Elif => {
                self.bump();
                let cond = self.expr()?;
                self.expect(TokKind::Colon)?;
                let then = self.suite()?;
                let els = self.elif_or_else()?;
                Ok(vec![Stmt::If(cond, then, els)])
            }
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
        // whose body first binds the variable to `counter + a`
        let start = if matches!(self.kind(), TokKind::Comma) {
            self.bump();
            let end = self.expr()?;
            Some((first.clone(), end))
        } else {
            None
        };
        self.expect(TokKind::RParen)?;
        self.expect(TokKind::Colon)?;
        let mut body = self.suite()?;
        match start {
            None => Ok(vec![Stmt::For(var, first, body, None)]),
            Some((a, b)) => {
                let (ctr, lo) = (format!("__range_{var}"), format!("__lo_{var}"));
                let lo_v = || Box::new(Expr::Var(lo.clone()));
                body.insert(0, Stmt::Assign(var.clone(), Expr::Bin(BinOp::Add, Box::new(Expr::Var(ctr.clone())), lo_v())));
                Ok(vec![Stmt::Assign(lo.clone(), a), Stmt::For(ctr, Expr::Bin(BinOp::Sub, Box::new(b), lo_v()), body, None)])
            }
        }
    }

    fn match_stmt(&mut self) -> Result<Stmt, Diag> {
        self.bump(); // 'match'
        let scrut = self.expr()?;
        self.expect(TokKind::Colon)?;
        self.expect_newline()?;
        self.expect(TokKind::Indent)?;
        let mut cases = Vec::new();
        while !matches!(self.kind(), TokKind::Dedent) {
            self.expect(TokKind::Case)?;
            let pat = self.pattern()?;
            self.expect(TokKind::Colon)?;
            let body = self.suite()?;
            cases.push((pat, body));
        }
        self.expect(TokKind::Dedent)?;
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
                    let binds = self.names_to_rparen()?;
                    Ok(Pat { ctor: n, binds })
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
    // cmp_expr -> bitor_expr (cmpop bitor_expr)?      (non-chaining)
    // bitor_expr -> bitxor_expr ('|' bitxor_expr)*
    // bitxor_expr -> bitand_expr ('^' bitand_expr)*
    // bitand_expr -> shift_expr ('&' shift_expr)*
    // shift_expr -> addsub_expr (('<<'|'>>') addsub_expr)*
    // addsub_expr -> muldiv_expr (('+'|'-') muldiv_expr)*
    // muldiv_expr -> unary (('*'|'/'|'//'|'%') unary)*
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

    fn cmp_expr(&mut self) -> Result<Expr, Diag> {
        let e = self.bitor_expr()?;
        let op = match self.kind() {
            TokKind::Lt => Some(CmpOp::Lt),
            TokKind::Le => Some(CmpOp::Le),
            TokKind::Gt => Some(CmpOp::Gt),
            TokKind::Ge => Some(CmpOp::Ge),
            TokKind::EqEq => Some(CmpOp::Eq),
            TokKind::NotEq => Some(CmpOp::Ne),
            _ => None,
        };
        match op {
            Some(op) => {
                self.bump();
                let rhs = self.bitor_expr()?;
                Ok(Expr::Cmp(op, Box::new(e), Box::new(rhs)))
            }
            None => Ok(e),
        }
    }

    fn bitor_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.bitxor_expr()?;
        while matches!(self.kind(), TokKind::Pipe) {
            self.bump();
            let rhs = self.bitxor_expr()?;
            e = Expr::Bin(BinOp::BitOr, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn bitxor_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.bitand_expr()?;
        while matches!(self.kind(), TokKind::Caret) {
            self.bump();
            let rhs = self.bitand_expr()?;
            e = Expr::Bin(BinOp::BitXor, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn bitand_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.shift_expr()?;
        while matches!(self.kind(), TokKind::Amp) {
            self.bump();
            let rhs = self.shift_expr()?;
            e = Expr::Bin(BinOp::BitAnd, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn shift_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.addsub_expr()?;
        loop {
            let op = match self.kind() {
                TokKind::Shl => BinOp::Shl,
                TokKind::Shr => BinOp::Shr,
                _ => break,
            };
            self.bump();
            let rhs = self.addsub_expr()?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn addsub_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.muldiv_expr()?;
        loop {
            let op = match self.kind() {
                TokKind::Plus => BinOp::Add,
                TokKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.muldiv_expr()?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn muldiv_expr(&mut self) -> Result<Expr, Diag> {
        let mut e = self.unary()?;
        loop {
            let op = match self.kind() {
                TokKind::Star => BinOp::Mul,
                TokKind::Slash => BinOp::Div,
                TokKind::SlashSlash => BinOp::FloorDiv,
                TokKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let rhs = self.unary()?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn unary(&mut self) -> Result<Expr, Diag> {
        // Unary +/- is not part of this grammar (see brief: decline
        // anything not listed); only binary +/- and unary `not` exist.
        if matches!(self.kind(), TokKind::Minus) {
            return Err(Diag::new(self.line(), "unary minus is not supported"));
        }
        if matches!(self.kind(), TokKind::Plus) {
            return Err(Diag::new(self.line(), "unary plus is not supported"));
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Result<Expr, Diag> {
        let mut e = self.atom()?;
        loop {
            if matches!(self.kind(), TokKind::LBracket) {
                self.bump();
                let idx = self.expr()?;
                self.expect(TokKind::RBracket)?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn atom(&mut self) -> Result<Expr, Diag> {
        let line = self.line();
        match self.kind().clone() {
            TokKind::Int(v) => {
                self.bump();
                Ok(Expr::Int(v))
            }
            TokKind::Float(v) => {
                self.bump();
                Ok(Expr::Float(v))
            }
            TokKind::True => {
                self.bump();
                Ok(Expr::Bool(true))
            }
            TokKind::False => {
                self.bump();
                Ok(Expr::Bool(false))
            }
            TokKind::Lambda => self.lambda(),
            TokKind::Name(n) => {
                self.bump();
                if matches!(self.kind(), TokKind::LParen) {
                    self.bump();
                    let mut args = Vec::new();
                    while !matches!(self.kind(), TokKind::RParen) {
                        args.push(self.expr()?);
                        if matches!(self.kind(), TokKind::Comma) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    self.expect(TokKind::RParen)?;
                    Ok(Expr::Call(n, args))
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
            other => Err(Diag::new(line, format!("unexpected token in expression: {:?}", other))),
        }
    }
}
