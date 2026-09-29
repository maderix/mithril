//! Runtime helpers of generated programs: the part of the generated
//! prelude that does not depend on the program (the linearity table `lin`
//! stays generated, so the helpers that branch on it are emitted with the
//! program and constant-fold there). Everything is `#[inline]`: the
//! generated crate inlines these like its own code.
#![allow(clippy::all, clippy::missing_safety_doc)]

use crate::{Engine, Redex, Wctx};

/// Dive result: Ok(value) | Err(rec) = suspended; the residue is spawned
/// behind a record chain whose root `rec` still needs its parent set.
pub type R = Result<u64, u64>;
/// "No destination" sentinel for nested dives (they unwind on fuel-out).
pub const NONE: u64 = u64::MAX;
pub const M56: u64 = (1u64 << 56) - 1;
pub const T_NUM: u64 = 2;
pub const T_FLO: u64 = 3;
pub const T_CON: u64 = 4;
pub const T_ARR: u64 = 14; // mithril_core::port::Tag::Arr
pub const T_LAM: u64 = 6; // mithril_core::port::Tag::Lam
#[inline] pub fn tag(p: u64) -> u64 { p >> 56 }
pub const TU: u64 = 16;
#[inline] pub fn ic(slot: u64, v: i64) -> u64 { ((TU + slot) << 56) | ((v as u64) & M56) }
/// match dispatch key: boxed ctors -> ctor tag, unboxed -> 0x1000 + slot
#[inline] pub fn mtag(p: u64) -> u32 { let t = tag(p); if t >= TU { 0x1000 + (t - TU) as u32 } else { con_tag(p) as u32 } }

#[inline] pub fn num(v: i64) -> u64 { (T_NUM << 56) | ((v as u64) & M56) }
#[inline] pub fn as_i(p: u64) -> i64 { ((p << 8) as i64) >> 8 }
#[inline] pub fn wrap56(v: i64) -> i64 { ((v as u64) << 8) as i64 >> 8 }
#[inline] pub fn con(addr: u32, k: u16, ar: u8) -> u64 { (T_CON << 56) | ((addr as u64) << 16) | ((k as u64) << 4) | ar as u64 }
#[inline] pub fn con_addr(p: u64) -> u32 { ((p >> 16) & ((1u64 << 40) - 1)) as u32 }
#[inline] pub fn con_tag(p: u64) -> u16 { ((p >> 4) & 0xFFF) as u16 }
#[inline] pub fn con_ar(p: u64) -> u8 { (p & 0xF) as u8 }
#[inline] pub fn flo(ctx: &mut Wctx, v: f64) -> u64 { let a = ctx.alloc(v.to_bits(), 0); (T_FLO << 56) | a as u64 }
#[inline] pub fn flo_val(ctx: &Wctx, p: u64) -> f64 { f64::from_bits(ctx.cell((p & M56) as u32)[0]) }
#[inline]
pub fn mith_unreachable() -> u64 { panic!("unreachable match arm reached at runtime") }
/// Field `i` of a constructor value (walks the >2-arity chain).
#[inline]
pub fn field(ctx: &Wctx, p: u64, i: usize) -> u64 {
    let (mut p, mut i) = (p, i);
    loop {
        let ar = con_ar(p) as usize;
        let c = ctx.cell(con_addr(p));
        if ar > 2 {
            if i == 0 { return c[0]; }
            p = c[1];
            i -= 1;
        } else {
            return c[i];
        }
    }
}

#[inline]
pub fn floor_div(a: i64, b: i64) -> i64 {
    let q = a.wrapping_div(b);
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) { q - 1 } else { q }
}

#[inline]
pub fn py_mod(a: i64, b: i64) -> i64 {
    let r = a.wrapping_rem(b);
    if r != 0 && (r < 0) != (b < 0) { r + b } else { r }
}

/// Consume with a reuse token: moves the fields out and, when the caller
/// held the only reference (always, for linear types), hands the cell back
/// as a token instead of freeing it (NOTOK otherwise).
pub const NOTOK: u32 = u32::MAX;
/// Tail recursion modulo cons: a TRMC loop holds the result's head and the
/// cell whose field 1 is the pending hole (NOHOLE before the first cell).
pub const NOHOLE: u32 = u32::MAX;
#[inline(always)]
pub fn hole_link(ctx: &mut Wctx, head: &mut u64, hole: &mut u32, p: u64) {
    if *hole == NOHOLE {
        *head = p;
    } else {
        ctx.set(*hole, 1, p);
    }
    *hole = con_addr(p);
}

#[inline(always)]
pub fn hole_fill(ctx: &mut Wctx, head: u64, hole: u32, v: u64) -> u64 {
    if hole == NOHOLE {
        return v;
    }
    ctx.set(hole, 1, v);
    head
}

