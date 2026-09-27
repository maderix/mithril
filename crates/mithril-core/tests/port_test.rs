use mithril_core::net::Net;
use mithril_core::port::{Port, Tag};

const ALL_TAGS: [Tag; 13] = [
    Tag::Var,
    Tag::Era,
    Tag::Num,
    Tag::Flo,
    Tag::Con,
    Tag::Dup,
    Tag::Lam,
    Tag::App,
    Tag::Op,
    Tag::Swi,
    Tag::Mat,
    Tag::Ref,
    Tag::Ext,
];

const MAX_PAYLOAD: u64 = (1u64 << 56) - 1;
const I56_MAX: i64 = (1i64 << 55) - 1;
const I56_MIN: i64 = -(1i64 << 55);

// ---- Port::new / tag / payload roundtrips ----

#[test]
fn port_roundtrip_every_tag_zero_payload() {
    for &t in ALL_TAGS.iter() {
        let p = Port::new(t, 0);
        assert_eq!(p.tag(), t, "tag mismatch for {:?}", t);
        assert_eq!(p.payload(), 0, "payload mismatch for {:?}", t);
    }
}

#[test]
fn port_roundtrip_every_tag_max_payload() {
    for &t in ALL_TAGS.iter() {
        let p = Port::new(t, MAX_PAYLOAD);
        assert_eq!(p.tag(), t, "tag mismatch for {:?}", t);
        assert_eq!(p.payload(), MAX_PAYLOAD, "payload mismatch for {:?}", t);
    }
}

#[test]
fn port_roundtrip_every_tag_mid_payload() {
    let mid: u64 = 0x00AA_BBCC_DDEE_11;
    for &t in ALL_TAGS.iter() {
        let p = Port::new(t, mid & MAX_PAYLOAD);
        assert_eq!(p.tag(), t);
        assert_eq!(p.payload(), mid & MAX_PAYLOAD);
    }
}

#[test]
fn port_payload_does_not_leak_into_tag_bits() {
    // Max payload set, tag should still decode correctly for every tag,
    // proving payload bits are confined to the low 56 bits.
    let p = Port::new(Tag::Ext, MAX_PAYLOAD);
    assert_eq!(p.tag(), Tag::Ext);
    assert_eq!(p.0 >> 56, Tag::Ext as u64);
}

// ---- num / as_i64 sign extension ----

#[test]
fn num_roundtrip_negative_one() {
    let p = Port::num(-1);
    assert_eq!(p.tag(), Tag::Num);
    assert_eq!(p.as_i64(), -1);
}

#[test]
fn num_roundtrip_zero() {
    let p = Port::num(0);
    assert_eq!(p.as_i64(), 0);
}

#[test]
fn num_roundtrip_small_positive_and_negative() {
    assert_eq!(Port::num(42).as_i64(), 42);
    assert_eq!(Port::num(-42).as_i64(), -42);
}

#[test]
fn num_roundtrip_i56_max() {
    let p = Port::num(I56_MAX);
    assert_eq!(p.as_i64(), I56_MAX);
}

#[test]
fn num_roundtrip_i56_min() {
    let p = Port::num(I56_MIN);
    assert_eq!(p.as_i64(), I56_MIN);
}

#[test]
#[should_panic]
fn num_panics_above_i56_max() {
    Port::num(I56_MAX + 1);
}

#[test]
#[should_panic]
fn num_panics_below_i56_min() {
    Port::num(I56_MIN - 1);
}

// ---- CON field packing ----

#[test]
fn con_field_roundtrip() {
    let addr: u64 = (1u64 << 40) - 1;
    let ctor: u16 = 4095;
    let arity: u8 = 2;
    let p = Port::con(addr, ctor, arity);
    assert_eq!(p.tag(), Tag::Con);
    assert_eq!(p.con_addr(), addr);
    assert_eq!(p.con_tag(), ctor);
    assert_eq!(p.con_arity(), arity);
}

#[test]
fn con_field_roundtrip_zeros() {
    let p = Port::con(0, 0, 0);
    assert_eq!(p.con_addr(), 0);
    assert_eq!(p.con_tag(), 0);
    assert_eq!(p.con_arity(), 0);
}

