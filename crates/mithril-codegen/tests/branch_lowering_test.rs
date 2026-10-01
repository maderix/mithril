//! Lowering preserves runtime branch geometry; backend printers share that IR.
use mithril_front::{core::eval_core, desugar, parse};

#[test]
fn lowering_keeps_branch_backedges_in_the_residual_program() {
    for update in [
        "if (n & 1) == 0:\n        return loop(n - 1, a + n)\n    return loop(n - 1, a - n)",
        "if (n & 1) == 0:\n        return loop(n - 1, a + n * 3)\n    elif (n & 3) == 1:\n        return loop(n - 1, a ^ n)\n    return loop(n - 1, a - n)",
        "if n == 1:\n        return loop(n - 1, a)\n    return loop(n - 1, a + 56 // (n - 1))",
    ] {
        let src = format!("def loop(n, a):\n    if n == 0:\n        return a\n    {update}\n\ndef main():\n    return loop(17, 3)\n");
        let module = desugar(&parse(&src).unwrap()).unwrap();
        let (_, lowered) = mithril_codegen::lower(&module);
        for (before, after) in module.fns.iter().zip(&lowered.fns) {
            assert_eq!(before.body, after.body, "{}: lowering speculated runtime branches", before.name);
        }
        assert_eq!(
            mithril_codegen::fmt_val(&eval_core(&module, module.main, &[])),
            mithril_codegen::fmt_val(&eval_core(&lowered, lowered.main, &[]))
        );
    }
}
