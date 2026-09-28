//! Agent encodings and cell-chain helpers shared by every reducer of the
//! net: the compile-time reducer (`mithril-net`) and the runtime engine
//! (`mithril-rt`) build and read the same cells.
//!
//! - `Var(w)`   — one end of a *wire*: cell `w` starts `[EMPTY, EMPTY]`;
//!   the first port linked to it is stored in slot 0.
//! - `Op(a,code)` — payload `addr:40|code:16`; cell `[other operand, ret]`.
//! - `Swi(a)`   — cell `[ret, Ext(arms)]`, arms cell `[then Ref, else Ref]`.
//! - `Mat(a,id)`— payload `addr:40|match_id:16`; cell `[ret, arm list]`.
//! - `Ref(h,f)` — payload `head+1:40|entry:16`; a (possibly partial)
//!   saturated call: `h` heads a chain of argument list cells.
//! - `Dup(a,l)` — payload `addr:32|label:24`; cell = the two copy targets.
//! - `Lam(a)`/`App(a)` — cells `[param, body]` / `[arg, ret]`.
//! - `Ext`      — internal chain pointers (`Ext(addr)`) and the `EMPTY`
//!   sentinel (all-ones payload); never a redex side.
//!
//! List cells (`Ref` args, `Mat` arms) are `[item, next]` with `next` either
//! `Ext(addr)` or `EMPTY`. Constructor chains: the 4-bit arity field is the
//! *remaining* field count, saturating at 15.

use crate::net::Net;
use crate::port::{Port, Tag};

/// The store a reducer rewrites: cells of two ports, a redex worklist and
/// a label supply. `Net` is the compile-time one; the runtime arena
/// implements it per worker.
pub trait Cells {
    fn cell(&self, i: u32) -> [u64; 2];
    fn set(&mut self, i: u32, slot: usize, p: Port);
    fn alloc(&mut self, a: Port, b: Port) -> u32;
    fn free_cell(&mut self, i: u32);
    /// Queue a pair of non-Var ports that met.
    fn push_redex(&mut self, a: Port, b: Port);
    /// A fresh Dup label (a new sharing site).
    fn fresh_label(&mut self) -> u32;
}

impl Cells for Net {
    fn cell(&self, i: u32) -> [u64; 2] {
        Net::cell(self, i)
    }
    fn set(&mut self, i: u32, slot: usize, p: Port) {
        Net::set(self, i, slot, p)
    }
    fn alloc(&mut self, a: Port, b: Port) -> u32 {
        Net::alloc(self, a, b)
    }
    fn free_cell(&mut self, i: u32) {
        Net::free_cell(self, i)
    }
    fn push_redex(&mut self, a: Port, b: Port) {
        self.redexes.push((a, b));
    }
    fn fresh_label(&mut self) -> u32 {
        let l = self.labels;
        self.labels = self.labels.wrapping_add(1) & 0xFF_FFFF;
        if self.labels == 0 {
            self.labels = 1;
        }
        l
    }
}

/// An unfilled wire slot and the terminator of a list chain. Encoded as
/// `Ext` with an all-ones payload so it cannot collide with a real
/// `Ext(addr)` chain pointer.
pub const EMPTY: Port = Port(((Tag::Ext as u64) << 56) | ((1u64 << 56) - 1));

/// 12-bit constructor tag reserved for tuples.
pub const CTAG_TUPLE: u16 = 0xFFF;
/// 12-bit constructor tag for desugar's unreachable-match sentinel.
pub const CTAG_UNREACHABLE: u16 = 0xFFE;

/// "Operands swapped" flag of an Op code: slot 0 holds the *first* operand.
pub const OP_FLIP: u16 = 1 << 8;

/// Erasure port.
pub fn era() -> Port {
    Port::new(Tag::Era, 0)
}

/// Allocate a fresh wire cell and return one of its (interchangeable) ends.
pub fn wire<C: Cells>(c: &mut C) -> Port {
    let w = c.alloc(EMPTY, EMPTY);
    Port::new(Tag::Var, w as u64)
}

