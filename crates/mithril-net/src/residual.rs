//! Residual readback: a quiesced-but-not-fully-reduced net back into Core.
//!
//! After specialization (`specialize`) a function's net holds a residual
//! program: agents parked on unknown inputs (an `Op` whose operand is a
//! parameter, a `Swi`/`Mat` on an unknown value, a `Dup` fanning an unknown
//! value out), calls kept as calls, opaque primitives, and the values that
//! did reduce. Cells are untyped, so readback first walks the net from its
//! roots (the result wire, the parameter wires, the residual pairs) with the
//! port tags in hand, classifying every live cell and recording, for every
//! wire, which agent produces the value flowing through it. The expression
//! tree is then read from the result wire down, producer by producer. A
//! `Dup` is a shared value: its input is read once into a fresh let-bound
//! variable (hoisted to the function head; every `Dup` in a residual net
//! comes from a strict, unconditionally evaluated binding, so hoisting only
//! reorders unconditional pure work) and each output reads the variable.
//! A pending call is placed the same way: bound in the frame the net
//! created it in, not where its result is first used (two independent
//! calls stay independent), and read in place when created where it is
//! used (tail calls stay tail calls). One rule places every bound value
//! (`place`): arms above the frame a value was created in are skipped, and
//! a closure keeps the value exactly when the value reads something the
//! closure binds; otherwise every application shares it. Parked branch closures splice the original Core of their lifted
//! entry under lets for the captured arguments.

use crate::{addr_of, cid_of, con_fields, dup_label,
    dup_addr, list_items, mat_addr, mat_id, op_addr, op_code, ref_entry, ref_head, MatchMeta, NetProg,
    CTAG_TUPLE, EMPTY, OP_FLIP, PRIM_BASE,
};
use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};
use mithril_front::ast::{BinOp, CmpOp};
use mithril_front::core::{Core, Prim};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Ports are already well-mixed 64-bit words: a multiplicative hash is
/// enough for the scan's visited set (SipHash dominated the scan).
#[derive(Default)]
pub(crate) struct PortHasher(u64);

impl Hasher for PortHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0 ^ n).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        self.0 ^= self.0 >> 29;
    }
}

pub(crate) type PortSet = HashSet<u64, BuildHasherDefault<PortHasher>>;

/// What produces the value on a wire.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Producer {
    /// an unknown input bound to a Core variable (a parameter, a match binder)
    Free(u32),
    Op(u32),
    Swi(u32),
    Mat(u32),
    Dup(u32),
    /// a call kept as a call: the Ref port and its result wire's cell (the
    /// wire is unique per residual call; the port is not: every nullary
    /// call to one function has the same port)
    Ref(Port, u32),
    /// an op that cannot fold at compile time: (op with cell [x, ret], y)
    ResOp(Port, Port),
    /// the parameter wire of a residual closure (the Lam cell)
    LamParam(u32),
    /// an application parked on an unknown function: the App cell
    App(u32),
}

/// Dense per-cell tables (the scan runs thousands of times per function).
pub(crate) struct Index {
    /// union-find over wire cells (two wires linked end to end are one):
    /// parent per cell, itself for a root
    uf: Vec<u32>,
    /// the non-Var port stored in a wire class, if any
    stored: Vec<Option<Port>>,
    pub(crate) producer: Vec<Option<Producer>>,
    /// agent address -> the wire class its port is stored in (its input)
    input_of: Vec<Option<u32>>,
    /// parked Swi/Mat agents: (cell, is_match) in discovery order
    pub(crate) parked: Vec<(u32, bool)>,
    /// label of every Dup cell met
    pub(crate) dup_labels: HashMap<u32, u32>,
    /// pending calls (by result wire) and Dup cells reached only inside an
    /// unapplied closure's body: no frame stamps them until the closure is
    /// applied (the net cannot fire them before)
    pub(crate) lam_only: HashSet<u32>,
    /// for a wire that is an output of a Dup cell: (cell, side)
    pub(crate) dup_side: HashMap<u32, (u32, usize)>,
    /// a chain link: the Dup cell it takes its input from, and the side
    pub(crate) dup_parent: HashMap<u32, (u32, usize)>,
    /// labels of dups met as superpositions (stored in a wire as the
    /// consumer of nothing: their sides are the sources)
    pub(crate) sup_labels: HashSet<u32>,
}

