//! Hand-written lexer for the Python subset: indentation is significant
//! (an indent/dedent stack, like Python's own tokenizer), 4 spaces per
//! level. Tabs in leading whitespace are rejected. Numbers include decimal
//! and `0x` hex ints and simple floats. `#` starts a line comment. String
//! literals are lexed only so that a docstring can be skipped and any other
//! string reported clearly.

use crate::ast::BinOp;
use crate::Diag;


#[derive(Clone, PartialEq, Debug)]
pub enum TokKind {
    Int(i64),
    Float(f64),
    Name(String),
    Str(String),
    // Keywords
    Def,
    Return,
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Match,
    Case,
    Lambda,
    And,
    Or,
    Not,
    True,
    False,
    Class,
    Pass,
    Break,
    Continue,
    // Punctuation / operators
    Plus,
    Minus,
    Star,
    StarStar,
    Slash,
    SlashSlash,
    Percent,
    Shl,
    Shr,
    Amp,
    Pipe,
    Caret,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    NotEq,
    Assign,
    /// `+=`, `//=`, ... (the operator)
    AugAssign(BinOp),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Colon,
    Comma,
    Dot,
    At,
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Token {
    pub kind: TokKind,
    pub line: u32,
}

fn keyword(s: &str) -> Option<TokKind> {
    Some(match s {
        "def" => TokKind::Def,
        "return" => TokKind::Return,
        "if" => TokKind::If,
        "elif" => TokKind::Elif,
        "else" => TokKind::Else,
        "while" => TokKind::While,
        "for" => TokKind::For,
        "in" => TokKind::In,
        "match" => TokKind::Match,
        "case" => TokKind::Case,
        "lambda" => TokKind::Lambda,
        "and" => TokKind::And,
        "or" => TokKind::Or,
        "not" => TokKind::Not,
        "True" => TokKind::True,
        "False" => TokKind::False,
        "class" => TokKind::Class,
        "pass" => TokKind::Pass,
        "break" => TokKind::Break,
        "continue" => TokKind::Continue,
        _ => return None,
    })
}

/// Python statements/keywords we recognize by name only to produce a
/// friendlier "unsupported statement" diagnostic (see parser).
pub const UNSUPPORTED_KEYWORDS: &[&str] = &[
    "import", "from", "with", "try", "except", "finally", "raise", "yield", "global",
    "nonlocal", "del", "assert",
];

struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
    line: u32,
    indent_stack: Vec<u32>,
    out: Vec<Token>,
    /// true at the start of a logical line, before any non-whitespace.
    at_line_start: bool,
    /// Open brackets: inside them newlines and indentation are not tokens.
    paren_depth: i32,
}

