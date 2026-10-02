use mithril_rt::{prelude::*, DiveResult, Engine, Program, Redex, Wctx, ROOT};

struct Cells;
impl Tables for Cells {
    fn lin(_: u16) -> bool {
        false
    }
    fn unbox_cid(_: u64) -> u32 {
        0
    }
    fn dup_closure(_: &mut Wctx, _: u64) -> u64 {
        unreachable!()
    }
    fn drop_closure(_: &mut Wctx, _: u64) {
        unreachable!()
    }
}
impl Program for Cells {
    fn n_rules(&self) -> usize {
        1
    }
    fn rule_cost(&self, _: u16) -> u32 {
        1
    }
    fn fire(&self, _: u16, _: Redex, ctx: &mut Wctx) {
        for width in [0, 1, 2, 3, 4, 16, 33] {
            let fields: Vec<_> = (0..width)
                .map(|i| {
                    if i % 2 == 0 {
                        num(-1099511627776 + i as i64)
                    } else {
                        ic(0, 4294967295)
                    }
                })
                .collect();
            let node = mk_con::<Self>(ctx, 0, &fields);
            let copy = dup_val::<Self>(ctx, node);
            for _ in 0..7 {
                let expected = [field(ctx, node, 0), field(ctx, node, 1)];
                assert_eq!(read_pair(ctx, node), expected);
                assert_eq!(read_pair(ctx, copy), expected);
            }
            free_val::<Self>(ctx, node);
            if width > 0 {
                assert_eq!(read_pair(ctx, copy)[0], fields[0]);
            }
            free_val::<Self>(ctx, copy);
        }
        ctx.deliver(ROOT, num(1));
    }
    fn dive(&self, _: u16, _: &[u64], _: &mut i64, _: &mut Wctx) -> DiveResult {
        unreachable!()
    }
}

#[test]
fn borrowed_pairs_match_separate_reads_without_consuming_shared_chains() {
    for threads in [1, 4] {
        assert_eq!(
            Engine::with_capacity(threads, 1, 4096, 4096).run(&Cells, Redex::default()),
            num(1)
        );
    }
}