pub fn op_port(addr: u32, code: u16) -> Port {
    Port::new(Tag::Op, ((addr as u64) << 16) | code as u64)
}
pub fn op_addr(p: Port) -> u32 {
    (p.payload() >> 16) as u32
}
pub fn op_code(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

pub fn dup_port(addr: u32, label: u32) -> Port {
    Port::new(Tag::Dup, ((addr as u64) << 24) | (label & 0xFF_FFFF) as u64)
}
pub fn dup_addr(p: Port) -> u32 {
    (p.payload() >> 24) as u32
}
pub fn dup_label(p: Port) -> u32 {
    (p.payload() & 0xFF_FFFF) as u32
}

pub fn mat_port(addr: u32, match_id: u16) -> Port {
    Port::new(Tag::Mat, ((addr as u64) << 16) | match_id as u64)
}
pub fn mat_addr(p: Port) -> u32 {
    (p.payload() >> 16) as u32
}
pub fn mat_id(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

pub fn ref_port(head: Port, entry: u16) -> Port {
    let h = if head == EMPTY { 0 } else { head.payload() + 1 };
    Port::new(Tag::Ref, (h << 16) | entry as u64)
}
pub fn ref_head(p: Port) -> Port {
    let h = p.payload() >> 16;
    if h == 0 {
        EMPTY
    } else {
        Port::new(Tag::Ext, h - 1)
    }
}
pub fn ref_entry(p: Port) -> u16 {
    (p.payload() & 0xFFFF) as u16
}

// ---- list chains ([item, Ext(next)|EMPTY] cells) ----

pub fn list_alloc<C: Cells>(c: &mut C, items: &[Port]) -> Port {
    let mut head = EMPTY;
    for &it in items.iter().rev() {
        let a = c.alloc(it, head);
        head = Port::new(Tag::Ext, a as u64);
    }
    head
}

/// The items of a list chain, without freeing it.
pub fn list_items<C: Cells>(c: &C, mut head: Port) -> Vec<Port> {
    let mut out = Vec::new();
    while head != EMPTY {
        debug_assert_eq!(head.tag(), Tag::Ext);
        let cell = c.cell(head.payload() as u32);
        out.push(Port(cell[0]));
        head = Port(cell[1]);
    }
    out
}

/// Walk and free a list chain, returning the items in order.
pub fn list_collect<C: Cells>(c: &mut C, mut head: Port) -> Vec<Port> {
    let mut out = Vec::new();
    while head != EMPTY {
        debug_assert_eq!(head.tag(), Tag::Ext);
        let a = head.payload() as u32;
        let cell = c.cell(a);
        c.free_cell(a);
        out.push(Port(cell[0]));
        head = Port(cell[1]);
    }
    out
}

// ---- constructor chains ----

/// A constructor of `fields`: arity <= 2 in one cell, wider ones chained
/// (`[field, rest]`) with the arity nibble saturating at 15.
pub fn con_alloc<C: Cells>(c: &mut C, ctag: u16, fields: &[Port]) -> Port {
    let n = fields.len();
    match n {
        0 => Port::con(0, ctag, 0),
        1 => {
            let a = c.alloc(fields[0], EMPTY);
            Port::con(a as u64, ctag, 1)
        }
        2 => {
            let a = c.alloc(fields[0], fields[1]);
            Port::con(a as u64, ctag, 2)
        }
        _ => {
            let rest = con_alloc(c, ctag, &fields[1..]);
            let a = c.alloc(fields[0], rest);
            Port::con(a as u64, ctag, n.min(15) as u8)
        }
    }
}

/// Walk and free a constructor chain, returning the field ports in order.
pub fn con_collect<C: Cells>(c: &mut C, mut p: Port) -> Vec<Port> {
    let mut out = Vec::new();
    loop {
        let n = p.con_arity();
        let a = p.con_addr() as u32;
        match n {
            0 => break,
            1 => {
                let cell = c.cell(a);
                c.free_cell(a);
                out.push(Port(cell[0]));
                break;
            }
            2 => {
                let cell = c.cell(a);
                c.free_cell(a);
                out.push(Port(cell[0]));
                out.push(Port(cell[1]));
                break;
            }
            _ => {
                let cell = c.cell(a);
                c.free_cell(a);
                out.push(Port(cell[0]));
                p = Port(cell[1]);
                debug_assert_eq!(p.tag(), Tag::Con);
            }
        }
    }
    out
}
