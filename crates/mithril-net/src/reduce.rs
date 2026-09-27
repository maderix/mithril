//! Compile-time reducer (fuel-bounded, deterministic FIFO worklist) and
//! readback of fully-reduced values.

use crate::rules::{flo_bits, process};
use crate::{NetProg, CTAG_TUPLE, CTAG_UNREACHABLE, EMPTY};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::core::{CoreModule, Val};

/// Run interaction rules on the net's redex worklist, in FIFO order, until
/// quiescence or until `fuel` rewrites have been performed (pure wiring
/// steps are free but only happen while fuel remains, so `fuel == 0`
/// leaves the net untouched). Unprocessed redexes stay queued, so a
/// fuel-starved reduction can be resumed by calling `reduce` again.
/// Returns the number of rewrites performed.
pub fn reduce(net: &mut Net, m: &CoreModule, fuel: u64) -> u64 {
    let prog = NetProg::new(m);
    let mut done = 0u64;
    let mut i = 0usize;
    while done < fuel && i < net.redexes.len() {
        let (a, b) = net.redexes[i];
        i += 1;
        done += process(net, &prog, a, b);
    }
    net.redexes.drain(..i);
    done
}

/// Follow filled wires without consuming them (readback is `&Net`).
fn peek(net: &Net, mut p: Port) -> Port {
    while p.tag() == Tag::Var {
        let c = net.cell(p.payload() as u32);
        if c[0] == EMPTY.0 {
            return p;
        }
        p = Port(c[0]);
    }
    p
}

/// Read a fully-reduced value: NUM -> `Val::I` (sign-extended i56), FLO
/// cell -> `Val::F`, CON chains -> `Val::C`/`Val::T`. Anything else (an
/// unfilled wire, a residual agent, the unreachable-match sentinel) is not
/// a value: `None`.
pub fn readback(net: &Net, root: Port) -> Option<Val> {
    let p = peek(net, root);
    match p.tag() {
        Tag::Num => Some(Val::I(p.as_i64())),
        Tag::Flo => Some(Val::F(f64::from_bits(flo_bits(net.cell(p.payload() as u32))))),
        Tag::Con => read_con(net, p),
        _ => None,
    }
}

fn read_con(net: &Net, mut p: Port) -> Option<Val> {
    let ctag = p.con_tag();
    if ctag == CTAG_UNREACHABLE {
        return None;
    }
    let mut fields = Vec::new();
    loop {
        let n = p.con_arity();
        let a = p.con_addr() as u32;
        match n {
            0 => break,
            1 => {
                fields.push(readback(net, Port(net.cell(a)[0]))?);
                break;
            }
            2 => {
                let c = net.cell(a);
                fields.push(readback(net, Port(c[0]))?);
                fields.push(readback(net, Port(c[1]))?);
                break;
            }
            _ => {
                let c = net.cell(a);
                fields.push(readback(net, Port(c[0]))?);
                p = Port(c[1]);
                if p.tag() != Tag::Con {
                    return None;
                }
            }
        }
    }
    if ctag == CTAG_TUPLE {
        Some(Val::T(fields))
    } else {
        Some(Val::C(ctag as u32, fields))
    }
}
