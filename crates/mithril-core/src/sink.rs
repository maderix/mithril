//! Output sinks: a program's result written as bytes, not as text.
//!
//! A run prints its result as text by default. With a sink, every lane (the
//! reference interpreter, CPU workers, the GPU) instead collects the value's
//! leaves, read depth-first, and hands them to the same encoder. The encoder
//! splits the file into pieces, and threads write the pieces at their own
//! offsets. Each piece is a disjoint byte range, so the bytes cannot depend
//! on which thread finishes first, and no text is formatted or parsed.
//!
//! - `--image out.ppm`: the value is `(width, height, pixels)`; the pixels
//!   are `width * height` ints `0xRRGGBB` or `3 * width * height` channels
//!   `0..255`. Written as binary PPM.
//! - `--raw out.bin`: every leaf as 8 little-endian bytes (an int as `i64`,
//!   a float as its `f64` bits), depth-first.

use std::borrow::Cow;
use std::ops::Range;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// Where a result goes instead of standard output.
#[derive(Clone, Debug, PartialEq)]
pub enum Sink {
    Image(PathBuf),
    Raw(PathBuf),
}

impl Sink {
    /// The sink named by `--image PATH` or `--raw PATH`, if `flag` is one.
    pub fn from_flag(flag: &str, path: &str) -> Option<Sink> {
        match flag {
            "--image" => Some(Sink::Image(PathBuf::from(path))),
            "--raw" => Some(Sink::Raw(PathBuf::from(path))),
            _ => None,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Sink::Image(p) | Sink::Raw(p) => p,
        }
    }

    /// The arguments that hand this sink to a compiled program.
    pub fn args(&self) -> [String; 2] {
        let flag = match self {
            Sink::Image(_) => "--image",
            Sink::Raw(_) => "--raw",
        };
        [flag.to_string(), self.path().display().to_string()]
    }
}

/// One leaf of a value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leaf {
    Int(i64),
    Float(f64),
}

/// The leaves of a value in order. An array of ints joins as one run of
/// words, borrowed from the arena (CPU) or moved from the readback buffer
/// (GPU), so a large image is never copied leaf by leaf.
#[derive(Default)]
pub struct Leaves<'a> {
    segs: Vec<Seg<'a>>,
    len: usize,
}

enum Seg<'a> {
    Ints(Cow<'a, [u64]>),
    Mixed(Vec<Leaf>),
}

