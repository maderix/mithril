//! Port encoding: a `Port` is a single u64 with an 8-bit tag in the high
//! byte and a 56-bit payload in the low bits. This is the atomic unit the
//! interaction-net runtime operates on.

/// Mask selecting the low 56 payload bits of a `u64`.
const MASK56: u64 = (1u64 << 56) - 1;

/// Largest value representable in the 56-bit two's-complement `int` type.
pub const I56_MAX: i64 = (1i64 << 55) - 1;
/// Smallest value representable in the 56-bit two's-complement `int` type.
pub const I56_MIN: i64 = -(1i64 << 55);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Port(pub u64); // tag:8 (high) | payload:56 (low)

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tag {
    Var = 0,
    Era = 1,
    Num = 2,
    Flo = 3,
    Con = 4,
    Dup = 5,
    Lam = 6,
    App = 7,
    Op = 8,
    Swi = 9,
    Mat = 10,
    Ref = 11,
    Ext = 12,
    /// Runtime only: a continuation. A value meeting `Kont(parent)` is
    /// delivered to the engine record `parent` (the compiled world's
    /// destination), never copied or erased.
    Kont = 13,
    /// Runtime only: an array block (a heap value the rules treat as opaque).
    Arr = 14,
    /// Runtime only: any other value form (an unboxed constructor rides
    /// tag bits 16.. with the field in the payload). The rules never
    /// inspect it; the program copies/erases/matches it.
    Other = 15,
}

impl Tag {
    fn from_u8(v: u8) -> Tag {
        match v {
            0 => Tag::Var,
            1 => Tag::Era,
            2 => Tag::Num,
            3 => Tag::Flo,
            4 => Tag::Con,
            5 => Tag::Dup,
            6 => Tag::Lam,
            7 => Tag::App,
            8 => Tag::Op,
            9 => Tag::Swi,
            10 => Tag::Mat,
            11 => Tag::Ref,
            12 => Tag::Ext,
            13 => Tag::Kont,
            14 => Tag::Arr,
            _ => Tag::Other,
        }
    }
}

impl Port {
    /// Build a port from a tag and a 56-bit payload.
    pub fn new(t: Tag, payload: u64) -> Port {
        debug_assert!(payload < (1u64 << 56), "payload {} exceeds 56 bits", payload);
        Port(((t as u64) << 56) | (payload & MASK56))
    }

    pub fn tag(self) -> Tag {
        Tag::from_u8((self.0 >> 56) as u8)
    }

    pub fn payload(self) -> u64 {
        self.0 & MASK56
    }

    /// Encode a signed 56-bit integer as a `Num` port. Panics if `v` is
    /// outside the representable i56 range.
    pub fn num(v: i64) -> Port {
        if !(I56_MIN..=I56_MAX).contains(&v) {
            panic!("num({}) out of i56 range [{}, {}]", v, I56_MIN, I56_MAX);
        }
        Port::new(Tag::Num, (v as u64) & MASK56)
    }

    /// Decode this port's payload as a sign-extended 56-bit integer.
    pub fn as_i64(self) -> i64 {
        let p = self.payload();
        // Shift the 56-bit field to the top of a 64-bit word, then
        // arithmetic-shift back down to sign-extend from bit 55.
        ((p << 8) as i64) >> 8
    }

    // CON payload layout: addr:40 | ctor_tag:12 | arity:4 (high to low).
    pub fn con(addr: u64, ctor: u16, arity: u8) -> Port {
        debug_assert!(addr < (1u64 << 40), "con addr {} exceeds 40 bits", addr);
        debug_assert!((ctor as u64) < (1u64 << 12), "con ctor {} exceeds 12 bits", ctor);
        debug_assert!((arity as u64) < (1u64 << 4), "con arity {} exceeds 4 bits", arity);
        let payload = (addr << 16) | ((ctor as u64) << 4) | (arity as u64);
        Port::new(Tag::Con, payload)
    }

    pub fn con_addr(self) -> u64 {
        self.payload() >> 16
    }

    pub fn con_tag(self) -> u16 {
        ((self.payload() >> 4) & 0xFFF) as u16
    }

    pub fn con_arity(self) -> u8 {
        (self.payload() & 0xF) as u8
    }
}