/// A suspension inside a TRMC loop: the suspended value belongs in the
/// hole, and the head is what the caller receives.
#[cold]
#[inline(never)]
pub fn hole_wrap(ctx: &mut Wctx, r: u64, head: u64, hole: u32, rule: u16) -> u64 {
    if hole == NOHOLE {
        return r;
    }
    let hr = ctx.alloc_rec(rule, 1, con_addr(head), hole, NONE);
    ctx.set_parent(r as u32, (hr as u64) << 3);
    hr as u64
}

/// Release an unused reuse token.
#[inline(always)]
pub fn tok_free(ctx: &mut Wctx, tok: u32) {
    if tok != NOTOK {
        ctx.free(tok);
    }
}

pub const ARR_BOXED: u64 = 1 << 63;
pub const ARR_RAW: u64 = 1 << 62;
/// IEEE-754 binary32 on bit patterns (see mithril_front::core::f32_prim).
#[inline(always)] pub fn f32b(x: i64) -> f32 { f32::from_bits(x as u32) }

#[inline(always)] pub fn f32i(x: f32) -> i64 { x.to_bits() as i64 }
#[inline(always)] pub fn f32_add(a: i64, b: i64) -> i64 { f32i(f32b(a) + f32b(b)) }
#[inline(always)] pub fn f32_sub(a: i64, b: i64) -> i64 { f32i(f32b(a) - f32b(b)) }
#[inline(always)] pub fn f32_mul(a: i64, b: i64) -> i64 { f32i(f32b(a) * f32b(b)) }
#[inline(always)] pub fn f32_div(a: i64, b: i64) -> i64 { f32i(f32b(a) / f32b(b)) }
#[inline(always)] pub fn f32_sqrt(a: i64) -> i64 { f32i(f32b(a).sqrt()) }
#[inline(always)] pub fn f32_lt(a: i64, b: i64) -> i64 { (f32b(a) < f32b(b)) as i64 }
#[inline(always)] pub fn f32_from_u32(a: i64) -> i64 { f32i((a as u32) as f32) }
#[inline(always)] pub fn f32_to_u32(a: i64) -> i64 { let x = f32b(a); if x.is_nan() || x < 0.0 || x >= 4294967296.0 { 0 } else { x as u32 as i64 } }
/// Native int representation: an i56 value held as `x << 8`, so i64
/// wrapping arithmetic is i56 wrapping arithmetic.
#[inline(always)]
pub fn sh(p: u64) -> i64 {
    (p << 8) as i64
}

/// A native (pre-shifted) int back to a tagged port.
#[inline(always)]
pub fn retag(x: i64) -> u64 {
    ((x as u64) >> 8) | (T_NUM << 56)
}

#[inline(always)]
pub fn arr_raw(p: u64) -> bool {
    // SAFETY: as arr_rc
    unsafe { *arr_block(p).add(1) & ARR_RAW != 0 }
}

/// Element k as a tagged port (no reference taken).
#[inline(always)]
pub fn arr_elem(p: u64, k: usize) -> u64 {
    // SAFETY: callers pass k < len
    let e = unsafe { *arr_elems(p).add(k) };
    if arr_raw(p) { retag(e as i64) } else { e }
}

/// An all-int array of n copies of the native int x.
#[inline]
pub fn arr_new_raw(n: i64, x: i64) -> u64 {
    if n < 0 {
        panic!("negative array size {}", n);
    }
    let p = arr_alloc_fill(n as usize, x as u64);
    // SAFETY: as arr_rc; fresh block
    unsafe { *arr_block(p).add(1) |= ARR_RAW }
    p
}

/// A raw array about to store a non-int: switch it to tagged elements.
#[cold]
#[inline(never)]
pub fn arr_unraw(a: u64) {
    let n = arr_len_of(a);
    for k in 0..n {
        // SAFETY: k < n; `a` uniquely ours
        unsafe { *arr_elems(a).add(k) = retag(*arr_elems(a).add(k) as i64) }
    }
    // SAFETY: as above
    unsafe { *arr_block(a).add(1) &= !ARR_RAW }
}

#[inline(always)]
pub fn is_heap(v: u64) -> bool {
    let t = tag(v);
    t == T_CON || t == T_FLO || t == T_ARR || t == T_LAM
}

#[inline(always)]
pub fn arr_mark_boxed(a: u64, v: u64) {
    if is_heap(v) {
        // SAFETY: as arr_rc; word 1 is the len word, `a` uniquely ours
        unsafe { *arr_block(a).add(1) |= ARR_BOXED }
    }
}

#[inline(always)]
pub fn arr_boxed(p: u64) -> bool {
    // SAFETY: as arr_rc
    unsafe { *arr_block(p).add(1) & ARR_BOXED != 0 }
}

#[inline(always)]
pub fn arr_block(p: u64) -> *mut u64 {
    (p & M56) as *mut u64
}

