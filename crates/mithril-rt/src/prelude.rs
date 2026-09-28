//! Runtime helpers of generated programs: the part of the generated
//! prelude that does not depend on the program (the linearity table `lin`
//! stays generated, so the helpers that branch on it are emitted with the
//! program and constant-fold there). Everything is `#[inline]`: the
//! generated crate inlines these like its own code.
#![allow(clippy::all, clippy::missing_safety_doc)]

use crate::{Redex, Wctx};

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


