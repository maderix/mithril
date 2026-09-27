//! Mithril front end: lexer, parser (surface AST), desugarer (Core IR) and
//! a reference interpreter over Core.

pub mod ast;
pub mod core;
pub mod desugar;
pub mod lex;
pub mod parse;

pub use ast::Module;
pub use core::{eval_core, CoreModule, Val};
pub use desugar::desugar;

/// A single diagnostic: a source line and a human-readable message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diag {
    pub line: u32,
    pub msg: String,
}

impl Diag {
    pub fn new(line: u32, msg: impl Into<String>) -> Diag {
        Diag { line, msg: msg.into() }
    }
}

/// Parse Mithril source into a surface `Module`.
pub fn parse(src: &str) -> Result<Module, Diag> {
    let toks = lex::lex(src)?;
    parse::parse_module(&toks)
}