/// A run of consecutive leaves inside one segment.
enum Part<'s> {
    Ints(&'s [u64]),
    Mixed(&'s [Leaf]),
}

impl<'a> Leaves<'a> {
    pub fn new() -> Self {
        Leaves::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn push(&mut self, leaf: Leaf) {
        match self.segs.last_mut() {
            Some(Seg::Mixed(v)) => v.push(leaf),
            _ => self.segs.push(Seg::Mixed(vec![leaf])),
        }
        self.len += 1;
    }

    /// Append ints given as their `i64` bit patterns.
    pub fn extend_ints(&mut self, words: impl Into<Cow<'a, [u64]>>) {
        let words = words.into();
        self.len += words.len();
        if !words.is_empty() {
            self.segs.push(Seg::Ints(words));
        }
    }

    /// Leaf `k` (`k < len`).
    fn get(&self, mut k: usize) -> Leaf {
        for s in &self.segs {
            let n = match s {
                Seg::Ints(w) => w.len(),
                Seg::Mixed(v) => v.len(),
            };
            if k < n {
                return match s {
                    Seg::Ints(w) => Leaf::Int(w[k] as i64),
                    Seg::Mixed(v) => v[k],
                };
            }
            k -= n;
        }
        panic!("leaf index out of range")
    }

    /// Call `f` on the runs that cover leaves `range`, in order.
    fn parts(&self, range: Range<usize>, mut f: impl FnMut(Part)) {
        let mut at = 0;
        for s in &self.segs {
            let n = match s {
                Seg::Ints(w) => w.len(),
                Seg::Mixed(v) => v.len(),
            };
            let (lo, hi) = (range.start.max(at), range.end.min(at + n));
            if lo < hi {
                match s {
                    Seg::Ints(w) => f(Part::Ints(&w[lo - at..hi - at])),
                    Seg::Mixed(v) => f(Part::Mixed(&v[lo - at..hi - at])),
                }
            }
            at += n;
            if at >= range.end {
                break;
            }
        }
    }

    fn has_float(&self, range: Range<usize>) -> bool {
        let mut found = false;
        self.parts(range, |p| {
            if let Part::Mixed(v) = p {
                found |= v.iter().any(|l| matches!(l, Leaf::Float(_)));
            }
        });
        found
    }
}

/// Write `leaves` to `sink`; returns the line a run prints in place of the
/// value. `threads` bounds the writers (at least one).
pub fn write(sink: &Sink, leaves: &Leaves, threads: usize) -> Result<String, String> {
    match sink {
        Sink::Raw(path) => {
            write_pieces(path, &[], leaves.len(), 8, threads, |range, out| {
                let mut o = 0;
                leaves.parts(range, |p| match p {
                    Part::Ints(w) => {
                        for &x in w {
                            out[o..o + 8].copy_from_slice(&x.to_le_bytes());
                            o += 8;
                        }
                    }
                    Part::Mixed(v) => {
                        for l in v {
                            let x = match *l {
                                Leaf::Int(i) => i as u64,
                                Leaf::Float(f) => f.to_bits(),
                            };
                            out[o..o + 8].copy_from_slice(&x.to_le_bytes());
                            o += 8;
                        }
                    }
                });
            })?;
            Ok(format!("wrote {} ({} values)", path.display(), leaves.len()))
        }
        Sink::Image(path) => {
            let shape = || "--image: the value must be (width, height, pixels)".to_string();
            let float = || "--image: an image holds ints; the value has a float".to_string();
            if leaves.len() < 2 {
                return Err(shape());
            }
            let (w, h) = match (leaves.get(0), leaves.get(1)) {
                (Leaf::Int(w), Leaf::Int(h)) if w > 0 && h > 0 => (w as usize, h as usize),
                (Leaf::Float(_), _) | (_, Leaf::Float(_)) => return Err(float()),
                _ => return Err(shape()),
            };
            let n = leaves.len() - 2;
            let packed = n == w * h;
            if !packed && n != 3 * w * h {
                return Err(format!("--image: {w}x{h} needs {} pixels or {} channels; the value has {n}", w * h, 3 * w * h));
            }
            if leaves.has_float(2..leaves.len()) {
                return Err(float());
            }
            let header = format!("P6\n{w} {h}\n255\n").into_bytes();
            // a piece is a run of pixels (packed: 3 bytes each) or channels
            let width = if packed { 3 } else { 1 };
            write_pieces(path, &header, n, width, threads, |range, out| {
                let mut o = 0;
                let mut put = |x: i64| {
                    if packed {
                        out[o] = (x >> 16 & 255) as u8;
                        out[o + 1] = (x >> 8 & 255) as u8;
                        out[o + 2] = (x & 255) as u8;
                        o += 3;
                    } else {
                        out[o] = x.clamp(0, 255) as u8;
                        o += 1;
                    }
                };
                leaves.parts(range.start + 2..range.end + 2, |p| match p {
                    Part::Ints(ws) => ws.iter().for_each(|&x| put(x as i64)),
                    Part::Mixed(v) => v.iter().for_each(|l| put(if let Leaf::Int(i) = *l { i } else { 0 })),
                });
            })?;
            Ok(format!("wrote {} ({w}x{h})", path.display()))
        }
    }
}

/// Create `path` holding `header` then `count` items of `width` bytes.
/// `fill(items, buffer)` encodes a run of items; runs of about 1 MiB are
/// handed to up to `threads` writers, each writing at the run's offset.
fn write_pieces(path: &Path, header: &[u8], count: usize, width: usize, threads: usize, fill: impl Fn(Range<usize>, &mut [u8]) + Sync) -> Result<(), String> {
    let err = |e: std::io::Error| format!("cannot write {}: {e}", path.display());
    let file = std::fs::File::create(path).map_err(err)?;
    file.set_len((header.len() + count * width) as u64).map_err(err)?;
    file.write_all_at(header, 0).map_err(err)?;
    let per = ((1 << 20) / width).max(1);
    let pieces = count.div_ceil(per);
    let writers = threads.clamp(1, pieces.max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let failed = std::sync::Mutex::new(None::<String>);
    std::thread::scope(|s| {
        for _ in 0..writers {
            s.spawn(|| {
                let mut buf = vec![0u8; per * width];
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if k >= pieces {
                        break;
                    }
                    let items = k * per..((k + 1) * per).min(count);
                    let out = &mut buf[..items.len() * width];
                    let at = header.len() + items.start * width;
                    fill(items, out);
                    if let Err(e) = file.write_all_at(out, at as u64) {
                        *failed.lock().unwrap() = Some(err(e));
                        break;
                    }
                }
            });
        }
    });
    match failed.into_inner().unwrap() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("mithril-sink-{}-{name}", std::process::id()))
    }

    fn of(v: &[Leaf]) -> Leaves<'static> {
        let mut l = Leaves::new();
        v.iter().for_each(|x| l.push(*x));
        l
    }

    #[test]
    fn packed_and_channel_images_write_the_same_ppm_at_any_thread_count() {
        // past one 1 MiB piece, so several writers share the file
        let (w, h) = (1000usize, 400usize);
        let rgb = |i: usize| ((i * 7) % 256, (i * 13) % 256, (i * 29) % 256);
        let mut want = format!("P6\n{w} {h}\n255\n").into_bytes();
        let (mut words, mut chans) = (Vec::new(), Vec::new());
        for i in 0..w * h {
            let (r, g, b) = rgb(i);
            want.extend([r as u8, g as u8, b as u8]);
            words.push(((r << 16) | (g << 8) | b) as u64);
            chans.extend([r as u64, g as u64, b as u64]);
        }
        let head = [Leaf::Int(w as i64), Leaf::Int(h as i64)];
        let mut forms: Vec<(&str, Leaves)> = Vec::new();
        // one borrowed run, leaf by leaf, and runs split across segments
        let mut l = of(&head);
        l.extend_ints(&words[..]);
        forms.push(("packed run", l));
        forms.push(("packed leaves", of(&[&head[..], &words.iter().map(|&x| Leaf::Int(x as i64)).collect::<Vec<_>>()].concat())));
        let mut l = of(&head);
        for c in chans.chunks(77_777) {
            l.extend_ints(c.to_vec());
            l.push(Leaf::Int(0));
            l.len -= 1;
            if let Some(Seg::Mixed(v)) = l.segs.last_mut() {
                v.pop();
            }
        }
        forms.push(("channel runs", l));
        for t in [1, 3, 16] {
            for (name, v) in &forms {
                let p = tmp(&format!("{}{t}.ppm", name.replace(' ', "_")));
                assert_eq!(write(&Sink::Image(p.clone()), v, t).unwrap(), format!("wrote {} ({w}x{h})", p.display()));
                assert!(std::fs::read(&p).unwrap() == want, "{name} t{t}");
                let _ = std::fs::remove_file(p);
            }
        }
    }

    #[test]
    fn channels_clamp_and_packed_pixels_take_the_low_24_bits() {
        let p = tmp("clamp.ppm");
        write(&Sink::Image(p.clone()), &of(&[Leaf::Int(2), Leaf::Int(1), Leaf::Int(-5), Leaf::Int(300), Leaf::Int(7), Leaf::Int(1), Leaf::Int(2), Leaf::Int(3)]), 1).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"P6\n2 1\n255\n\x00\xff\x07\x01\x02\x03");
        write(&Sink::Image(p.clone()), &of(&[Leaf::Int(1), Leaf::Int(1), Leaf::Int(0x7f_123456)]), 1).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"P6\n1 1\n255\n\x12\x34\x56");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn raw_writes_every_leaf_as_eight_little_endian_bytes() {
        let mixed: Vec<Leaf> = (0..150_000i64).map(|i| if i % 5 == 0 { Leaf::Float(i as f64 * 0.5) } else { Leaf::Int(-i) }).collect();
        let ints: Vec<u64> = (0..200_000i64).map(|i| (i * 1_000_003) as u64).collect();
        let mut l = of(&mixed[..70_000]);
        l.extend_ints(&ints[..]);
        mixed[70_000..].iter().for_each(|x| l.push(*x));
        let mut want = Vec::new();
        let bits = |x: &Leaf| match *x {
            Leaf::Int(i) => i as u64,
            Leaf::Float(f) => f.to_bits(),
        };
        mixed[..70_000].iter().for_each(|x| want.extend(bits(x).to_le_bytes()));
        ints.iter().for_each(|x| want.extend(x.to_le_bytes()));
        mixed[70_000..].iter().for_each(|x| want.extend(bits(x).to_le_bytes()));
        assert_eq!(l.len(), 350_000);
        for t in [1, 4, 16] {
            let p = tmp(&format!("raw{t}.bin"));
            assert_eq!(write(&Sink::Raw(p.clone()), &l, t).unwrap(), format!("wrote {} (350000 values)", p.display()));
            assert!(std::fs::read(&p).unwrap() == want, "t{t}");
            let _ = std::fs::remove_file(p);
        }
        let p = tmp("empty.bin");
        write(&Sink::Raw(p.clone()), &Leaves::new(), 4).unwrap();
        assert!(std::fs::read(&p).unwrap().is_empty());
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn malformed_images_are_errors() {
        let p = tmp("bad.ppm");
        let img = |v: &[Leaf]| write(&Sink::Image(p.clone()), &of(v), 2);
        assert!(img(&[Leaf::Int(2)]).unwrap_err().contains("(width, height, pixels)"));
        assert!(img(&[Leaf::Int(0), Leaf::Int(1)]).unwrap_err().contains("(width, height, pixels)"));
        assert!(img(&[Leaf::Int(2), Leaf::Int(1), Leaf::Int(5)]).unwrap_err().contains("needs 2 pixels or 6 channels; the value has 1"));
        assert!(img(&[Leaf::Int(1), Leaf::Int(1), Leaf::Float(0.5)]).unwrap_err().contains("float"));
        assert!(img(&[Leaf::Float(1.0), Leaf::Int(1), Leaf::Int(5)]).unwrap_err().contains("float"));
        let e = write(&Sink::Raw(tmp("no/such/dir.bin")), &of(&[Leaf::Int(1)]), 1).unwrap_err();
        assert!(e.contains("cannot write"), "{e}");
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn flags_name_the_sinks() {
        assert_eq!(Sink::from_flag("--image", "a.ppm"), Some(Sink::Image("a.ppm".into())));
        assert_eq!(Sink::from_flag("--raw", "a.bin").unwrap().args(), ["--raw".to_string(), "a.bin".to_string()]);
        assert_eq!(Sink::from_flag("--threads", "4"), None);
    }
}