impl Index {
    fn find(&mut self, w: u32) -> u32 {
        let mut r = w;
        while self.uf[r as usize] != r {
            r = self.uf[r as usize];
        }
        // path compression
        let mut c = w;
        while self.uf[c as usize] != r {
            let p = self.uf[c as usize];
            self.uf[c as usize] = r;
            c = p;
        }
        r
    }
    fn union(&mut self, a: u32, b: u32) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.uf[ra as usize] = rb;
        }
    }
    /// The producers recorded, for callers walking every wire class.
    pub(crate) fn producers(&self) -> impl Iterator<Item = &Producer> {
        self.producer.iter().flatten()
    }

    /// Whether the value at `p` depends on an unapplied closure's parameter.
    fn needs_param(&self, net: &Net, p: Port, memo: &mut HashMap<u32, bool>) -> bool {
        let wire = |w: u32| Port::new(Tag::Var, w as u64);
        match p.tag() {
            Tag::Con => con_fields(net, p).into_iter().any(|f| self.needs_param(net, f, memo)),
            Tag::Ref => list_items(net, ref_head(p)).into_iter().any(|a| self.needs_param(net, a, memo)),
            Tag::Var => {
                let mut r = p.payload() as u32;
                while self.uf[r as usize] != r {
                    r = self.uf[r as usize];
                }
                if let Some(v) = memo.get(&r) {
                    return *v;
                }
                memo.insert(r, false);
                let input = |me: &Self, memo: &mut HashMap<u32, bool>, a: u32| me.input_of[a as usize].is_some_and(|w| me.needs_param(net, wire(w), memo));
                let v = match self.producer[r as usize] {
                    Some(Producer::LamParam(_)) => true,
                    Some(Producer::Ref(a, _)) => self.needs_param(net, a, memo),
                    Some(Producer::ResOp(a, y)) => self.needs_param(net, Port(net.cell(op_addr(a))[0]), memo) || self.needs_param(net, y, memo),
                    Some(Producer::Op(a) | Producer::App(a)) => self.needs_param(net, Port(net.cell(a)[0]), memo) || input(self, memo, a),
                    Some(Producer::Swi(a) | Producer::Mat(a)) => input(self, memo, a),
                    Some(Producer::Dup(d)) => self.dup_needs_param(net, d, memo),
                    Some(Producer::Free(_)) | None => false,
                };
                memo.insert(r, v);
                v
            }
            _ => false,
        }
    }

    /// A dup's input: its own, or through the dup it was commuted from.
    fn dup_needs_param(&self, net: &Net, mut d: u32, memo: &mut HashMap<u32, bool>) -> bool {
        while let Some((pd, _)) = self.dup_parent.get(&d) {
            d = *pd;
        }
        self.input_of[d as usize].is_some_and(|w| self.needs_param(net, Port::new(Tag::Var, w as u64), memo))
    }
}

fn is_consumer(p: Port) -> bool {
    matches!(p.tag(), Tag::Op | Tag::Swi | Tag::Mat | Tag::Dup | Tag::App)
}