#[inline(always)]
pub fn arr_rc(p: u64) -> &'static std::sync::atomic::AtomicU64 {
    // SAFETY: a live T_ARR port points at a block whose first word is its
    // refcount (arr_alloc), alive while this reference is
    unsafe { &*(arr_block(p) as *const std::sync::atomic::AtomicU64) }
}

#[inline(always)]
pub fn arr_len_of(p: u64) -> usize {
    // SAFETY: as arr_rc; word 1 is the length
    unsafe { (*arr_block(p).add(1) & !(ARR_BOXED | ARR_RAW)) as usize }
}

#[inline(always)]
pub fn arr_elems(p: u64) -> *mut u64 {
    // SAFETY: as arr_rc; elements start at word 2
    unsafe { arr_block(p).add(2) }
}

#[inline]
pub fn arr_alloc(n: usize) -> u64 {
    arr_alloc_fill(n, 0)
}

/// Arrays currently allocated (reported by MITHRIL_STATS; a leak check).
pub static ARR_LIVE: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
#[inline]
pub fn arr_alloc_fill(n: usize, fill: u64) -> u64 {
    ARR_LIVE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut v: Vec<u64> = Vec::with_capacity(n + 2);
    v.push(1);
    v.push(n as u64);
    v.resize(n + 2, fill);
    let b = Box::into_raw(v.into_boxed_slice()) as *mut u64;
    debug_assert!((b as u64) >> 56 == 0);
    (T_ARR << 56) | (b as u64)
}

#[inline]
pub fn arr_free_block(p: u64) {
    ARR_LIVE.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    let n = arr_len_of(p);
    // SAFETY: the block was made by arr_alloc with n + 2 words and this
    // was its last reference
    unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(arr_block(p), n + 2))) }
}

#[cold]
#[inline(never)]
pub fn arr_oob(i: i64, n: usize) -> ! {
    panic!("array index {} out of bounds (length {})", i, n)
}

/// Int-element arrays (typed `Arr(true)`, always raw): no element
/// refcounting, one shift to or from the tagged form.
#[inline(always)]
pub fn arr_get_i(a: u64, i: i64) -> u64 {
    retag(arr_get_r(a, i) as i64)
}

/// Native read: the raw (pre-shifted) element.
#[inline(always)]
pub fn arr_get_r(a: u64, i: i64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    debug_assert!(arr_raw(a) || n == 0);
    // SAFETY: bounds checked
    unsafe { *arr_elems(a).add(i as usize) }
}

/// Native code: arrays there are linear and made unique at the boundary
/// (`arr_own`), so a write needs no refcount check.
#[inline(always)]
pub fn arr_set_u(a: u64, i: i64, v: u64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    debug_assert!(arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1);
    // SAFETY: bounds checked; `a` is unique (see arr_own)
    unsafe { *arr_elems(a).add(i as usize) = v };
    a
}

/// Native access with the length held in a local (see scalar.rs `lens`).
#[inline(always)]
pub fn arr_get_n(a: u64, n: usize, i: i64) -> u64 {
    if (i as u64) >= n as u64 {
        arr_oob(i, n);
    }
    // SAFETY: bounds checked against the array's length
    unsafe { *arr_elems(a).add(i as usize) }
}

#[inline(always)]
pub fn arr_set_n(a: u64, n: usize, i: i64, v: u64) -> u64 {
    if (i as u64) >= n as u64 {
        arr_oob(i, n);
    }
    // SAFETY: bounds checked; `a` is unique (see arr_own)
    unsafe { *arr_elems(a).add(i as usize) = v };
    a
}

#[inline]
pub fn arr_new_i(n: i64, v: u64) -> u64 {
    arr_new_raw(n, sh(v))
}

/// Spawn a saturated call: arity <= 2 rides in (a, b); wider calls put
/// arg0 in `a` and chain args[1..] through cells in `b` (addr+1, 0 = end).
#[inline]
pub fn spawn_call(ctx: &mut Wctx, rule: u16, args: &[u64], parent: u64) {
    let (a, b) = match args.len() {
        0 => (0, 0),
        1 => (args[0], 0),
        2 => (args[0], args[1]),
        _ => {
            let mut ch: u64 = 0;
            for &x in args[1..].iter().rev() {
                ch = ctx.alloc(x, ch) as u64 + 1;
            }
            (args[0], ch)
        }
    };
    ctx.spawn(rule, Redex { a, b, aux: parent });
}

/// Elementwise wrapping add of two equal-shape int tuples, in place into `a`
/// (re-masked to the low 32 bits per element when `mask32`); `b` is
/// consumed and its cells freed.
#[inline]
pub fn tup_add(ctx: &mut Wctx, a: u64, b: u64, mask32: bool) -> u64 {
    let (mut pa, mut pb) = (a, b);
    loop {
        let n = con_ar(pa) as usize;
        if n == 0 { return a; }
        let (ca, cb) = (con_addr(pa), con_addr(pb));
        let (xa, xb) = (ctx.cell(ca), ctx.cell(cb));
        ctx.free(cb);
        let add = |x: u64, y: u64| {
            let v = as_i(x).wrapping_add(as_i(y));
            num(if mask32 { v & 0xFFFF_FFFF } else { wrap56(v) })
        };
        ctx.set(ca, 0, add(xa[0], xb[0]));
        if n <= 2 {
            if n == 2 { ctx.set(ca, 1, add(xa[1], xb[1])); }
            return a;
        }
        pa = xa[1];
        pb = xb[1];
    }
}



