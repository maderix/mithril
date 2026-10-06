//! Range requests on the GPU (`RangeExec`) with handwritten leaves: a
//! wrapping sum, a fill that reads a borrowed array, an index out of
//! bounds (the request is declined and the array untouched), recursion that
//! makes the nesting limit grow, recursion past the deepest limit, and
//! binary32 on subnormals (fast pass, then the exact pass on the marked
//! indices), checked against the CPU's binary32.

#![cfg(target_os = "macos")]

use mithril_metal::range::{Arg, RangeExec};

const LEAVES: &str = r#"// mithril: frames 2
// fold 0: the sum of i * i, wrapping
i64 sq(thread i64 *fuel, i64 lo, i64 hi) {
  i64 s = 0;
  for (i64 i = lo; i < hi; i++) s = i_add(s, i_mul(i, i * 0x9e3779b97f4a7c15l));
  return s;
}
// fold 1: a[i] = b[i % len(b)] * 3 + i
i64 fill(thread i64 *fuel, i64 lo, i64 hi, i64 a, i64 b) {
  for (i64 i = lo; i < hi; i++) arr_set_u(a, i, arr_get_r(b, i % (i64)arr_len_of(b)) * 3 + i);
  return a;
}
// fold 2: a[i + 1] (the last index is out of bounds)
i64 past(thread i64 *fuel, i64 lo, i64 hi, i64 a) {
  for (i64 i = lo; i < hi; i++) arr_set_u(a, i + 1, 7);
  return a;
}
// folds 3 and 4: guarded recursion n deep (the nesting depth by value)
i64 down(thread i64 *fuel, i64 dl, i64 n) {
  if (dl >= DEEP_LIMIT) { fuel[1] |= DEEP_FAULT; return 0; }
  return n == 0 ? 0 : down(fuel, dl + 1, n - 1) + 1;
}
// fold 5: x / 3 * 0.5 + x for x the subnormal with bits i: the GPU flushes
// subnormals, so the fast pass marks these indices and the exact pass
// computes them; fills a[i] and sums the bits
i64 tiny(thread i64 *fuel, i64 i, i64 a) {
  i64 x = i & 0x807fffff;
  i64 y = m_f32_add(fuel, m_f32_mul(fuel, m_f32_div(fuel, x, 0x40400000), 0x3f000000), x);
  arr_set_u(a, i, (u64)y);
  return y;
}
i64 prog_range_leaf(uint fid, i64 i, device const ulong *args, thread i64 *fuel) {
  switch (fid) {
  case 0: return sq(fuel, i, i + 1);
  case 1: return fill(fuel, i, i + 1, (i64)args[2], (i64)args[3]);
  case 2: return past(fuel, i, i + 1, (i64)args[2]);
  case 3: return down(fuel, 0, 40 + i % 3);
  case 5: return tiny(fuel, i, (i64)args[2]);
  default: return down(fuel, 0, 100000);
  }
}
"#;

fn block(len: usize, f: impl Fn(usize) -> u64) -> Vec<u64> {
    let mut b = vec![1, len as u64 | (1 << 62)];
    b.extend((0..len).map(f));
    b
}

#[test]
fn range_requests_run_on_the_gpu_or_decline() {
    let ex = match RangeExec::new(LEAVES) {
        Ok(e) => e,
        Err(e) if e.contains("no Metal device") => return,
        Err(e) => panic!("{e}"),
    };
    // a sum over a range larger than one group, from an offset
    let want = (5..200_005i64).fold(0u64, |s, i| s.wrapping_add((i.wrapping_mul(i.wrapping_mul(0x9e3779b97f4a7c15u64 as i64))) as u64));
    assert_eq!(ex.run(0, 5, 200_005, &mut []), Some(want));
    // a fill reading a borrowed array; the borrowed one is not written back
    let mut a = block(1000, |_| 0);
    let mut b = block(37, |k| (k * k) as u64);
    let b_before = b.clone();
    let r = ex.run(1, 0, 1000, &mut [Arg::Array { block: &mut a, write: true }, Arg::Array { block: &mut b, write: false }]);
    assert!(r.is_some());
    assert!((0..1000).all(|i| a[2 + i] == ((i % 37) * (i % 37) * 3 + i) as u64));
    assert_eq!(b, b_before);
    // out of bounds: declined, and the array is as it was
    let mut c = block(64, |k| k as u64);
    let before = c.clone();
    assert_eq!(ex.run(2, 0, 64, &mut [Arg::Array { block: &mut c, write: true }]), None);
    assert_eq!(c, before);
    // recursion 42 deep: the limit grows from the start (32) until it holds
    let want: u64 = (0..10_000u64).map(|i| 40 + i % 3).sum();
    assert_eq!(ex.run(3, 0, 10_000, &mut []), Some(want));
    // and again at once, on the grown stack
    assert_eq!(ex.run(3, 0, 10_000, &mut []), Some(want));
    // recursion past the deepest limit: declined (the CPU runs it)
    assert_eq!(ex.run(4, 0, 64, &mut []), None);
}

#[test]
fn binary32_on_subnormals_is_exact_through_the_exact_pass() {
    use mithril_core::float as fl;
    let ex = match RangeExec::new(LEAVES) {
        Ok(e) => e,
        Err(e) if e.contains("no Metal device") => return,
        Err(e) => panic!("{e}"),
    };
    let n = 1usize << 20;
    let mut a = block(n, |_| 0);
    let sum = ex.run(5, 0, n as i64, &mut [Arg::Array { block: &mut a, write: true }]).unwrap();
    let mut want_sum = 0u64;
    for i in 0..n as i64 {
        let x = i & 0x807f_ffff;
        let y = fl::f32_add(fl::f32_mul(fl::f32_div(x, 0x4040_0000), 0x3f00_0000), x);
        assert_eq!(a[2 + i as usize], y as u64, "index {i}");
        want_sum = want_sum.wrapping_add(y as u64);
    }
    assert_eq!(sum, want_sum);
}

#[test]
fn leaves_without_the_header_are_refused() {
    let Err(e) = RangeExec::new("i64 prog_range_leaf() { return 0; }") else { panic!("accepted") };
    assert!(e.contains("no range leaves"), "{e}");
    let Err(e) = RangeExec::new("") else { panic!("accepted") };
    assert!(e.contains("no range leaves"), "{e}");
}
