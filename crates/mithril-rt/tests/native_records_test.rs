use mithril_rt::prelude::*;

fn check<const K: usize>() {
    let shape=[0;K];
    let value=|depth:u64| std::array::from_fn(|i| depth.wrapping_mul(0x9e3779b97f4a7c15).rotate_left(i as u32)^u64::MAX.wrapping_sub(i as u64));
    for depth in [1,2,17,257] {
        let mut stack=native_records(shape);
        assert!(native_record_empty(&stack));
        for n in 0..depth {native_record_push(&mut stack,value(n));}
        for n in (0..depth).rev() {
            assert_eq!(native_record_read(&stack,shape),value(n));
            assert_eq!(native_record_read(&stack,shape),value(n),"reading retains the record");
            native_record_replace(&mut stack,value(n+1000));
            assert_eq!(native_record_read(&stack,shape),value(n+1000));
            let mut inner=native_records(shape);
            native_record_push(&mut inner,value(5000));
            assert_eq!(native_record_read(&inner,shape),value(5000));
            native_record_pop(&mut inner,shape);native_record_done(&mut inner);
            assert_eq!(native_record_read(&stack,shape),value(n+1000),"nested stack retains outer record");
            native_record_pop(&mut stack,shape);
        }
        assert!(native_record_empty(&stack));native_record_done(&mut stack);
    }
}
#[test]
fn records_preserve_all_bits_and_lifetimes_for_arbitrary_widths() {
    check::<0>();check::<1>();check::<2>();check::<3>();check::<7>();check::<17>();
}
#[test]
fn empty_record_operations_fail_without_inventing_values() {
    assert!(std::panic::catch_unwind(||native_record_read(&native_records([0;3]),[0;3])).is_err());
    assert!(std::panic::catch_unwind(||native_record_pop(&mut native_records([0;3]),[0;3])).is_err());
    assert!(std::panic::catch_unwind(||native_record_replace(&mut native_records([0;3]),[u64::MAX;3])).is_err());
}