// ---- the context vocabulary of the lowered IR (crates/mithril-codegen/src/lir.rs) ----
//
// Generated code names the worker context only through these free
// functions, so a backend without a context object (the device) supplies
// the same names over its own arena.

#[inline] pub fn alloc2(ctx: &mut Wctx, a: u64, b: u64) -> u32 { ctx.alloc(a, b) }
#[inline] pub fn cell_set(ctx: &Wctx, i: u32, slot: usize, v: u64) { ctx.set(i, slot, v) }
#[inline] pub fn alloc_rec(ctx: &mut Wctx, rule: u16, pend: u32, d: u32, s: u32, parent: u64) -> u32 { ctx.alloc_rec(rule, pend, d, s, parent) }
#[inline] pub fn set_parent(ctx: &mut Wctx, rec: u32, parent: u64) { ctx.set_parent(rec, parent) }
#[inline] pub fn ready_rec(ctx: &mut Wctx, rec: u32) { ctx.ready_rec(rec) }
#[inline] pub fn deliver(ctx: &mut Wctx, parent: u64, v: u64) { ctx.deliver(parent, v) }
#[inline] pub fn rec_parent(ctx: &Wctx, rec: u32) -> u64 { ctx.rec(rec).parent }
#[inline] pub fn rec_d(ctx: &Wctx, rec: u32) -> u32 { ctx.rec(rec).d }
#[inline] pub fn rec_s(ctx: &Wctx, rec: u32) -> u32 { ctx.rec(rec).s }
#[inline] pub fn fuel_of(ctx: &Wctx) -> i64 { ctx.fuel() }
/// The record address form of a record index (slot 0).
#[inline] pub fn rec_addr(rec: u32) -> u64 { (rec as u64) << 3 }
/// Pop the head value of a `[value, next]` spill chain (`ch` = addr + 1;
/// 0 = end), freeing its cell.
#[inline]
pub fn pop_chain(ctx: &mut Wctx, ch: &mut u64) -> u64 {
    let c = ctx.cell((*ch - 1) as u32);
    ctx.free((*ch - 1) as u32);
    *ch = c[1];
    c[0]
}
/// Dive `f` (`args[0]` = the destination) and deliver its result there
/// (a suspended dive has attached its residue to the destination itself).
#[inline]
pub fn dive_to(ctx: &mut Wctx, f: u16, args: &[u64]) {
    if let crate::DiveResult::Done(v) = ctx.dive(f, args) {
        ctx.deliver(args[0], v);
    }
}
/// Dive `f` with no destination (`args[0]` = `NONE`): `Ok(value)`, or
/// `Err(rec)` = the root record of its residue, whose parent the caller sets.
#[inline]
pub fn dive_res(ctx: &mut Wctx, f: u16, args: &[u64]) -> Result<u64, u32> {
    match ctx.dive(f, args) {
        crate::DiveResult::Done(v) => Ok(v),
        crate::DiveResult::Suspended(rec) => Err(rec),
    }
}
/// The budget a callee gets: the caller's (the CPU has one world; the
/// device runtime hands none in its parallel world, where every call is a
/// task at once).
#[inline] pub fn fork_fuel(fuel: &mut i64) -> &mut i64 { fuel }
/// A fork site's call in the rule form: the same on the CPU.
#[inline]
pub fn dive_res_fork(ctx: &mut Wctx, f: u16, args: &[u64]) -> Result<u64, u32> {
    dive_res(ctx, f, args)
}
/// A segment's tail call (delivering to `args[0]`): one world on the CPU.
#[inline]
pub fn tail_to(ctx: &mut Wctx, f: u16, args: &[u64]) {
    dive_to(ctx, f, args)
}
#[inline] pub fn imax(a: i64, b: i64) -> i64 { a.max(b) }
#[inline] pub fn sat_mul(a: i64, b: i64) -> i64 { a.saturating_mul(b) }
#[inline] pub fn is_err(r: &R) -> bool { r.is_err() }
#[inline] pub fn atomic_load(a: &std::sync::atomic::AtomicI64) -> i64 { a.load(std::sync::atomic::Ordering::Relaxed) }
#[inline] pub fn atomic_store(a: &std::sync::atomic::AtomicI64, v: i64) { a.store(v, std::sync::atomic::Ordering::Relaxed) }
#[inline] pub fn atomic_max(a: &std::sync::atomic::AtomicI64, v: i64) { a.fetch_max(v, std::sync::atomic::Ordering::Relaxed); }
#[inline] pub fn flag_load(a: &std::sync::atomic::AtomicBool) -> bool { a.load(std::sync::atomic::Ordering::Relaxed) }
#[inline] pub fn flag_store(a: &std::sync::atomic::AtomicBool, v: bool) { a.store(v, std::sync::atomic::Ordering::Relaxed) }

