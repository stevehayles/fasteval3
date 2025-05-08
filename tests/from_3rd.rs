use fasteval3::{
    eval_compiled_ref, CachedCallbackNamespace, Compiler, EmptyNamespace, Error, Evaler, Parser,
    Slab,
};

use std::str::from_utf8;

#[allow(clippy::needless_pass_by_value)] // This type is explicitly required by our namespace.
fn evalns_cb(name: &str, args: Vec<f32>) -> Option<f32> {
    match name {
        "w" => Some(0.0),
        "x" => Some(1.0),
        "y" => Some(2.0),
        "y7" => Some(2.7),
        "z" => Some(3.0),
        "foo" => Some(args[0] * 10.0),
        "bar" => Some(args[0] + args[1]),
        _ => None,
    }
}

fn chk_ok(expr_str: &str, expect_compile_str: &str, expect_slab_str: &str, expect_eval: f32) {
    let mut slab = Slab::new();
    let expr = Parser::new()
        .parse(expr_str, &mut slab.ps)
        .unwrap()
        .from(&slab.ps);
    let instr = expr.compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);

    assert_eq!(format!("{instr:?}"), expect_compile_str);
    assert_eq!(format!("{slab:?}"), expect_slab_str);

    (|| -> Result<(), Error> {
        let mut ns = CachedCallbackNamespace::new(evalns_cb);
        assert!((eval_compiled_ref!(&instr, &slab, &mut ns) - expect_eval).abs() < f32::EPSILON);

        // Make sure Instruction eval matches normal eval:
        assert!(
            (eval_compiled_ref!(&instr, &slab, &mut ns) - expr.eval(&slab, &mut ns).unwrap()).abs()
                < f32::EPSILON
        );

        Ok(())
    })()
    .unwrap();
}

fn chk_perr(expr_str: &str, expect_err: Error) {
    let mut slab = Slab::new();
    let res = Parser::new().parse(expr_str, &mut slab.ps);
    assert_eq!(res, Err(expect_err));
}

fn chk_eerr(expr_str: &str, expect_err: Error) {
    let mut slab = Slab::new();
    let expr = Parser::new()
        .parse(expr_str, &mut slab.ps)
        .unwrap()
        .from(&slab.ps);
    let instr = expr.compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);
    let mut ns = CachedCallbackNamespace::new(evalns_cb);
    assert_eq!(instr.eval(&slab, &mut ns), Err(expect_err));
}

#[test]
fn meval() {
    chk_perr("", Error::EofWhileParsing(String::from("value")));
    chk_perr("(", Error::EofWhileParsing(String::from("value")));
    chk_perr("0(", Error::UnparsedTokensRemaining(String::from("(")));
    chk_eerr("e", Error::Undefined(String::from("e")));
    chk_perr("1E", Error::ParseF32(String::from("1E")));
    chk_perr("1e+", Error::ParseF32(String::from("1e+")));
    chk_perr("()", Error::InvalidValue);
    chk_perr("2)", Error::UnparsedTokensRemaining(String::from(")")));
    chk_perr("2^", Error::EofWhileParsing(String::from("value")));
    chk_perr("(((2)", Error::EofWhileParsing(String::from("parentheses")));
    chk_perr("f(2,)", Error::InvalidValue);
    chk_perr("f(,2)", Error::InvalidValue);

    chk_ok("round(sin (pi()) * cos(0))",
"IConst(-0.0)",
"Slab{ exprs:{ 0:Expression { first: EStdFunc(EFuncPi), pairs: [] }, 1:Expression { first: EConstant(0.0), pairs: [] }, 2:Expression { first: EStdFunc(EFuncSin(ExpressionI(0))), pairs: [ExprPair(EMul, EStdFunc(EFuncCos(ExpressionI(1))))] }, 3:Expression { first: EStdFunc(EFuncRound { modulus: None, expr: ExpressionI(2) }), pairs: [] } }, vals:{}, instrs:{} }",
0.0);

    chk_ok("max(1.)",
"IConst(1.0)",
"Slab{ exprs:{ 0:Expression { first: EConstant(1.0), pairs: [] }, 1:Expression { first: EStdFunc(EFuncMax { first: ExpressionI(0), rest: [] }), pairs: [] } }, vals:{}, instrs:{} }",
1.0);

    chk_ok("max(1., 2., -1)",
"IConst(2.0)",
"Slab{ exprs:{ 0:Expression { first: EConstant(1.0), pairs: [] }, 1:Expression { first: EConstant(2.0), pairs: [] }, 2:Expression { first: EConstant(-1.0), pairs: [] }, 3:Expression { first: EStdFunc(EFuncMax { first: ExpressionI(0), rest: [ExpressionI(1), ExpressionI(2)] }), pairs: [] } }, vals:{}, instrs:{} }",
2.0);

    chk_ok("sin(1.) + cos(2.)",
"IConst(0.4253241)",
"Slab{ exprs:{ 0:Expression { first: EConstant(1.0), pairs: [] }, 1:Expression { first: EConstant(2.0), pairs: [] }, 2:Expression { first: EStdFunc(EFuncSin(ExpressionI(0))), pairs: [ExprPair(EAdd, EStdFunc(EFuncCos(ExpressionI(1))))] } }, vals:{}, instrs:{} }",
(1f32).sin() + (2f32).cos());
}

#[test]
fn overflow_stack() {
    chk_perr(
        from_utf8(&[b'('; 1]).unwrap(),
        Error::EofWhileParsing(String::from("value")),
    );
    chk_perr(
        from_utf8(&[b'('; 2]).unwrap(),
        Error::EofWhileParsing(String::from("value")),
    );
    chk_perr(
        from_utf8(&[b'('; 4]).unwrap(),
        Error::EofWhileParsing(String::from("value")),
    );
    chk_perr(
        from_utf8(&[b'('; 8]).unwrap(),
        Error::EofWhileParsing(String::from("value")),
    );
    chk_perr(
        from_utf8(&[b'('; 16]).unwrap(),
        Error::EofWhileParsing(String::from("value")),
    );
    chk_perr(
        from_utf8(&[b'('; 32]).unwrap(),
        Error::EofWhileParsing(String::from("value")),
    );
    chk_perr(from_utf8(&[b'('; 33]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 64]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 128]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 256]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 512]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 1024]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 2048]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 4096]).unwrap(), Error::TooDeep);
    chk_perr(from_utf8(&[b'('; 8192]).unwrap(), Error::TooLong);

    // Test custom safety parse limits:
    assert_eq!(
        Parser {
            expr_len_limit: fasteval3::parser::DEFAULT_EXPR_LEN_LIMIT,
            expr_depth_limit: 31
        }
        .parse(from_utf8(&[b'('; 32]).unwrap(), &mut Slab::new().ps),
        Err(Error::TooDeep)
    );

    assert_eq!(
        Parser {
            expr_len_limit: 8,
            expr_depth_limit: fasteval3::parser::DEFAULT_EXPR_DEPTH_LIMIT
        }
        .parse(from_utf8(&[b'('; 32]).unwrap(), &mut Slab::new().ps),
        Err(Error::TooLong)
    );
}