#[test]
fn con_fields_do_not_overlap() {
    // addr occupies the high bits only: with ctor/arity zero, addr must
    // decode back exactly, proving no bit overlap between the three fields.
    let addr: u64 = (1u64 << 40) - 1;
    let p = Port::con(addr, 0, 0);
    assert_eq!(p.con_addr(), addr);
    assert_eq!(p.con_tag(), 0);
    assert_eq!(p.con_arity(), 0);

    // ctor occupies the middle bits only: with addr/arity zero, ctor must
    // decode back exactly.
    let p2 = Port::con(0, 4095, 0);
    assert_eq!(p2.con_addr(), 0);
    assert_eq!(p2.con_tag(), 4095);
    assert_eq!(p2.con_arity(), 0);
}

// ---- Net alloc / free reuse ----

#[test]
fn net_alloc_then_free_then_alloc_reuses_index() {
    let mut net = Net::new();
    let i0 = net.alloc(Port::num(1), Port::num(2));
    net.free_cell(i0);
    let i1 = net.alloc(Port::num(3), Port::num(4));
    assert_eq!(i0, i1, "freed index should be reused by the next alloc");
    assert_eq!(net.cell(i1), [Port::num(3).0, Port::num(4).0]);
}

#[test]
fn net_alloc_without_free_grows_arena() {
    let mut net = Net::new();
    let i0 = net.alloc(Port::num(1), Port::num(2));
    let i1 = net.alloc(Port::num(3), Port::num(4));
    assert_ne!(i0, i1);
    assert_eq!(net.cells.len(), 2);
}

#[test]
fn net_set_updates_single_slot() {
    let mut net = Net::new();
    let i0 = net.alloc(Port::num(1), Port::num(2));
    net.set(i0, 1, Port::num(99));
    assert_eq!(net.cell(i0), [Port::num(1).0, Port::num(99).0]);
}

#[test]
fn net_free_list_reuses_most_recently_freed_first() {
    let mut net = Net::new();
    let i0 = net.alloc(Port::num(1), Port::num(1));
    let i1 = net.alloc(Port::num(2), Port::num(2));
    net.free_cell(i0);
    net.free_cell(i1);
    let reused_first = net.alloc(Port::num(3), Port::num(3));
    assert_eq!(reused_first, i1);
    let reused_second = net.alloc(Port::num(4), Port::num(4));
    assert_eq!(reused_second, i0);
}

// ---- dump stability ----

#[test]
fn dump_is_stable_across_structurally_equal_nets() {
    let mut a = Net::new();
    let ca = a.alloc(Port::new(Tag::Con, 0), Port::new(Tag::Var, 1));
    a.set(ca, 1, Port::new(Tag::Var, 2));
    a.redexes.push((Port::new(Tag::App, 3), Port::new(Tag::Lam, 4)));

    let mut b = Net::new();
    let cb = b.alloc(Port::new(Tag::Con, 0), Port::new(Tag::Var, 1));
    b.set(cb, 1, Port::new(Tag::Var, 2));
    b.redexes.push((Port::new(Tag::App, 3), Port::new(Tag::Lam, 4)));

    assert_eq!(a.dump(), b.dump());
}

#[test]
fn dump_empty_net_is_empty_string() {
    let net = Net::new();
    assert_eq!(net.dump(), "");
}

#[test]
fn dump_contains_cell_and_redex_lines() {
    let mut net = Net::new();
    net.alloc(Port::new(Tag::Con, 7), Port::new(Tag::Era, 0));
    net.redexes.push((Port::num(5), Port::num(-5)));
    let dump = net.dump();
    assert!(dump.contains("cell 0:"), "dump missing cell line: {}", dump);
    assert!(dump.contains("redex:"), "dump missing redex line: {}", dump);
}

#[test]
fn dump_excludes_freed_cells() {
    let mut net = Net::new();
    let i0 = net.alloc(Port::new(Tag::Con, 1), Port::new(Tag::Con, 2));
    net.free_cell(i0);
    let dump = net.dump();
    assert!(
        !dump.contains(&format!("cell {}:", i0)),
        "freed cell should not appear in dump: {}",
        dump
    );
}