// ---- values: constructors, sharing, arrays, arithmetic ----
//
// Generic over the program's tables (`Tables`): `lin(k)` constant-folds
// per instantiation, so linear constructors move with no refcount traffic.
// The `#[inline]` hints only keep these instances in the generated crate's
// codegen unit as they were when the text lived there (measured: without
// them lexer and tree-radix run 16-21% more instructions).

/// What a generated program supplies to the value helpers.
pub trait Tables {
    /// Constructor k is statically linear: never shared, no refcount (a
    /// constant table lookup: folds for a statically known k).
    fn lin(k: u16) -> bool;
    /// unbox slot -> original ctor id (for printing)
    fn unbox_cid(slot: u64) -> u32;
    /// Share a closure value (a DUP-LAM copy in the net).
    fn dup_closure(ctx: &mut Wctx, p: u64) -> u64;
    /// Drop a closure value (the net erases it).
    fn drop_closure(ctx: &mut Wctx, p: u64);
}

/// Constructor allocation; arity <= 2 direct, wider ctors chain cells
/// (slot 0 = field, slot 1 = continuation con). The arity nibble saturates
/// at 15: any nibble > 2 means "one field + a link", so arbitrary widths
/// chain fine and walkers never rely on the nibble as an exact total.
#[inline]
pub fn mk_con<T: Tables>(ctx: &mut Wctx, k: u16, fs: &[u64]) -> u64 {
    let n = fs.len();
    if n == 0 { return con(0, k, 0); }
    if n <= 2 {
        let a = calloc::<T>(ctx, k, fs[0], if n == 2 { fs[1] } else { 0 });
        return con(a, k, n as u8);
    }
    let a = calloc::<T>(ctx, k, fs[n - 2], fs[n - 1]);
    let mut chain = con(a, k, 2);
    let mut i = n - 2;
    while i > 0 {
        i -= 1;
        let a = calloc::<T>(ctx, k, fs[i], chain);
        chain = con(a, k, (n - i).min(15) as u8);
    }
    chain
}

/// Constructor k is statically linear (see LIN): never shared, no rc.
#[inline(always)] pub fn lin<T: Tables>(k: u16) -> bool { T::lin(k) }

#[inline(always)] pub fn calloc<T: Tables>(ctx: &mut Wctx, k: u16, a: u64, b: u64) -> u32 { if lin::<T>(k) { ctx.alloc_lin(a, b) } else { ctx.alloc(a, b) } }

#[inline(always)] pub fn mk_con1<T: Tables>(ctx: &mut Wctx, k: u16, f0: u64) -> u64 { let a = calloc::<T>(ctx, k, f0, 0); con(a, k, 1) }

#[inline(always)] pub fn mk_con2<T: Tables>(ctx: &mut Wctx, k: u16, f0: u64, f1: u64) -> u64 { let a = calloc::<T>(ctx, k, f0, f1); con(a, k, 2) }

/// Consume an arity<=2 constructor: move both fields out. Unique owner
/// moves raw and frees the cell; shared increfs the fields and decrefs the
/// root. (Chained arity>2 ctors take the generic dup+free path instead.)
#[inline(always)]
pub fn consume2<T: Tables>(ctx: &mut Wctx, p: u64) -> (u64, u64) {
    let a = con_addr(p);
    let c = ctx.cell(a);
    if lin::<T>(con_tag(p)) || ctx.rc_unique(a) {
        ctx.free(a);
        (c[0], c[1])
    } else {
        let f0 = dup_val::<T>(ctx, c[0]);
        let f1 = dup_val::<T>(ctx, c[1]);
        let _ = ctx.rc_dec(a);
        (f0, f1)
    }
}

/// consume2 with the constructor statically known (the match arm's ctor):
/// `lin::<T>(k)` constant-folds, so linear types move with zero rc traffic.
#[inline(always)]
pub fn consume2k<T: Tables>(ctx: &mut Wctx, p: u64, k: u16) -> (u64, u64) {
    let a = con_addr(p);
    let c = ctx.cell(a);
    if lin::<T>(k) || ctx.rc_unique(a) {
        ctx.free(a);
        (c[0], c[1])
    } else {
        let f0 = dup_val::<T>(ctx, c[0]);
        let f1 = dup_val::<T>(ctx, c[1]);
        let _ = ctx.rc_dec(a);
        (f0, f1)
    }
}