pub fn lex(src: &str) -> Result<Vec<Token>, Diag> {
    let mut lx = Lexer {
        src: src.as_bytes(),
        pos: 0,
        line: 1,
        indent_stack: vec![0],
        out: Vec::new(),
        at_line_start: true,
        paren_depth: 0,
    };
    lx.run()?;
    Ok(lx.out)
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> u8 {
        *self.src.get(self.pos).unwrap_or(&0)
    }
    fn peek_at(&self, off: usize) -> u8 {
        *self.src.get(self.pos + off).unwrap_or(&0)
    }
    fn bump(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        c
    }

    fn run(&mut self) -> Result<(), Diag> {
        loop {
            if self.peek() == 0 {
                break;
            }
            if self.at_line_start && self.paren_depth == 0 {
                if self.handle_indent()? {
                    // blank/comment-only line: keep scanning at line start
                    continue;
                }
            }
            let c = self.peek();
            if c == b' ' || c == b'\t' {
                self.pos += 1;
                continue;
            }
            if c == b'#' {
                while self.peek() != b'\n' && self.peek() != 0 {
                    self.pos += 1;
                }
                continue;
            }
            if c == b'\n' {
                let line = self.line;
                self.bump();
                if self.paren_depth == 0 {
                    self.out.push(Token { kind: TokKind::Newline, line });
                    self.at_line_start = true;
                }
                continue;
            }
            if c == b'\r' {
                self.pos += 1;
                continue;
            }
            if c.is_ascii_digit() {
                self.lex_number()?;
                continue;
            }
            if c.is_ascii_alphabetic() || c == b'_' {
                self.lex_name();
                continue;
            }
            if c == b'"' || c == b'\'' {
                self.lex_string()?;
                continue;
            }
            self.lex_op()?;
        }
        // Final NEWLINE (if the file didn't end with one) then dedent to 0.
        if !matches!(self.out.last().map(|t| &t.kind), Some(TokKind::Newline) | None) {
            self.out.push(Token { kind: TokKind::Newline, line: self.line });
        }
        while *self.indent_stack.last().unwrap() > 0 {
            self.indent_stack.pop();
            self.out.push(Token { kind: TokKind::Dedent, line: self.line });
        }
        self.out.push(Token { kind: TokKind::Eof, line: self.line });
        Ok(())
    }

    /// Consumes leading whitespace of a logical line and emits
    /// Indent/Dedent tokens as needed. Returns `true` if the line was blank
    /// or comment-only (caller should keep scanning at line start).
    fn handle_indent(&mut self) -> Result<bool, Diag> {
        let line = self.line;
        let mut col: u32 = 0;
        loop {
            match self.peek() {
                b' ' => {
                    col += 1;
                    self.pos += 1;
                }
                b'\t' => {
                    return Err(Diag::new(line, "tab indentation is not allowed; use 4 spaces"));
                }
                _ => break,
            }
        }
        // Blank line or comment-only line: no indent change, no NEWLINE.
        if self.peek() == b'\n' {
            self.bump();
            return Ok(true);
        }
        if self.peek() == b'#' {
            while self.peek() != b'\n' && self.peek() != 0 {
                self.pos += 1;
            }
            if self.peek() == b'\n' {
                self.bump();
            }
            return Ok(true);
        }
        if self.peek() == 0 {
            return Ok(true);
        }
        self.at_line_start = false;
        let top = *self.indent_stack.last().unwrap();
        if col > top {
            self.indent_stack.push(col);
            self.out.push(Token { kind: TokKind::Indent, line });
        } else if col < top {
            while *self.indent_stack.last().unwrap() > col {
                self.indent_stack.pop();
                self.out.push(Token { kind: TokKind::Dedent, line });
            }
            if *self.indent_stack.last().unwrap() != col {
                return Err(Diag::new(line, "mismatched dedent"));
            }
        }
        Ok(false)
    }

    fn lex_number(&mut self) -> Result<(), Diag> {
        let line = self.line;
        let start = self.pos;
        if self.peek() == b'0' && (self.peek_at(1) == b'x' || self.peek_at(1) == b'X') {
            self.pos += 2;
            let hstart = self.pos;
            while self.peek().is_ascii_hexdigit() {
                self.pos += 1;
            }
            let text = std::str::from_utf8(&self.src[hstart..self.pos]).unwrap();
            let v = u64::from_str_radix(text, 16)
                .map_err(|_| Diag::new(line, "invalid hex literal"))?;
            if v > i64::MAX as u64 {
                return Err(Diag::new(line, "int literal out of range"));
            }
            self.out.push(Token { kind: TokKind::Int(v as i64), line });
            return Ok(());
        }
        while self.peek().is_ascii_digit() {
            self.pos += 1;
        }
        let mut is_float = false;
        if self.peek() == b'.' && self.peek_at(1).is_ascii_digit() {
            is_float = true;
            self.pos += 1;
            while self.peek().is_ascii_digit() {
                self.pos += 1;
            }
        }
        // an exponent: `1e-9`, `2.5E3`
        let sign = matches!(self.peek_at(1), b'+' | b'-') as usize;
        if matches!(self.peek(), b'e' | b'E') && self.peek_at(1 + sign).is_ascii_digit() {
            is_float = true;
            self.pos += 1 + sign;
            while self.peek().is_ascii_digit() {
                self.pos += 1;
            }
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        if is_float {
            let v: f64 = text.parse().map_err(|_| Diag::new(line, "invalid float literal"))?;
            self.out.push(Token { kind: TokKind::Float(v), line });
        } else {
            let v: i128 = text.parse().map_err(|_| Diag::new(line, "invalid int literal"))?;
            if v > i64::MAX as i128 {
                return Err(Diag::new(line, "int literal out of range"));
            }
            self.out.push(Token { kind: TokKind::Int(v as i64), line });
        }
        Ok(())
    }

    fn lex_name(&mut self) {
        let line = self.line;
        let start = self.pos;
        while self.peek().is_ascii_alphanumeric() || self.peek() == b'_' {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap().to_string();
        let kind = keyword(&text).unwrap_or(TokKind::Name(text));
        self.out.push(Token { kind, line });
    }

    /// `'...'`, `"..."` or a triple-quoted string (which may span lines).
    fn lex_string(&mut self) -> Result<(), Diag> {
        let line = self.line;
        let q = self.bump();
        let triple = self.peek() == q && self.peek_at(1) == q;
        if triple {
            self.pos += 2;
        }
        let start = self.pos;
        loop {
            match self.peek() {
                0 => return Err(Diag::new(line, "unterminated string literal")),
                b'\\' => {
                    self.bump();
                    self.bump();
                }
                b'\n' if !triple => return Err(Diag::new(line, "unterminated string literal")),
                c if c == q && (!triple || (self.peek_at(1) == q && self.peek_at(2) == q)) => break,
                _ => {
                    self.bump();
                }
            }
        }
        let text = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        self.pos += if triple { 3 } else { 1 };
        self.out.push(Token { kind: TokKind::Str(text), line });
        Ok(())
    }

    fn eat(&mut self, c: u8) -> bool {
        let y = self.peek() == c;
        if y {
            self.pos += 1;
        }
        y
    }

    fn lex_op(&mut self) -> Result<(), Diag> {
        use TokKind::*;
        let line = self.line;
        let c = self.bump();
        // `op=`: an augmented assignment (`==`, `<=`, `>=` and `!=` are comparisons)
        let aug = |lx: &mut Self, op: BinOp| if lx.eat(b'=') { Some(AugAssign(op)) } else { None };
        let kind = match c {
            b'(' | b'[' => {
                self.paren_depth += 1;
                if c == b'(' { LParen } else { LBracket }
            }
            b')' | b']' => {
                self.paren_depth -= 1;
                if c == b')' { RParen } else { RBracket }
            }
            b':' => Colon,
            b',' => Comma,
            b'.' => Dot,
            b'@' => At,
            b'+' => aug(self, BinOp::Add).unwrap_or(Plus),
            b'-' => aug(self, BinOp::Sub).unwrap_or(Minus),
            b'*' if self.eat(b'*') => StarStar,
            b'*' => aug(self, BinOp::Mul).unwrap_or(Star),
            b'%' => aug(self, BinOp::Mod).unwrap_or(Percent),
            b'&' => aug(self, BinOp::BitAnd).unwrap_or(Amp),
            b'|' => aug(self, BinOp::BitOr).unwrap_or(Pipe),
            b'^' => aug(self, BinOp::BitXor).unwrap_or(Caret),
            b'/' if self.eat(b'/') => aug(self, BinOp::FloorDiv).unwrap_or(SlashSlash),
            b'/' => aug(self, BinOp::Div).unwrap_or(Slash),
            b'<' if self.eat(b'<') => aug(self, BinOp::Shl).unwrap_or(Shl),
            b'<' => if self.eat(b'=') { Le } else { Lt },
            b'>' if self.eat(b'>') => aug(self, BinOp::Shr).unwrap_or(Shr),
            b'>' => if self.eat(b'=') { Ge } else { Gt },
            b'=' => if self.eat(b'=') { EqEq } else { Assign },
            b'!' if self.eat(b'=') => NotEq,
            other => return Err(Diag::new(line, format!("unexpected character '{}'", other as char))),
        };
        self.out.push(Token { kind, line });
        Ok(())
    }
}