/// Typed walk from the roots (the root wire, the unknown inputs, the
/// residual pairs); fills the index. `free` are the unknown input wires
/// with their Core variables.
pub(crate) fn scan(net: &Net, free: &[(Port, u32)]) -> Index {
    let residual = &net.residual;
    let n = net.cells.len();
    let mut ix = Index { uf: (0..n as u32).collect(), stored: vec![None; n], producer: vec![None; n], input_of: vec![None; n], parked: Vec::new(), dup_labels: HashMap::new(), lam_only: HashSet::new(), dup_side: HashMap::new(), dup_parent: HashMap::new(), sup_labels: HashSet::new() };
    let params: Vec<Port> = free.iter().map(|(p, _)| *p).collect();
    for (p, v) in free {
        debug_assert_eq!(p.tag(), Tag::Var);
        ix.producer[p.payload() as usize] = Some(Producer::Free(*v));
    }
    let mut seen: PortSet = PortSet::default();
    let mut work: Vec<Port> = vec![crate::root_port()];
    work.extend(params.iter().copied());
    for (a, b) in residual {
        match a.tag() {
            Tag::Ref => {
                if b.tag() == Tag::Var {
                    ix.producer[b.payload() as usize] = Some(Producer::Ref(*a, b.payload() as u32));
                }
            }
            Tag::Op => {
                let c = net.cell(op_addr(*a));
                let ret = Port(c[1]);
                debug_assert_eq!(ret.tag(), Tag::Var);
                ix.producer[ret.payload() as usize] = Some(Producer::ResOp(*a, *b));
            }
            t => panic!("ICE: residual pair headed by {:?}", t),
        }
        work.push(*a);
        work.push(*b);
    }
    let mut pending_unions: Vec<(u32, u32)> = Vec::new();
    while let Some(p) = work.pop() {
        if !seen.insert(p.0) {
            continue;
        }
        match p.tag() {
            Tag::Num | Tag::Big | Tag::Era | Tag::Flo | Tag::Ext => {}
            Tag::Kont | Tag::Arr | Tag::Other => panic!("ICE: runtime-only agent {:?} at compile time", p.tag()),
            Tag::Var => {
                let w = p.payload() as u32;
                let s = Port(net.cell(w)[0]);
                if s == EMPTY {
                    continue;
                }
                if s.tag() == Tag::Var {
                    pending_unions.push((w, s.payload() as u32));
                } else {
                    ix.stored[w as usize] = Some(s);
                    if is_consumer(s) {
                        ix.input_of[addr_of(s) as usize] = Some(w);
                    }
                    if s.tag() == Tag::Dup {
                        ix.dup_labels.insert(dup_addr(s), dup_label(s));
                    }
                }
                work.push(s);
            }
            Tag::Con => {
                for f in con_fields(net, p) {
                    work.push(f);
                }
            }
            Tag::Op => {
                let c = net.cell(op_addr(p));
                let ret = Port(c[1]);
                if ret.tag() == Tag::Var {
                    let r = &mut ix.producer[ret.payload() as usize];
                    if r.is_none() {
                        *r = Some(Producer::Op(op_addr(p)));
                    }
                }
                work.push(Port(c[0]));
                work.push(ret);
            }
            Tag::Swi => {
                let s = p.payload() as u32;
                ix.parked.push((s, false));
                let c = net.cell(s);
                let ret = Port(c[0]);
                if ret.tag() == Tag::Var {
                    ix.producer[ret.payload() as usize] = Some(Producer::Swi(s));
                }
                let c2 = net.cell(Port(c[1]).payload() as u32);
                work.push(ret);
                work.push(Port(c2[0]));
                work.push(Port(c2[1]));
            }
            Tag::Mat => {
                let m = mat_addr(p);
                let c = net.cell(m);
                if Port(c[1]) != EMPTY {
                    ix.parked.push((m, true)); // a projection has no arms
                }
                let ret = Port(c[0]);
                if ret.tag() == Tag::Var {
                    ix.producer[ret.payload() as usize] = Some(Producer::Mat(m));
                }
                work.push(ret);
                for r in list_items(net, Port(c[1])) {
                    work.push(r);
                }
            }
            Tag::Ref => {
                for a in list_items(net, ref_head(p)) {
                    work.push(a);
                }
            }
            Tag::Dup => {
                // a value shared k ways is a chain of k-1 Dup cells, each
                // link's port sitting in the previous cell's second slot;
                // every cell is its own dup (its own label) whose input is
                // its parent's output
                let mut d = dup_addr(p);
                ix.dup_labels.insert(d, dup_label(p));
                loop {
                    let c = net.cell(d);
                    let mut next = None;
                    for (side, o) in [Port(c[0]), Port(c[1])].into_iter().enumerate() {
                        match o.tag() {
                            Tag::Var => {
                                // a lambda copy's parameter wire keeps its
                                // LamParam producer (the dup is the
                                // superposition of the copies' parameters)
                                let slot = &mut ix.producer[o.payload() as usize];
                                if !matches!(slot, Some(Producer::LamParam(_))) {
                                    *slot = Some(Producer::Dup(d));
                                }
                                ix.dup_side.insert(o.payload() as u32, (d, side));
                                work.push(o);
                            }
                            Tag::Dup => {
                                let n = dup_addr(o);
                                ix.dup_labels.insert(n, dup_label(o));
                                ix.dup_parent.insert(n, (d, side));
                                next = Some(n);
                            }
                            _ => work.push(o),
                        }
                    }
                    match next {
                        Some(n) => d = n,
                        None => break,
                    }
                }
            }
            Tag::Lam => {
                // [param, body]: the parameter is an unknown input of the
                // body, the body a value read under the closure's frame
                let l = p.payload() as u32;
                let c = net.cell(l);
                if Port(c[0]).tag() == Tag::Var {
                    ix.producer[Port(c[0]).payload() as usize] = Some(Producer::LamParam(l));
                }
                work.push(Port(c[0]));
                work.push(Port(c[1]));
            }
            Tag::App => {
                // [arg, ret], parked on the function's wire
                let a = p.payload() as u32;
                let c = net.cell(a);
                let ret = Port(c[1]);
                if ret.tag() == Tag::Var {
                    let r = &mut ix.producer[ret.payload() as usize];
                    if r.is_none() {
                        *r = Some(Producer::App(a));
                    }
                }
                work.push(Port(c[0]));
                work.push(ret);
            }
        }
    }
    for (a, b) in pending_unions {
        ix.union(a, b);
    }
    // re-key everything recorded by wire id under its class
    for w in 0..n as u32 {
        let r = ix.find(w);
        if r != w {
            if let Some(v) = ix.stored[w as usize].take() {
                ix.stored[r as usize] = Some(v);
            }
            if let Some(v) = ix.producer[w as usize].take() {
                ix.producer[r as usize] = Some(v);
            }
        }
    }
    for a in 0..n {
        if let Some(w) = ix.input_of[a] {
            ix.input_of[a] = Some(ix.find(w));
        }
    }
    // what depends on an unapplied closure's parameter is stamped at its application
    let mut memo = HashMap::new();
    for (a, b) in residual {
        if a.tag() == Tag::Ref && b.tag() == Tag::Var && ix.needs_param(net, *a, &mut memo) {
            ix.lam_only.insert(b.payload() as u32);
        }
    }
    let dups: Vec<u32> = ix.dup_labels.keys().copied().collect();
    for d in dups {
        if ix.dup_needs_param(net, d, &mut memo) {
            ix.lam_only.insert(d);
        }
    }
    let sides: Vec<(u32, (u32, usize))> = ix.dup_side.iter().map(|(w, v)| (*w, *v)).collect();
    for (w, v) in sides {
        let r = ix.find(w);
        ix.dup_side.entry(r).or_insert(v);
    }
    // orientation: a dup stored in a wire nothing produces is a
    // superposition (a copied closure's parameter, or what commutations
    // made of it): its slots are its sources, not outputs it produces.
    // A wire between a superposition's slot and a fan-out's slot (the
    // shape a DUP–DUP commutation leaves) is produced by the fan-out.
    let dups: Vec<u32> = ix.dup_labels.keys().copied().collect();
    let mut sups: HashSet<u32> = HashSet::new();
    // to a fixpoint: a superposition's sources are its slots, so a dup
    // whose input is one of those (a superposition of superpositions) is
    // itself one
    loop {
        let mut changed = false;
        for &d in &dups {
            if sups.contains(&d) || ix.dup_parent.contains_key(&d) {
                continue;
            }
            let Some(w) = ix.input_of[d as usize] else { continue };
            if ix.producer[w as usize].is_none() {
                sups.insert(d);
                ix.sup_labels.insert(ix.dup_labels[&d]);
                changed = true;
            }
        }
        if !changed {
            break;
        }
        for &d in &dups {
            let c = net.cell(d);
            for (side, o) in [Port(c[0]), Port(c[1])].into_iter().enumerate() {
                if o.tag() != Tag::Var {
                    continue;
                }
                let r = ix.find(o.payload() as u32);
                if sups.contains(&d) {
                    if matches!(ix.producer[r as usize], Some(Producer::Dup(x)) if x == d) {
                        ix.producer[r as usize] = None;
                        ix.dup_side.remove(&r);
                    }
                } else if ix.producer[r as usize].is_none() {
                    ix.producer[r as usize] = Some(Producer::Dup(d));
                    ix.dup_side.insert(r, (d, side));
                }
            }
        }
    }
    ix
}