#[inline(always)]
pub fn consume2r<T: Tables>(ctx: &mut Wctx, p: u64, k: u16) -> (u64, u64, u32) {
    let a = con_addr(p);
    let c = ctx.cell(a);
    if lin::<T>(k) || ctx.rc_unique(a) {
        (c[0], c[1], a)
    } else {
        let f0 = dup_val::<T>(ctx, c[0]);
        let f1 = dup_val::<T>(ctx, c[1]);
        let _ = ctx.rc_dec(a);
        (f0, f1, NOTOK)
    }
}

/// Consume a chained (arity > 2) constructor: move every field out and
/// free only the chain cells when this was the last reference (always,
/// for linear types); otherwise share the fields and drop the root.
#[inline(always)]
pub fn consume_chain<T: Tables, const N: usize>(ctx: &mut Wctx, p: u64, k: u16) -> [u64; N] {
    let a = con_addr(p);
    let mut out = [0u64; N];
    if N == 0 {
        return out; // nullary: no cell
    }
    if lin::<T>(k) || ctx.rc_unique(a) {
        let mut cur = a;
        for slot in out.iter_mut().take(N.saturating_sub(2)) {
            let c = ctx.cell(cur);
            ctx.free(cur);
            *slot = c[0];
            cur = con_addr(c[1]);
        }
        let c = ctx.cell(cur);
        ctx.free(cur);
        if N >= 2 {
            out[N - 2] = c[0];
            out[N - 1] = c[1];
        } else {
            out[0] = c[0];
        }
    } else {
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = dup_val::<T>(ctx, field(ctx, p, i));
        }
        free_val::<T>(ctx, p);
    }
    out
}

/// Build a 2-field ctor in a reuse token's cell (or allocate).
#[inline(always)]
pub fn mk_con2r<T: Tables>(ctx: &mut Wctx, tok: u32, k: u16, f0: u64, f1: u64) -> u64 {
    if tok == NOTOK {
        return mk_con2::<T>(ctx, k, f0, f1);
    }
    ctx.set(tok, 0, f0);
    ctx.set(tok, 1, f1);
    if !lin::<T>(k) {
        ctx.rc_set1(tok);
    }
    con(tok, k, 2)
}

/// Share a value: O(1) refcount bump on the root cell (immediates copy
/// for free; a >2-arity chain is owned by its first cell).
#[inline(always)]
pub fn dup_val<T: Tables>(ctx: &mut Wctx, p: u64) -> u64 {
    match tag(p) {
        t if t >= TU => p,
        T_LAM => T::dup_closure(ctx, p),
        T_FLO => {
            ctx.rc_inc((p & M56) as u32);
            p
        }
        T_CON => {
            if con_ar(p) > 0 {
                debug_assert!(!lin::<T>(con_tag(p)), "share of a statically linear value");
                ctx.rc_inc(con_addr(p));
            }
            p
        }
        T_ARR => {
            arr_rc(p).fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            p
        }
        _ => p,
    }
}

#[inline]
pub fn arr_new<T: Tables>(ctx: &mut Wctx, n: i64, v: u64) -> u64 {
    if n < 0 {
        panic!("negative array size {}", n);
    }
    if tag(v) == T_NUM {
        return arr_new_raw(n, sh(v));
    }
    let n = n as usize;
    if !is_heap(v) {
        return arr_alloc_fill(n, v);
    }
    let p = arr_alloc(n);
    let e = arr_elems(p);
    for k in 0..n {
        // SAFETY: k < n, inside the block
        unsafe { *e.add(k) = if k + 1 == n { v } else { dup_val::<T>(ctx, v) } }
    }
    if n == 0 {
        free_val::<T>(ctx, v);
    } else {
        arr_mark_boxed(p, v);
    }
    p
}

#[inline(always)]
pub fn arr_get<T: Tables>(ctx: &mut Wctx, a: u64, i: i64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    if arr_raw(a) {
        return arr_elem(a, i as usize);
    }
    // SAFETY: bounds checked
    dup_val::<T>(ctx, unsafe { *arr_elems(a).add(i as usize) })
}

#[inline(always)]
pub fn arr_set<T: Tables>(ctx: &mut Wctx, a: u64, i: i64, v: u64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    let a = if arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1 { a } else { arr_copy::<T>(ctx, a) };
    if arr_raw(a) {
        if tag(v) == T_NUM {
            // SAFETY: bounds checked; `a` is now uniquely ours
            unsafe { *arr_elems(a).add(i as usize) = sh(v) as u64 };
            return a;
        }
        arr_unraw(a);
    }
    // SAFETY: bounds checked; `a` is now uniquely ours
    let slot = unsafe { arr_elems(a).add(i as usize) };
    free_val::<T>(ctx, unsafe { *slot });
    unsafe { *slot = v };
    arr_mark_boxed(a, v);
    a
}

