//! The software binary64 header (`device/soft64.h`), compiled as C++ on the
//! host and checked against the hardware: every f64 operation and
//! comparison, the f32 operations through f64 (subnormals included) and
//! both conversions, over random bit patterns and operands built at the
//! edges: zeros, subnormals, the normal boundary, the top exponents,
//! infinities, NaNs, rounding ties and near-equal pairs.

use mithril_core::float::{canon32, canon64, f64_op};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const DRIVER: &str = r#"
#include <stdio.h>
#include "soft64.h"
int main() {
  sf_u64 r[3];
  while (fread(r, 8, 3, stdin) == 3) {
    sf_u64 k = r[0], a = r[1], b = r[2], z = 0;
    if (k < 4) z = sf_f64_op((int)k, a, b);
    else if (k == 4) z = sf_sqrt(a);
    else if (k == 5) z = sf_lt(a, b);
    else if (k == 6) z = sf_le(a, b);
    else if (k == 7) z = sf_eq(a, b);
    else if (k < 13) z = sf_f32_op((int)(k - 8), (sf_u32)a, (sf_u32)b);
    else if (k == 13) z = sf_f32_to_f64((sf_u32)a);
    else z = sf_f64_to_f32(a);
    fwrite(&z, 8, 1, stdout);
  }
  return 0;
}
"#;

const KINDS: u64 = 15;

/// The hardware result for record (k, a, b), NaNs canonical.
fn want(k: u64, a: u64, b: u64) -> u64 {
    let (x, y) = (f64::from_bits(a), f64::from_bits(b));
    let (p, q) = (f32::from_bits(a as u32), f32::from_bits(b as u32));
    match k {
        0..=3 => f64_op(k as u8, x, y).unwrap().to_bits(),
        4 => canon64(x.sqrt()).to_bits(),
        5 => (x < y) as u64,
        6 => (x <= y) as u64,
        7 => (x == y) as u64,
        8 => canon32(p + q).to_bits() as u64,
        9 => canon32(p - q).to_bits() as u64,
        10 => canon32(p * q).to_bits() as u64,
        11 => canon32(p / q).to_bits() as u64,
        12 => canon32(p.sqrt()).to_bits() as u64,
        13 => canon64(p as f64).to_bits(),
        _ => canon32(x as f32).to_bits() as u64,
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

    fn pick<T: Copy>(&mut self, xs: &[T]) -> T {
        xs[(self.next() % xs.len() as u64) as usize]
    }

    /// A binary64 pattern from the edges: an exponent and a significand
    /// each drawn from its interesting values or at random.
    fn edge64(&mut self) -> u64 {
        let e = match self.next() % 3 {
            0 => self.pick(&[0, 1, 2, 3, 52, 53, 54, 1022, 1023, 1024, 2045, 2046, 2047]),
            1 => self.next() % 2048,
            // near the exponent of a narrowed f32 (normal, subnormal, overflow)
            _ => 896 - 30 + self.next() % 60 + self.pick(&[0, 128 + 30]),
        };
        let m = match self.next() % 6 {
            0 => 0,
            1 => self.pick(&[1, 2, (1 << 52) - 1, 1 << 51, (1 << 51) + 1]),
            2 => (self.next() >> 12) & !((1 << 29) - 1) | (1 << 28),  // an f32 tie
            3 => (self.next() >> 12) & !0xfff,                      // short significands
            _ => self.next() >> 12,
        };
        (self.next() & (1 << 63)) | (e << 52) | m
    }

    fn edge32(&mut self) -> u64 {
        let e = match self.next() % 3 {
            0 => self.pick(&[0, 1, 2, 23, 24, 126, 127, 128, 253, 254, 255]),
            _ => self.next() % 256,
        };
        let m = match self.next() % 5 {
            0 => 0,
            1 => self.pick(&[1, 2, 3, (1 << 23) - 1, 1 << 22]),
            2 => self.next() % 64,
            _ => self.next() & 0x7f_ffff,
        };
        ((self.next() & 1) << 31) | (e << 23) | m
    }

    /// An operand pair: unrelated, or the second near the first (for
    /// cancellation and ties).
    fn pair(&mut self, f32: bool) -> (u64, u64) {
        let any = |r: &mut Self| match r.next() % 4 {
            0 => r.next() >> if f32 { 32 } else { 0 },
            _ if f32 => r.edge32(),
            _ => r.edge64(),
        };
        let a = any(self);
        let b = match self.next() % 4 {
            0 => {
                let near = a.wrapping_add(self.next() % 9).wrapping_sub(4) ^ (self.next() & 1) << if f32 { 31 } else { 63 };
                if f32 { near & 0xffff_ffff } else { near }
            }
            _ => any(self),
        };
        (a, b)
    }
}

fn driver() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("soft64");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("driver.cpp");
    std::fs::write(&src, DRIVER).unwrap();
    let header = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("device");
    let exe = dir.join("driver");
    // no contraction: the header must not depend on the compiler fusing
    let out = Command::new("c++")
        .args(["-std=c++14", "-O2", "-ffp-contract=off", "-Wall", "-Werror", "-I"])
        .arg(&header)
        .arg(&src)
        .arg("-o")
        .arg(&exe)
        .output()
        .expect("a C++ compiler (c++) is needed to test device/soft64.h");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    exe
}

