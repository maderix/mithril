//! Every scalar operation of the Metal backend, run on the GPU and compared
//! with the CPU's definition bit for bit: ints (wrapping, shifts at and past
//! 64, division signs, MIN / -1), binary32 over normal, subnormal, zero,
//! infinite and NaN operands (NaN results compared canonical, as they are
//! observed), the binary16 conversions over every pattern, and software
//! binary64.

#![cfg(target_os = "macos")]

use mithril_core::float as fl;
use mithril_front::ast::BinOp;
use mithril_front::core::int_op;
use mithril_metal::metal::{Device, Dispatch};

const KERNEL: &str = r#"
kernel void conformance(device const ulong *rec [[buffer(0)]], device ulong *out [[buffer(1)]],
                        uint i [[thread_position_in_grid]]) {
  int op = (int)rec[3 * i];
  i64 a = (i64)rec[3 * i + 1], b = (i64)rec[3 * i + 2], z = 0;
  switch (op) {
  case 0: z = i_add(a, b); break;
  case 1: z = i_sub(a, b); break;
  case 2: z = i_mul(a, b); break;
  case 3: z = idiv(a, b); break;
  case 4: z = floor_div(a, b); break;
  case 5: z = py_mod(a, b); break;
  case 6: z = i_shl(a, b); break;
  case 7: z = i_shr(a, b); break;
  case 8: z = a & b; break;
  case 9: z = a | b; break;
  case 10: z = a ^ b; break;
  case 11: z = a < b; break;
  case 12: z = a <= b; break;
  case 13: z = a == b; break;
  case 20: z = f32_add(a, b); break;
  case 21: z = f32_sub(a, b); break;
  case 22: z = f32_mul(a, b); break;
  case 23: z = f32_div(a, b); break;
  case 24: z = f32_sqrt(a); break;
  case 25: z = f32_lt(a, b); break;
  case 26: z = f32_le(a, b); break;
  case 27: z = f32_from_u32(a); break;
  case 28: z = f32_to_u32(a); break;
  case 29: z = f32_canon(a); break;
  case 30: z = f16_to_f32(a); break;
  case 31: z = f32_to_f16(a); break;
  case 40: case 41: case 42: case 43: z = f64_op(op - 40, a, b); break;
  case 44: z = f64_lt(a, b); break;
  case 45: z = f64_le(a, b); break;
  default: z = f64_eq(a, b); break;
  }
  out[i] = (ulong)z;
}
"#;

const INT_OPS: [BinOp; 11] = [BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::Div, BinOp::FloorDiv, BinOp::Mod, BinOp::Shl, BinOp::Shr, BinOp::BitAnd, BinOp::BitOr, BinOp::BitXor];

/// The CPU's result for record (op, a, b); f32 and f16 arithmetic NaNs
/// canonical (their bits are observed only canonical).
fn want(op: u64, a: u64, b: u64) -> u64 {
    let (x, y) = (a as i64, b as i64);
    let c32 = |v: i64| fl::f32_canon(v) as u64;
    match op {
        // a zero divisor gives 0 on the devices (the CPU reports an error)
        0..=10 => int_op(INT_OPS[op as usize], x, y).unwrap_or(0) as u64,
        11 => (x < y) as u64,
        12 => (x <= y) as u64,
        13 => (x == y) as u64,
        20 => c32(fl::f32_add(x, y)),
        21 => c32(fl::f32_sub(x, y)),
        22 => c32(fl::f32_mul(x, y)),
        23 => c32(fl::f32_div(x, y)),
        24 => c32(fl::f32_sqrt(x)),
        25 => fl::f32_lt(x, y) as u64,
        26 => fl::f32_le(x, y) as u64,
        27 => fl::f32_from_u32(x) as u64,
        28 => fl::f32_to_u32(x) as u64,
        29 => fl::f32_canon(x) as u64,
        30 => fl::f16_to_f32(x) as u64,
        31 => fl::f32_to_f16(x) as u64,
        40..=43 => fl::f64_op((op - 40) as u8, f64::from_bits(a), f64::from_bits(b)).unwrap().to_bits(),
        44 => (f64::from_bits(a) < f64::from_bits(b)) as u64,
        45 => (f64::from_bits(a) <= f64::from_bits(b)) as u64,
        _ => (f64::from_bits(a) == f64::from_bits(b)) as u64,
    }
}