#[inline(always)]
pub fn arr_set_i<T: Tables>(ctx: &mut Wctx, a: u64, i: i64, v: u64) -> u64 {
    let n = arr_len_of(a);
    if i < 0 || i as usize >= n {
        arr_oob(i, n);
    }
    let a = if arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1 { a } else { arr_copy::<T>(ctx, a) };
    debug_assert!(arr_raw(a) || n == 0);
    // SAFETY: bounds checked; `a` is now uniquely ours
    unsafe { *arr_elems(a).add(i as usize) = sh(v) as u64 };
    a
}

/// Take `a` as the unique reference (copy when shared).
#[inline(always)]
pub fn arr_own<T: Tables>(ctx: &mut Wctx, a: u64) -> u64 {
    if arr_rc(a).load(std::sync::atomic::Ordering::Acquire) == 1 { a } else { arr_copy::<T>(ctx, a) }
}

#[cold]
#[inline(never)]
pub fn arr_copy<T: Tables>(ctx: &mut Wctx, a: u64) -> u64 {
    let n = arr_len_of(a);
    let b = arr_alloc(n);
    if arr_boxed(a) {
        for k in 0..n {
            // SAFETY: k < n in both blocks
            unsafe { *arr_elems(b).add(k) = dup_val::<T>(ctx, *arr_elems(a).add(k)) }
        }
        // SAFETY: as arr_rc; `b` is ours
        unsafe { *arr_block(b).add(1) |= ARR_BOXED }
    } else {
        // SAFETY: n elements in both blocks, distinct allocations
        unsafe { std::ptr::copy_nonoverlapping(arr_elems(a), arr_elems(b), n) }
        if arr_raw(a) {
            // SAFETY: as arr_rc; `b` is ours
            unsafe { *arr_block(b).add(1) |= ARR_RAW }
        }
    }
    free_val::<T>(ctx, a);
    b
}

#[inline]
pub fn arr_drop<T: Tables>(ctx: &mut Wctx, p: u64) {
    if arr_rc(p).fetch_sub(1, std::sync::atomic::Ordering::AcqRel) != 1 {
        return;
    }
    if arr_boxed(p) {
        let n = arr_len_of(p);
        for k in 0..n {
            // SAFETY: k < n; the block is still allocated
            free_val::<T>(ctx, unsafe { *arr_elems(p).add(k) });
        }
    }
    arr_free_block(p);
}

/// Drop a reference; the last one tears the value down (frees the chain
/// cells and drops every field).
/// Drop a reference (immediates return at once; the teardown is out of line
/// so hot paths stay small).
#[inline(always)]
pub fn free_val<T: Tables>(ctx: &mut Wctx, p: u64) {
    let t = tag(p);
    if t != T_CON && t != T_FLO && t != T_ARR && t != T_LAM {
        return;
    }
    free_val_slow::<T>(ctx, p);
}

/// Last use of a boxed value that is only projected: move field `i` out,
/// freeing the cells and dropping the other fields (a shared value keeps
/// its fields: share field `i`, drop this reference).
#[inline]
pub fn take_field<T: Tables>(ctx: &mut Wctx, p: u64, i: usize) -> u64 {
    let root = con_addr(p);
    if !lin::<T>(con_tag(p)) && !ctx.rc_unique(root) {
        let f = dup_val::<T>(ctx, field(ctx, p, i));
        free_val::<T>(ctx, p);
        return f;
    }
    let (mut q, mut idx, mut out) = (p, 0usize, 0u64);
    loop {
        let ar = con_ar(q) as usize;
        let ca = con_addr(q);
        let c = ctx.cell(ca);
        ctx.free(ca);
        if ar > 2 {
            if idx == i {
                out = c[0];
            } else {
                free_val::<T>(ctx, c[0]);
            }
            idx += 1;
            q = c[1];
        } else {
            for (s, f) in c.iter().enumerate().take(ar) {
                if idx + s == i {
                    out = *f;
                } else {
                    free_val::<T>(ctx, *f);
                }
            }
            return out;
        }
    }
}

/// Move every field out of a boxed k-tuple at its last use, freeing its
/// cells (a shared tuple keeps its fields: share them, drop this reference).
#[inline]
pub fn untup<T: Tables, const K: usize>(ctx: &mut Wctx, p: u64) -> [u64; K] {
    let mut out = [0u64; K];
    if !lin::<T>(con_tag(p)) && !ctx.rc_unique(con_addr(p)) {
        for (i, o) in out.iter_mut().enumerate() {
            *o = dup_val::<T>(ctx, field(ctx, p, i));
        }
        free_val::<T>(ctx, p);
        return out;
    }
    let (mut q, mut idx) = (p, 0usize);
    loop {
        let ar = con_ar(q) as usize;
        let ca = con_addr(q);
        let c = ctx.cell(ca);
        ctx.free(ca);
        if ar > 2 {
            out[idx] = c[0];
            idx += 1;
            q = c[1];
        } else {
            out[idx..idx + ar].copy_from_slice(&c[..ar]);
            return out;
        }
    }
}