/// Per arm of an instantiated parked branch: the binder variables of its
/// pattern (a match arm) and the scope frame its own shared values belong
/// to.
#[derive(Clone)]
pub(crate) struct ArmInfo {
    pub binders: Vec<u32>,
    pub frame: usize,
}

pub(crate) struct Reader<'a> {
    net: &'a Net,
    prog: &'a NetProg,
    ix: Index,
    next: u32,
    /// a shared Dup's binding, per superposition selection in effect
    shared: HashMap<(u32, Vec<(u32, usize)>), u32>,
    /// pattern binders per arm frame
    arm_binders: HashMap<usize, Vec<u32>>,
    /// the scope frame each Dup cell was created in (see `specialize`)
    dup_frame: &'a HashMap<u32, usize>,
    /// the scope frame each pending call was created in, by its result
    /// wire's cell
    ref_frame: &'a HashMap<u32, usize>,
    /// (agent cell, arm index) -> arm info
    arms: &'a HashMap<(u32, usize), ArmInfo>,
    /// active frames, innermost last
    stack: Vec<usize>,
    /// while a value created in an outer frame is read: that frame's stack
    /// position (values bind there, not in the arms above); usize::MAX = top
    floor: usize,
    /// shared bindings per frame, in dependency order
    bindings: HashMap<usize, Vec<(u32, Core)>>,
    /// residual closures being read, innermost last: (frame, parameter var)
    lams: Vec<(usize, u32)>,
    /// parameter var per Lam cell
    lam_param: HashMap<u32, u32>,
    /// superposition sides in effect: reading through side `side` of a
    /// dup with a copy label selects, for every same-label dup met as a
    /// superposition, its source of that side
    sel: Vec<(u32, usize)>,
}