/// The device's result, with the same canonical view of f32 NaNs.
fn seen(op: u64, z: u64) -> u64 {
    match op {
        20..=24 => fl::f32_canon(z as i64) as u64,
        _ => z,
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn pick(&mut self, xs: &[u64]) -> u64 {
        xs[(self.next() % xs.len() as u64) as usize]
    }

    fn int(&mut self) -> u64 {
        match self.next() % 4 {
            0 => self.pick(&[0, 1, 2, 63, 64, 65, 127, u64::MAX, u64::MAX - 1, 1 << 63, (1 << 63) - 1, 1 << 55]),
            1 => (self.next() % 200).wrapping_sub(100),
            _ => self.next(),
        }
    }

    fn f32(&mut self) -> u64 {
        let e = match self.next() % 3 {
            0 => self.pick(&[0, 1, 2, 23, 24, 25, 100, 126, 127, 128, 150, 200, 253, 254, 255]),
            _ => self.next() % 256,
        };
        let m = match self.next() % 5 {
            0 => 0,
            1 => self.pick(&[1, 2, 3, (1 << 23) - 1, 1 << 22, (1 << 22) + 1]),
            2 => self.next() % 64,
            _ => self.next() & 0x7f_ffff,
        };
        ((self.next() & 1) << 31) | (e << 23) | m
    }

    fn f64(&mut self) -> u64 {
        let e = match self.next() % 3 {
            0 => self.pick(&[0, 1, 2, 52, 1022, 1023, 1024, 2045, 2046, 2047]),
            _ => self.next() % 2048,
        };
        let m = match self.next() % 4 {
            0 => 0,
            1 => self.pick(&[1, (1 << 52) - 1, 1 << 51]),
            _ => self.next() >> 12,
        };
        (self.next() & (1 << 63)) | (e << 52) | m
    }
}

fn records() -> Vec<[u64; 3]> {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let ops: Vec<u64> = (0..=13).chain(20..=31).chain(40..=46).collect();
    let mut recs = Vec::new();
    for i in 0..2_000_000usize {
        let op = ops[i % ops.len()];
        let (a, b) = match op {
            0..=13 => (rng.int(), rng.int()),
            27 => (rng.next() & 0xffff_ffff, 0),
            30 => (rng.next() & 0xffff, 0),
            40..=46 => (rng.f64(), rng.f64()),
            _ => {
                // an operand pair, the second near the first half the time
                let a = rng.f32();
                let b = if rng.next() & 1 == 0 { rng.f32() } else { (a + rng.next() % 5).wrapping_sub(2) & 0xffff_ffff };
                (a, b)
            }
        };
        recs.push([op, a, b]);
    }
    // every binary16 pattern, and every f32 subnormal through narrowing
    for h in 0..=0xffffu64 {
        recs.push([30, h, 0]);
    }
    for m in (0..0x80_0000u64).step_by(7) {
        recs.push([31, m, 0]);
        recs.push([20 + m % 5, m | ((m & 2) << 30), rng.f32()]);
    }
    recs
}

#[test]
fn every_scalar_operation_is_bit_equal_to_the_cpu() {
    let Some(dev) = Device::new() else {
        eprintln!("no Metal device: skipped");
        return;
    };
    let src = format!("{}\n{KERNEL}", mithril_metal::ops_source());
    let lib = dev.compile(&src).unwrap_or_else(|e| panic!("MSL: {e}"));
    let pso = dev.pipeline(&lib, "conformance", 1).unwrap();
    let recs = records();
    let mut input = dev.buffer(recs.len() * 24);
    input.words_mut().copy_from_slice(&recs.concat());
    let out = dev.buffer(recs.len() * 8);
    dev.run(&[Dispatch { pipeline: &pso, buffers: vec![&input, &out], reached: vec![], threads: recs.len(), group: 256 }]).unwrap();
    let mut bad = Vec::new();
    let mut per_op = std::collections::BTreeMap::<u64, usize>::new();
    for (r, &z) in recs.iter().zip(out.words()) {
        let (got, w) = (seen(r[0], z), want(r[0], r[1], r[2]));
        if got != w {
            *per_op.entry(r[0]).or_default() += 1;
            if bad.len() < 12 {
                bad.push(format!("op {} a {:#x} b {:#x}: metal {got:#x} cpu {w:#x}", r[0], r[1], r[2]));
            }
        }
    }
    assert!(bad.is_empty(), "mismatches per op {per_op:?} of {}:\n{}", recs.len(), bad.join("\n"));
}