fn run(exe: &PathBuf, records: &[[u64; 3]]) -> Vec<u64> {
    let mut child = Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    let mut input = Vec::with_capacity(records.len() * 24);
    for r in records {
        r.iter().for_each(|w| input.extend(w.to_le_bytes()));
    }
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || stdin.write_all(&input).unwrap());
    let out = child.wait_with_output().unwrap();
    writer.join().unwrap();
    out.stdout.chunks(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect()
}

#[test]
fn soft64_matches_the_hardware_bit_for_bit() {
    let exe = driver();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut records = Vec::new();
    for i in 0..3_000_000u64 {
        let k = i % KINDS;
        let (a, b) = rng.pair((8..=13).contains(&k));
        records.push([k, a, b]);
    }
    // every class against every class, for each operation
    let specials64 = [0, 1 << 63, 1, (1 << 52) - 1, 1 << 52, 0x3ff0 << 48, 0x7fef_ffff_ffff_ffff, 0x7ff0 << 48, 0xfff0 << 48, 0x7ff8 << 48, 0x7ff0_0000_0000_0001, 0xfff8_0000_0000_1234];
    let specials32 = [0, 1 << 31, 1, 0x7f_ffff, 0x80_0000, 0x3f80_0000, 0x7f7f_ffff, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0x7f80_0001, 0xffc0_1234];
    for k in 0..KINDS {
        let sp: &[u64] = if (8..=13).contains(&k) { &specials32 } else { &specials64 };
        for &a in sp {
            for &b in sp {
                records.push([k, a, b]);
            }
        }
    }
    let got = run(&exe, &records);
    assert_eq!(got.len(), records.len());
    let mut bad = Vec::new();
    for (r, &g) in records.iter().zip(&got) {
        let w = want(r[0], r[1], r[2]);
        if g != w {
            bad.push(format!("kind {} a {:#018x} b {:#018x}: soft {g:#018x} hardware {w:#018x}", r[0], r[1], r[2]));
        }
    }
    assert!(bad.is_empty(), "{} mismatches of {}:\n{}", bad.len(), records.len(), bad[..bad.len().min(12)].join("\n"));
}

#[test]
fn f32_through_soft64_is_exact_on_every_subnormal() {
    // each f32 subnormal (and its neighbours across the normal boundary)
    // times, plus and divided by a spread of operands
    let exe = driver();
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut records = Vec::new();
    for m in 0..=0x80_0400u64 {
        let k = 8 + rng.next() % 5;
        let b = match rng.next() % 3 {
            0 => rng.edge32(),
            1 => (rng.next() % 0x80_0000) | ((rng.next() & 1) << 31),
            _ => (126 + rng.next() % 4) << 23 | (rng.next() & 0x7f_ffff),
        };
        records.push([k, m | ((m & 1) << 31), b]);
        records.push([13, m, 0]);
    }
    let got = run(&exe, &records);
    let bad = records.iter().zip(&got).filter(|(r, &g)| g != want(r[0], r[1], r[2])).count();
    assert_eq!(bad, 0, "of {}", records.len());
}