/// The frame of a residual closure's body (disjoint from arm frames).
fn lam_frame(l: u32) -> usize {
    (1usize << 40) + l as usize
}

/// Whether `e` reads any of `vars`.
fn mentions(e: &Core, vars: &[u32]) -> bool {
    e.any(&mut |n| match n {
        Core::Var(v) => Some(vars.contains(v)),
        _ => None,
    })
}

impl<'a> Reader<'a> {
    pub(crate) fn new(
        net: &'a Net,
        prog: &'a NetProg,
        free: &[(Port, u32)],
        next: u32,
        dup_frame: &'a HashMap<u32, usize>,
        ref_frame: &'a HashMap<u32, usize>,
        arms: &'a HashMap<(u32, usize), ArmInfo>,
    ) -> Reader<'a> {
        let ix = scan(net, free);
        let mut arm_binders: HashMap<usize, Vec<u32>> = HashMap::new();
        for a in arms.values() {
            arm_binders.entry(a.frame).or_default().extend(a.binders.iter().copied());
        }
        Reader { net, prog, ix, next, shared: HashMap::new(), arm_binders, dup_frame, ref_frame, arms, stack: vec![0], floor: usize::MAX, bindings: HashMap::new(), lams: Vec::new(), lam_param: HashMap::new(), sel: Vec::new() }
    }

    /// Read a frame's expression from `p` (its tail, compound) and wrap it
    /// in the frame's bindings: every compound value the frame computes is
    /// let-bound in evaluation order (let-normal form), shared values once.
    pub(crate) fn read_frame(&mut self, frame: usize, p: Port) -> Core {
        // a new scope (an arm, a closure body): its own values bind in it
        let floor = std::mem::replace(&mut self.floor, usize::MAX);
        self.stack.push(frame);
        let mut e = self.read(p);
        self.stack.pop();
        self.floor = floor;
        for (v, b) in self.bindings.remove(&frame).unwrap_or_default().into_iter().rev() {
            e = Core::Let(v, Box::new(b), Box::new(e));
        }
        e
    }

    /// The value of `p` as an operand: a variable, a literal or a
    /// single-use scalar expression; any other compound value (a call, a
    /// projection, a branch, data) is let-bound in the current frame first.
    fn atom(&mut self, p: Port) -> Core {
        let e = self.read(p);
        self.bind_atom(e)
    }

    fn bind_atom(&mut self, e: Core) -> Core {
        if matches!(e, Core::Var(_) | Core::Num(_) | Core::Flo(_) | Core::Op2(..) | Core::Cmp(..) | Core::If(..)) {
            return e;
        }
        self.bind(e)
    }

    /// Let-bind a value in the innermost active frame that needs it: a
    /// closure frame whose parameter the value does not read is peeled
    /// (the value is computed once outside the closure and shared by every
    /// call, which is what the net did); a branch frame never is (its
    /// work is conditional).
    fn bind(&mut self, e: Core) -> Core {
        if matches!(e, Core::Var(_) | Core::Num(_) | Core::Flo(_)) {
            return e;
        }
        let frame = self.place(&e);
        self.bind_in(frame, e)
    }

    /// what frame `fr` binds: pattern binders and let-bound values
    fn bound_in(&self, fr: usize) -> Vec<u32> {
        let mut vs: Vec<u32> = self.arm_binders.get(&fr).cloned().unwrap_or_default();
        vs.extend(self.bindings.get(&fr).into_iter().flatten().map(|(v, _)| *v));
        vs
    }

    /// The one placement rule for every let-bound value: no lower than the
    /// innermost frame binding something it reads; above that, a closure
    /// frame only when the value reads what it binds (else every
    /// application shares it), a non-closure frame unless above `floor`.
    fn place(&self, e: &Core) -> usize {
        let need = self.stack.iter().rposition(|fr| mentions(e, &self.bound_in(*fr))).unwrap_or(0);
        for (i, &fr) in self.stack.iter().enumerate().rev() {
            match self.lams.iter().find(|(lf, _)| *lf == fr) {
                Some((_, x)) => {
                    let mut inside = vec![*x];
                    for g in &self.stack[i..] {
                        inside.extend(self.bound_in(*g));
                    }
                    if mentions(e, &inside) {
                        return fr;
                    }
                }
                None if i > self.floor && i > need => continue,
                None => return fr,
            }
        }
        0
    }

    /// Read with frame `frame`'s stack position as the floor (see `place`).
    fn read_in<T>(&mut self, frame: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let pos = self.stack.iter().rposition(|g| *g == frame).unwrap_or_else(|| panic!("ICE: frame {frame} read outside its scope (stack {:?})", self.stack));
        let saved = std::mem::replace(&mut self.floor, pos);
        let v = f(self);
        self.floor = saved;
        v
    }

    /// where values bind now: the floor, or the top
    fn here(&self) -> usize {
        self.floor.min(self.stack.len() - 1)
    }

    fn bind_in(&mut self, frame: usize, e: Core) -> Core {
        if matches!(e, Core::Var(_) | Core::Num(_) | Core::Flo(_)) {
            return e;
        }
        let x = self.fresh();
        self.bindings.entry(frame).or_default().push((x, e));
        Core::Var(x)
    }

    fn fresh(&mut self) -> u32 {
        self.next += 1;
        self.next - 1
    }

    pub(crate) fn read(&mut self, p: Port) -> Core {
        match p.tag() {
            Tag::Num | Tag::Big => Core::Num(p.int_value()),
            Tag::Flo => Core::Flo(f64::from_bits(crate::flo_bits(self.net.cell(p.payload() as u32)))),
            Tag::Con => {
                let fields: Vec<Core> = con_fields(self.net, p).into_iter().map(|f| self.atom(f)).collect();
                if p.con_tag() == CTAG_TUPLE {
                    Core::Tuple(fields)
                } else {
                    Core::Ctor(cid_of(p.con_tag()), fields)
                }
            }
            Tag::Var => self.read_wire(p.payload() as u32),
            Tag::Lam => {
                let l = p.payload() as u32;
                let x = self.param_var(l);
                let frame = lam_frame(l);
                self.lams.push((frame, x));
                let body = self.read_frame(frame, Port(self.net.cell(l)[1]));
                self.lams.pop();
                Core::Lam(x, Box::new(body))
            }
            t => panic!("ICE: readback of a {:?} port as a value", t),
        }
    }

    fn read_wire(&mut self, w: u32) -> Core {
        let r = self.ix.find(w);
        if let Some(s) = self.ix.stored[r as usize] {
            if !is_consumer(s) {
                return self.read(s);
            }
            if s.tag() == Tag::Dup && self.ix.producer[r as usize].is_none() {
                // a superposition feeding this use (a dup stored where
                // nothing produces): its source of the side being read
                if let Some(side) = self.selected(dup_addr(s)) {
                    let src = Port(self.net.cell(dup_addr(s))[side]);
                    return self.read(src);
                }
            }
        }
        self.read_producer(r)
    }

    /// The side selected for dup `d`'s label, if its label is a copy in
    /// progress.
    fn selected(&self, d: u32) -> Option<usize> {
        let label = *self.ix.dup_labels.get(&d)?;
        self.sel.iter().rev().find(|(l, _)| *l == label).map(|(_, s)| *s)
    }

    /// The expression of the agent producing the value of wire class `r`.
    fn read_producer(&mut self, r: u32) -> Core {
        let prod = self.producer_of(r);
        match prod {
            Producer::Free(v) => Core::Var(v),
            Producer::LamParam(l) => Core::Var(self.param_var(l)),
            Producer::App(a) => {
                let f = self.read_input(a);
                let f = self.bind_atom(f);
                let arg = self.atom(Port(self.net.cell(a)[0]));
                Core::App(Box::new(f), Box::new(arg))
            }
            Producer::Op(a) => {
                let code = op_code(self.stored(a));
                self.read_op(a, code)
            }
            Producer::ResOp(op, y) => {
                let code = op_code(op) & !OP_FLIP;
                let c = self.net.cell(op_addr(op));
                let x = self.atom(Port(c[0]));
                // an array set's second operand is its (index, value) pair,
                // consumed in place, never bound
                let y = if code == crate::prim_code(Prim::ArrSet) { self.read(y) } else { self.atom(y) };
                op_core(code, x, y)
            }
            Producer::Swi(s) => {
                let cond = self.read_input(s);
                let cond = self.bind_atom(cond);
                let c = self.net.cell(s);
                let c2 = self.net.cell(Port(c[1]).payload() as u32);
                let t = self.arm(s, 0, Port(c2[0]));
                let e = self.arm(s, 1, Port(c2[1]));
                Core::If(Box::new(cond), Box::new(t), Box::new(e))
            }
            Producer::Mat(m) => {
                let scrut = self.read_input(m);
                let scrut = self.bind_atom(scrut);
                let c = self.net.cell(m);
                let mid = mat_id(self.stored(m)) as usize;
                match &self.prog.metas[mid] {
                    MatchMeta::Proj(i) => Core::Proj(Box::new(scrut), *i),
                    MatchMeta::Arms(tags) => {
                        let slots = list_items(self.net, Port(c[1]));
                        let tags = tags.clone();
                        let mut arms = Vec::new();
                        for (i, (t, r)) in tags.iter().zip(slots).enumerate() {
                            let binders = self.arms[&(m, i)].binders.clone();
                            let body = self.arm(m, i, r);
                            arms.push((cid_of(*t), binders, body));
                        }
                        Core::Match(Box::new(scrut), arms)
                    }
                }
            }
            Producer::Dup(d) => {
                let side = self.ix.dup_side.get(&r).map(|x| x.1).unwrap_or(0);
                self.read_dup(d, side)
            }
            Producer::Ref(r, ret) => {
                let entry = ref_entry(r) as usize;
                assert!(entry < self.prog.nfns, "ICE: residual call to a lifted entry");
                let call = |me: &mut Self| Core::Call(entry as u32, list_items(me.net, ref_head(r)).into_iter().map(|a| me.atom(a)).collect());
                // A pending call binds in the frame it was created in (the
                // net fires it there), under anything it reads (`place`).
                let created = self.ref_frame.get(&ret).map(|f| {
                    self.stack.iter().rposition(|g| g == f).unwrap_or_else(|| panic!("ICE: a call's frame {f} is not active (stack {:?})", self.stack))
                });
                match created {
                    Some(pos) if pos != self.here() => {
                        let frame = self.stack[pos];
                        let call = self.read_in(frame, call);
                        let at = self.read_in(frame, |me| me.place(&call));
                        self.bind_in(at, call)
                    }
                    // created where it is used: read in place (the caller
                    // binds it, or keeps it as a tail call)
                    _ => call(self),
                }
            }
        }
    }

    fn param_var(&mut self, l: u32) -> u32 {
        if let Some(x) = self.lam_param.get(&l) {
            return *x;
        }
        let x = self.fresh();
        self.lam_param.insert(l, x);
        x
    }

    /// Output `side` of Dup cell `d`. A copy dup (its label also names
    /// superpositions) fans out an open term: read once per side under
    /// that side's selection, never shared. Any other dup shares one
    /// value: bound once, in the frame it was created in (or the
    /// innermost closure whose parameter it reads).
    fn read_dup(&mut self, d: u32, side: usize) -> Core {
        let label = self.ix.dup_labels[&d];
        if self.ix.sup_labels.contains(&label) {
            self.sel.push((label, side));
            let e = self.read_dup_input(d);
            self.sel.pop();
            return e;
        }
        let key = (d, self.sel.clone());
        if let Some(x) = self.shared.get(&key) {
            return Core::Var(*x);
        }
        let frame = *self.dup_frame.get(&d).unwrap_or(&0);
        let e = self.read_in(frame, |me| me.read_dup_input(d));
        let at = self.read_in(frame, |me| me.place(&e));
        let a = self.bind_in(at, e);
        if let Core::Var(v) = a {
            self.shared.insert(key, v);
        }
        a
    }

    /// What flows into Dup cell `d`: its input wire, or its parent link's
    /// output.
    fn read_dup_input(&mut self, d: u32) -> Core {
        if let Some((p, side)) = self.ix.dup_parent.get(&d).copied() {
            return self.read_dup(p, side);
        }
        self.read_input(d)
    }

    fn producer_of(&self, r: u32) -> Producer {
        self.ix.producer[r as usize].unwrap_or_else(|| panic!("ICE: residual wire class {} has no producer (stored {:?})", r, self.ix.stored[r as usize]))
    }

    /// The port of agent `a` as stored in its input wire.
    fn stored(&self, a: u32) -> Port {
        let r = self.ix.input_of[a as usize].unwrap_or_else(|| panic!("ICE: agent at cell {} is not stored in any wire", a));
        self.ix.stored[r as usize].unwrap()
    }

    /// The value the agent at `a` is waiting on: its input wire's producer
    /// (the port stored in that wire is the agent itself).
    fn read_input(&mut self, a: u32) -> Core {
        let r = match self.ix.input_of[a as usize] {
            Some(r) => r,
            None => panic!("ICE: agent at cell {} ({:?}) is not stored in any wire; cell = {:?}", a, self.ix.producers().find(|p| matches!(p, Producer::Op(x) | Producer::Swi(x) | Producer::Mat(x) | Producer::Dup(x) if *x == a)), self.net.cell(a).map(|w| Port(w).tag())),
        };
        self.read_producer(r)
    }

    fn read_op(&mut self, a: u32, code: u16) -> Core {
        let c = self.net.cell(a);
        let flipped = code & OP_FLIP != 0;
        // operands in source order (the first operand is the one the op
        // was linked to: its input wire, unless flipped)
        let code = code & !OP_FLIP;
        let pair = code == crate::prim_code(Prim::ArrSet);
        let (x, y) = if flipped {
            let x = self.atom(Port(c[0]));
            let m = self.read_input(a);
            (x, if pair { m } else { self.bind_atom(m) })
        } else {
            let m = self.read_input(a);
            let x = self.bind_atom(m);
            (x, if pair { self.read(Port(c[0])) } else { self.atom(Port(c[0])) })
        };
        op_core(code, x, y)
    }

    /// An instantiated arm of a parked branch: its slot holds the wire its
    /// body's value flows into (see `specialize`).
    fn arm(&mut self, agent: u32, i: usize, slot: Port) -> Core {
        let frame = self.arms[&(agent, i)].frame;
        self.read_frame(frame, slot)
    }
}

fn op_core(code: u16, x: Core, y: Core) -> Core {
    if code == crate::ARR_PAIR {
        return Core::Tuple(vec![x, y]);
    }
    if code >= PRIM_BASE {
        let (p, unary) = crate::prim_of_code(code);
        return if unary {
            Core::Prim(p, vec![x])
        } else if p == Prim::ArrSet {
            match y {
                Core::Tuple(mut iv) if iv.len() == 2 => {
                    let v = iv.pop().unwrap();
                    let i = iv.pop().unwrap();
                    Core::Prim(Prim::ArrSet, vec![x, i, v])
                }
                other => panic!("ICE: array set residual without its (index, value) pair: {:?}", other),
            }
        } else {
            Core::Prim(p, vec![x, y])
        };
    }
    if code >= 16 {
        return Core::Cmp(CmpOp::ALL[(code - 16) as usize], Box::new(x), Box::new(y));
    }
    Core::Op2(BinOp::ALL[code as usize], Box::new(x), Box::new(y))
}