#[inline(never)]
pub fn free_val_slow<T: Tables>(ctx: &mut Wctx, p: u64) {
    match tag(p) {
        T_ARR => arr_drop::<T>(ctx, p),
        // a dropped closure: the net erases it (and the work pending
        // in its body)
        T_LAM => T::drop_closure(ctx, p),
        t if t >= TU => {}
        T_FLO => {
            let a = (p & M56) as u32;
            if ctx.rc_dec(a) {
                ctx.free(a);
            }
        }
        T_CON => {
            if con_ar(p) == 0 {
                return;
            }
            let root = con_addr(p);
            if !lin::<T>(con_tag(p)) && !ctx.rc_dec(root) {
                return;
            }
            // last reference: free the chain, dropping each field
            let mut q = p;
            loop {
                let ar = con_ar(q) as usize;
                let ca = con_addr(q);
                let c = ctx.cell(ca);
                ctx.free(ca);
                if ar > 2 {
                    free_val_slow::<T>(ctx, c[0]);
                    q = c[1];
                } else {
                    for s in 0..ar {
                        free_val::<T>(ctx, c[s]);
                    }
                    return;
                }
            }
        }
        _ => {}
    }
}

#[inline]
pub fn bin<T: Tables>(ctx: &mut Wctx, op: u8, a: u64, b: u64, own: u8) -> u64 {
    if tag(a) == T_NUM && tag(b) == T_NUM {
        let (x, y) = (as_i(a), as_i(b));
        return num(wrap56(match op {
            0 => x.wrapping_add(y),
            1 => x.wrapping_sub(y),
            2 => x.wrapping_mul(y),
            3 => x.wrapping_div(y),
            4 => floor_div(x, y),
            5 => py_mod(x, y),
            6 => x.wrapping_shl(y as u32),
            7 => x.wrapping_shr(y as u32),
            8 => x & y,
            9 => x | y,
            _ => x ^ y,
        }));
    }
    let (x, y) = (flo_val(ctx, a), flo_val(ctx, b));
    if own & 1 != 0 { free_val::<T>(ctx, a); }
    if own & 2 != 0 { free_val::<T>(ctx, b); }
    let v = match op { 0 => x + y, 1 => x - y, 2 => x * y, 3 => x / y, _ => panic!("op not defined on floats") };
    flo(ctx, v)
}

#[inline]
pub fn cmp<T: Tables>(ctx: &mut Wctx, op: u8, a: u64, b: u64, own: u8) -> u64 {
    let o = if tag(a) == T_NUM && tag(b) == T_NUM {
        as_i(a).partial_cmp(&as_i(b))
    } else {
        if own & 1 != 0 { free_val::<T>(ctx, a); }
        if own & 2 != 0 { free_val::<T>(ctx, b); }
        flo_val(ctx, a).partial_cmp(&flo_val(ctx, b))
    };
    let o = o.expect("incomparable values (NaN?)");
    use std::cmp::Ordering::*;
    let r = match op {
        0 => o == Less,
        1 => o != Greater,
        2 => o == Greater,
        3 => o != Less,
        4 => o == Equal,
        _ => o != Equal,
    };
    num(if r { 1 } else { 0 })
}

/// n-tuple of int zeros (the identity of the additive fold combiners).
#[inline]
pub fn zeros<T: Tables>(ctx: &mut Wctx, n: usize) -> u64 {
    let fs: Vec<u64> = (0..n).map(|_| num(0)).collect();
    mk_con::<T>(ctx, 0xFFF, &fs)
}

/// Post-run readback + printing of the delivered root value.
pub fn show<T: Tables>(eng: &Engine, p: u64) -> String {
    match tag(p) {
        t if t >= TU => format!("C{}({})", T::unbox_cid(t - TU), as_i(p)),
        T_LAM => "<closure>".to_string(),
        T_NUM => as_i(p).to_string(),
        T_FLO => format!("{:?}", f64::from_bits(eng.cell((p & M56) as u32)[0])),
        T_CON => {
            let k = con_tag(p);
            let mut q = p;
            let mut fs: Vec<String> = Vec::new();
            if con_ar(p) > 0 {
                loop {
                    let ar = con_ar(q) as usize;
                    let c = eng.cell(con_addr(q));
                    if ar > 2 {
                        fs.push(show::<T>(eng, c[0]));
                        q = c[1];
                    } else {
                        for s in 0..ar {
                            fs.push(show::<T>(eng, c[s]));
                        }
                        break;
                    }
                }
            }
            if k == 0xFFF { format!("({})", fs.join(", ")) } else { format!("C{}({})", k, fs.join(", ")) }
        }
        T_ARR => {
            let n = arr_len_of(p);
            let fs: Vec<String> = (0..n).map(|k| show::<T>(eng, arr_elem(p, k))).collect();
            format!("[{}]", fs.join(", "))
        }
        _ => panic!("unprintable result port {:#x}", p),
    }
}

