#[cfg(feature = "eval-builtin")]
use fasteval3::compiler::Instruction::IEvalFunc;
use fasteval3::compiler::Instruction::{
    self, IAdd, IConst, IExp, IFuncACos, IFuncACosH, IFuncASin, IFuncASinH, IFuncATan, IFuncATanH,
    IFuncAbs, IFuncCeil, IFuncCos, IFuncCosH, IFuncFloor, IFuncInt, IFuncLog, IFuncMax, IFuncMin,
    IFuncRound, IFuncSign, IFuncSin, IFuncSinH, IFuncTan, IFuncTanH, IInv, IMod, IMul, INeg, INot,
    IPrintFunc, IVar, IAND, IEQ, IGT, IGTE, ILT, ILTE, INE, IOR,
};
use fasteval3::compiler::IC;
#[cfg(feature = "eval-builtin")]
use fasteval3::parser::{EvalFunc, KWArg};
use fasteval3::parser::{
    ExpressionOrString::{EExpr, EStr},
    PrintFunc,
};
use fasteval3::{
    eval_compiled, eval_compiled_ref, CachedCallbackNamespace, Compiler, EmptyNamespace, Error,
    Evaler, ExpressionI, InstructionI, Parser, Slab,
};

pub(crate) mod common;

use common::assert_error_margin;

#[test]
fn slab_overflow() {
    let mut slab = Slab::with_capacity(2);
    assert_eq!(
        Parser::new().parse("1 + 2 + -3 + ( +4 )", &mut slab.ps),
        Ok(ExpressionI(1))
    );
    assert_eq!(format!("{slab:?}"),
"Slab{ exprs:{ 0:Expression { first: EConstant(4.0), pairs: [] }, 1:Expression { first: EConstant(1.0), pairs: [ExprPair(EAdd, EConstant(2.0)), ExprPair(EAdd, EConstant(-3.0)), ExprPair(EAdd, EUnaryOp(EParentheses(ExpressionI(0))))] } }, vals:{}, instrs:{} }");

    assert_eq!(
        Parser::new().parse("1 + 2 + -3 + ( ++4 )", &mut slab.ps),
        Ok(ExpressionI(1))
    );
    assert_eq!(format!("{slab:?}"),
"Slab{ exprs:{ 0:Expression { first: EUnaryOp(EPos(ValueI(0))), pairs: [] }, 1:Expression { first: EConstant(1.0), pairs: [ExprPair(EAdd, EConstant(2.0)), ExprPair(EAdd, EConstant(-3.0)), ExprPair(EAdd, EUnaryOp(EParentheses(ExpressionI(0))))] } }, vals:{ 0:EConstant(4.0) }, instrs:{} }");

    assert_eq!(
        Parser::new().parse("1 + 2 + -3 + ( +++4 )", &mut slab.ps),
        Ok(ExpressionI(1))
    );
    assert_eq!(format!("{slab:?}"),
"Slab{ exprs:{ 0:Expression { first: EUnaryOp(EPos(ValueI(1))), pairs: [] }, 1:Expression { first: EConstant(1.0), pairs: [ExprPair(EAdd, EConstant(2.0)), ExprPair(EAdd, EConstant(-3.0)), ExprPair(EAdd, EUnaryOp(EParentheses(ExpressionI(0))))] } }, vals:{ 0:EConstant(4.0), 1:EUnaryOp(EPos(ValueI(0))) }, instrs:{} }");

    assert_eq!(
        Parser::new().parse("1 + 2 + -3 + ( ++++4 )", &mut slab.ps),
        Err(Error::SlabOverflow)
    );
}

#[test]
fn basics() {
    let mut slab = Slab::new();
    let mut ns = EmptyNamespace;

    let expr_i = Parser::new().parse("3*3-3/3+1", &mut slab.ps).unwrap();
    let expr_ref = slab.ps.get_expr(expr_i);
    let instr = expr_ref.compile(&slab.ps, &mut slab.cs, &mut ns);
    assert_eq!(instr, IConst(9.0));
    assert_eq!(format!("{slab:?}"),
"Slab{ exprs:{ 0:Expression { first: EConstant(3.0), pairs: [ExprPair(EMul, EConstant(3.0)), ExprPair(ESub, EConstant(3.0)), ExprPair(EDiv, EConstant(3.0)), ExprPair(EAdd, EConstant(1.0))] } }, vals:{}, instrs:{} }");

    (|| -> Result<(), Error> {
        assert_error_margin(eval_compiled_ref!(&instr, &slab, &mut ns), 9.0);
        assert_error_margin(eval_compiled_ref!(&instr, &slab, &mut ns), 9.0);
        Ok(())
    })()
    .unwrap();
}

fn comp(expr_str: &str) -> (Slab, Instruction) {
    let mut slab = Slab::new();
    let instr = Parser::new()
        .parse(expr_str, &mut slab.ps)
        .unwrap()
        .from(&slab.ps)
        .compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);
    (slab, instr)
}

#[allow(clippy::needless_pass_by_value)] // The amount of work it would take to fix this... Is immeasurable.
fn comp_chk(expr_str: &str, expect_instr: Instruction, expect_fmt: &str, expect_eval: f32) {
    let mut slab = Slab::new();

    let mut ns = CachedCallbackNamespace::new(|name, args| match name {
        "w" => Some(0.0),
        "x" => Some(1.0),
        "y" => Some(2.0),
        "y7" => Some(2.7),
        "z" => Some(3.0),
        "foo" => Some(args[0] * 10.0),
        "bar" => Some(args[0] + args[1]),
        _ => None,
    });

    let expr = Parser::new()
        .parse(expr_str, &mut slab.ps)
        .unwrap()
        .from(&slab.ps);
    let instr = expr.compile(&slab.ps, &mut slab.cs, &mut ns);

    assert_eq!(instr, expect_instr);
    assert_eq!(format!("{:?}", slab.cs), expect_fmt);

    (|| -> Result<(), Error> {
        assert_error_margin(eval_compiled_ref!(&instr, &slab, &mut ns), expect_eval);

        // Make sure Instruction eval matches normal eval:
        assert_error_margin(
            eval_compiled_ref!(&instr, &slab, &mut ns),
            expr.eval(&slab, &mut ns).unwrap(),
        );

        Ok(())
    })()
    .unwrap();
}
#[cfg(feature = "unsafe-vars")]
fn unsafe_comp_chk(expr_str: &str, expect_fmt: &str, expect_eval: f32) {
    fn replace_addrs(mut s: String) -> String {
        let mut start = 0;
        loop {
            match s[start..].find(" 0x") {
                None => break,
                Some(i) => {
                    let v = unsafe { s.as_mut_vec() };

                    start = start + i + 3;
                    loop {
                        match v.get(start) {
                            None => break,
                            Some(&b) => {
                                if (b'0' <= b && b <= b'9') || (b'a' <= b && b <= b'f') {
                                    v[start] = b'?';
                                    start += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                }
            };
        }
        s
    }

    let mut slab = Slab::new();
    let w = 0.0;
    let x = 1.0;
    let y = 2.0;
    let y7 = 2.7;
    let z = 3.0;
    unsafe {
        slab.ps.add_unsafe_var("w".to_string(), &w);
        slab.ps.add_unsafe_var("x".to_string(), &x);
        slab.ps.add_unsafe_var("y".to_string(), &y);
        slab.ps.add_unsafe_var("y7".to_string(), &y7);
        slab.ps.add_unsafe_var("z".to_string(), &z);
    }

    let expr = Parser::new()
        .parse(expr_str, &mut slab.ps)
        .unwrap()
        .from(&slab.ps);
    let instr = expr.compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);

    assert_eq!(replace_addrs(format!("{:?}", slab.cs)), expect_fmt);

    (|| -> Result<(), Error> {
        let mut ns = EmptyNamespace;
        assert_eq!(eval_compiled_ref!(&instr, &slab, &mut ns), expect_eval);

        // Make sure Instruction eval matches normal eval:
        assert_eq!(
            eval_compiled_ref!(&instr, &slab, &mut ns),
            expr.eval(&slab, &mut ns).unwrap()
        );

        Ok(())
    })()
    .unwrap();
}

fn comp_chk_str(expr_str: &str, expect_instr: &str, expect_fmt: &str, expect_eval: f32) {
    let mut slab = Slab::new();
    let expr = Parser::new()
        .parse(expr_str, &mut slab.ps)
        .unwrap()
        .from(&slab.ps);
    let instr = expr.compile(&slab.ps, &mut slab.cs, &mut EmptyNamespace);

    assert_eq!(format!("{instr:?}"), expect_instr);
    assert_eq!(format!("{:?}", slab.cs), expect_fmt);

    let mut ns = CachedCallbackNamespace::new(|name, args| match name {
        "w" => Some(0.0),
        "x" => Some(1.0),
        "y" => Some(2.0),
        "y7" => Some(2.7),
        "z" => Some(3.0),
        "foo" => Some(args[0] * 10.0),
        "bar" => Some(args[0] + args[1]),
        _ => None,
    });

    (|| -> Result<(), Error> {
        if expect_eval.is_nan() {
            assert!(eval_compiled_ref!(&instr, &slab, &mut ns).is_nan());

            assert!(eval_compiled_ref!(&instr, &slab, &mut ns).is_nan());
            assert!(expr.eval(&slab, &mut ns).unwrap().is_nan());
        } else {
            // These two checks do not pass the (x - y).abs() < f32::EPSILON evaluation.
            // There's some imprecision here.
            // TODO: Fix imprecision.
            assert_eq!(eval_compiled_ref!(&instr, &slab, &mut ns), expect_eval);

            // Make sure Instruction eval matches normal eval:
            assert_eq!(
                eval_compiled_ref!(&instr, &slab, &mut ns),
                expr.eval(&slab, &mut ns).unwrap()
            );
        }

        Ok(())
    })()
    .unwrap();
}

#[test]
fn double_neg() {
    assert_eq!(comp("1+1.5").1, IConst(2.5));
    assert_eq!(comp("-1.5").1, IConst(-1.5));
    assert_eq!(comp("--1.5").1, IConst(1.5));
    assert_eq!(comp("1 + -1.5").1, IConst(-0.5));
    assert_eq!(comp("1 + --1.5").1, IConst(2.5));
    assert_eq!(comp("1 + ----1.5").1, IConst(2.5));
    assert_eq!(comp("1 - ----1.5").1, IConst(-0.5));

    assert_eq!(comp("x").1, IVar(String::from("x")));

    comp_chk("1-1", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "1 + x",
        IAdd(InstructionI(0), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk(
        "x + 1",
        IAdd(InstructionI(0), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk(
        "0.5 + x + 0.5",
        IAdd(InstructionI(0), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk(
        "0.5 - x - 0.5",
        INeg(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        -1.0,
    );
    comp_chk(
        "0.5 - -x - 0.5",
        IVar(String::from("x")),
        "CompileSlab{ instrs:{} }",
        1.0,
    );
    comp_chk(
        "0.5 - --x - 1.5",
        IAdd(InstructionI(1), IC::C(-1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:INeg(InstructionI(0)) } }",
        -2.0,
    );
    comp_chk(
        "0.5 - ---x - 1.5",
        IAdd(InstructionI(0), IC::C(-1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );
    comp_chk(
        "0.5 - (---x) - 1.5",
        IAdd(InstructionI(0), IC::C(-1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );
    comp_chk(
        "0.5 - -(--x) - 1.5",
        IAdd(InstructionI(0), IC::C(-1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );
    comp_chk(
        "0.5 - --(-x) - 1.5",
        IAdd(InstructionI(0), IC::C(-1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );
    comp_chk("0.5 - --(-x - 1.5)", IAdd(InstructionI(3), IC::C(0.5)), "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:INeg(InstructionI(0)), 2:IAdd(InstructionI(1), C(-1.5)), 3:INeg(InstructionI(2)) } }", 3.0);
    comp_chk("0.5 - --((((-(x)) - 1.5)))", IAdd(InstructionI(3), IC::C(0.5)), "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:INeg(InstructionI(0)), 2:IAdd(InstructionI(1), C(-1.5)), 3:INeg(InstructionI(2)) } }", 3.0);
    comp_chk("0.5 - -(-(--((((-(x)) - 1.5)))))", IAdd(InstructionI(3), IC::C(0.5)), "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:INeg(InstructionI(0)), 2:IAdd(InstructionI(1), C(-1.5)), 3:INeg(InstructionI(2)) } }", 3.0);
}

#[test]
fn all_instrs() {
    // IConst:
    comp_chk("1", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("-1", IConst(-1.0), "CompileSlab{ instrs:{} }", -1.0);

    // IVar:
    comp_chk(
        "x",
        IVar(String::from("x")),
        "CompileSlab{ instrs:{} }",
        1.0,
    );
    comp_chk("x()", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("x[]", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);

    // INeg:
    comp_chk(
        "-x",
        INeg(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        -1.0,
    );

    // INot:
    comp_chk(
        "!x",
        INot(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );

    // IInv:
    comp_chk(
        "1/x",
        IInv(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        1.0,
    );

    // IAdd:
    comp_chk(
        "1 + x",
        IAdd(InstructionI(0), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk(
        "1 - x",
        IAdd(InstructionI(1), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:INeg(InstructionI(0)) } }",
        0.0,
    );
    comp_chk(
        "x + 2+pi()-360",
        IAdd(InstructionI(0), IC::C(-354.858_4)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        -353.858_4,
    );
    comp_chk(
        "x-360 + 2+pi()",
        IAdd(InstructionI(0), IC::C(-354.858_4)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        -353.858_4,
    );
    comp_chk(
        "1 - -(x-360 + 2+pi())",
        IAdd(InstructionI(1), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:IAdd(InstructionI(0), C(-354.8584)) } }",
        -352.858_4,
    );
    comp_chk(
        "3 + 3 - 3 + 3 - 3 + 3",
        IConst(6.0),
        "CompileSlab{ instrs:{} }",
        6.0,
    );
    comp_chk(
        "3 + x - 3 + 3 + y - 3",
        IAdd(InstructionI(0), IC::I(InstructionI(1))),
        "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:IVar(\"y\") } }",
        3.0,
    );

    // IMul:
    comp_chk(
        "2 * x",
        IMul(InstructionI(0), IC::C(2.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk(
        "x * 2",
        IMul(InstructionI(0), IC::C(2.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk(
        "x / 2",
        IMul(InstructionI(0), IC::C(0.5)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.5,
    );
    comp_chk(
        "x * 2*pi()/360",
        IMul(InstructionI(0), IC::C(0.017_453_294)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.017_453_294,
    );
    comp_chk(
        "x/360 * 2*pi()",
        IMul(InstructionI(0), IC::C(0.017_453_294)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.017_453_294,
    );
    comp_chk("1 / -(x/360 * 2*pi())", IInv(InstructionI(2)), "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:IMul(InstructionI(0), C(0.017453294)), 2:INeg(InstructionI(1)) } }", -57.295_773);
    comp_chk(
        "3 * 3 / 3 * 3 / 3 * 3",
        IConst(9.0),
        "CompileSlab{ instrs:{} }",
        9.0,
    );
    comp_chk(
        "3 * x / 3 * 3 * y / 3",
        IMul(InstructionI(0), IC::I(InstructionI(1))),
        "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:IVar(\"y\") } }",
        2.0,
    );

    // IMod:
    comp_chk("8 % 3", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk(
        "8 % z",
        IMod {
            dividend: IC::C(8.0),
            divisor: IC::I(InstructionI(0)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        2.0,
    );
    comp_chk("-8 % 3", IConst(-2.0), "CompileSlab{ instrs:{} }", -2.0);
    comp_chk("8 % -3", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk(
        "-8 % z",
        IMod {
            dividend: IC::C(-8.0),
            divisor: IC::I(InstructionI(0)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        -2.0,
    );
    comp_chk(
        "8 % -z",
        IMod {
            dividend: IC::C(8.0),
            divisor: IC::I(InstructionI(1)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"z\"), 1:INeg(InstructionI(0)) } }",
        2.0,
    );
    comp_chk("8 % 3 % 2", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk("8 % z % 2", IMod { dividend: IC::I(InstructionI(1)), divisor: IC::C(2.0) }, "CompileSlab{ instrs:{ 0:IVar(\"z\"), 1:IMod { dividend: C(8.0), divisor: I(InstructionI(0)) } } }", 0.0);

    // IExp:
    comp_chk("2 ^ 3", IConst(8.0), "CompileSlab{ instrs:{} }", 8.0);
    comp_chk(
        "2 ^ z",
        IExp {
            base: IC::C(2.0),
            power: IC::I(InstructionI(0)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        8.0,
    );
    comp_chk("4 ^ 0.5", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk(
        "2 ^ 0.5",
        IConst(std::f32::consts::SQRT_2), // 1.4142135623730951
        "CompileSlab{ instrs:{} }",
        std::f32::consts::SQRT_2,
    );
    comp_chk_str(
        "-4 ^ 0.5",
        "IConst(NaN)",
        "CompileSlab{ instrs:{} }",
        f32::NAN,
    );
    comp_chk(
        "y ^ 0.5",
        IExp {
            base: IC::I(InstructionI(0)),
            power: IC::C(0.5),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"y\") } }",
        std::f32::consts::SQRT_2,
    );
    comp_chk(
        "2 ^ 3 ^ 2",
        IConst(512.0),
        "CompileSlab{ instrs:{} }",
        512.0,
    );
    comp_chk("2 ^ z ^ 2", IExp { base: IC::C(2.0), power: IC::I(InstructionI(1)) }, "CompileSlab{ instrs:{ 0:IVar(\"z\"), 1:IExp { base: I(InstructionI(0)), power: C(2.0) } } }", 512.0);
    comp_chk("2 ^ z ^ 1 ^ 2 ^ 1", IExp { base: IC::C(2.0), power: IC::I(InstructionI(1)) }, "CompileSlab{ instrs:{ 0:IVar(\"z\"), 1:IExp { base: I(InstructionI(0)), power: C(1.0) } } }", 8.0);

    // ILT:
    comp_chk("2 < 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "2 < z",
        ILT(IC::C(2.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("3 < 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "3 < z",
        ILT(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk("1 < 2 < 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);

    // ILTE:
    comp_chk("2 <= 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "2 <= z",
        ILTE(IC::C(2.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("3 <= 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "3 <= z",
        ILTE(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("4 <= 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "4 <= z",
        ILTE(IC::C(4.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );

    // IEQ:
    comp_chk("2 == 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "2 == z",
        IEQ(IC::C(2.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk("3 == 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "3 == z",
        IEQ(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("4 == 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "4 == z",
        IEQ(IC::C(4.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "4 == z == 1.0",
        IEQ(IC::I(InstructionI(1)), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"z\"), 1:IEQ(C(4.0), I(InstructionI(0))) } }",
        0.0,
    );
    comp_chk(
        "3.1 == z",
        IEQ(IC::C(3.1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.01 == z",
        IEQ(IC::C(3.01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.001 == z",
        IEQ(IC::C(3.001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.0001 == z",
        IEQ(IC::C(3.0001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.00001 == z",
        IEQ(IC::C(3.00001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.000001 == z",
        IEQ(IC::C(3.000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.0000001 == z",
        IEQ(IC::C(3.000_000_1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.00000001 == z",
        IEQ(IC::C(3.000_000_01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.000000001 == z",
        IEQ(IC::C(3.000_000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.0000000001 == z",
        IEQ(IC::C(3.000_000_000_1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.00000000001 == z",
        IEQ(IC::C(3.000_000_000_01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.000000000001 == z",
        IEQ(IC::C(3.000_000_000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.0000000000001 == z",
        IEQ(IC::C(3.000_000_000_000_1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.00000000000001 == z",
        IEQ(IC::C(3.000_000_000_000_01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.000000000000001 == z",
        IEQ(IC::C(3.000_000_000_000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.0000000000000001 == z",
        IEQ(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );

    // INE:
    comp_chk("2 != 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "2 != z",
        INE(IC::C(2.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("3 != 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "3 != z",
        INE(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk("4 != 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "4 != z",
        INE(IC::C(4.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.1 != z",
        INE(IC::C(3.1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.01 != z",
        INE(IC::C(3.01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.001 != z",
        INE(IC::C(3.001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.0001 != z",
        INE(IC::C(3.0001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.00001 != z",
        INE(IC::C(3.00001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk(
        "3.000001 != z",
        INE(IC::C(3.000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.0000001 != z",
        INE(IC::C(3.000_000_1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.00000001 != z",
        INE(IC::C(3.000_000_01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.000000001 != z",
        INE(IC::C(3.000_000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.0000000001 != z",
        INE(IC::C(3.000_000_000_1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.00000000001 != z",
        INE(IC::C(3.000_000_000_01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.000000000001 != z",
        INE(IC::C(3.000_000_000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.0000000000001 != z",
        INE(IC::C(3.000_000_000_000_1), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.00000000000001 != z",
        INE(IC::C(3.000_000_000_000_01), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.000000000000001 != z",
        INE(IC::C(3.000_000_000_000_001), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk(
        "3.0000000000000001 != z",
        INE(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );

    // IGTE:
    comp_chk("2 >= 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "2 >= z",
        IGTE(IC::C(2.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk("3 >= 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "3 >= z",
        IGTE(IC::C(3.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("4 >= 3", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "4 >= z",
        IGTE(IC::C(4.0), IC::I(InstructionI(0))),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );

    // IGT:
    comp_chk("3 > 2", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "z > 2",
        IGT(IC::I(InstructionI(0)), IC::C(2.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        1.0,
    );
    comp_chk("3 > 3", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "z > 3",
        IGT(IC::I(InstructionI(0)), IC::C(3.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"z\") } }",
        0.0,
    );
    comp_chk("3 > 2 > 1", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);

    // IAND:
    comp_chk("2 and 3", IConst(3.0), "CompileSlab{ instrs:{} }", 3.0);
    comp_chk("2 && 3", IConst(3.0), "CompileSlab{ instrs:{} }", 3.0);
    comp_chk(
        "2 and 3 and 4",
        IConst(4.0),
        "CompileSlab{ instrs:{} }",
        4.0,
    );
    comp_chk("2 && 3 && 4", IConst(4.0), "CompileSlab{ instrs:{} }", 4.0);
    comp_chk(
        "0 and 1 and 2",
        IConst(0.0),
        "CompileSlab{ instrs:{} }",
        0.0,
    );
    comp_chk("0 && 1 && 2", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "1 and 0 and 2",
        IConst(0.0),
        "CompileSlab{ instrs:{} }",
        0.0,
    );
    comp_chk("1 && 0 && 2", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "1 and 2 and 0",
        IConst(0.0),
        "CompileSlab{ instrs:{} }",
        0.0,
    );
    comp_chk("1 && 2 && 0", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "x and 2",
        IAND(InstructionI(0), IC::C(2.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );
    comp_chk("0 and x", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "w and x",
        IAND(InstructionI(0), IC::I(InstructionI(1))),
        "CompileSlab{ instrs:{ 0:IVar(\"w\"), 1:IVar(\"x\") } }",
        0.0,
    );

    // IOR:
    comp_chk("2 or 3", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk("2 || 3", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk("2 or 3 or 4", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk("2 || 3 || 4", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk("0 or 1 or 2", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("0 || 1 || 2", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("1 or 0 or 2", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("1 || 0 || 2", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("1 or 2 or 0", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("1 || 2 || 0", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "x or 2",
        IOR(InstructionI(0), IC::C(2.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        1.0,
    );
    comp_chk(
        "0 or x",
        IVar(String::from("x")),
        "CompileSlab{ instrs:{} }",
        1.0,
    );
    comp_chk(
        "w or x",
        IOR(InstructionI(0), IC::I(InstructionI(1))),
        "CompileSlab{ instrs:{ 0:IVar(\"w\"), 1:IVar(\"x\") } }",
        1.0,
    );
    comp_chk(
        "x or w",
        IOR(InstructionI(0), IC::I(InstructionI(1))),
        "CompileSlab{ instrs:{ 0:IVar(\"x\"), 1:IVar(\"w\") } }",
        1.0,
    );

    // IVar
    comp_chk(
        "x",
        IVar(String::from("x")),
        "CompileSlab{ instrs:{} }",
        1.0,
    );
    {
        let (_s, i) = comp("int");
        assert_eq!(i, IVar(String::from("int")));

        let (_s, i) = comp("print");
        assert_eq!(i, IVar(String::from("print")));

        let (_s, i) = comp("eval");
        assert_eq!(i, IVar(String::from("eval")));
    }

    // IUnsafeVar
    #[cfg(feature = "unsafe-vars")]
    {
        unsafe_comp_chk("x", "CompileSlab{ instrs:{} }", 1.0);
        unsafe_comp_chk("x + y", "CompileSlab{ instrs:{ 0:IUnsafeVar { name: \"x\", ptr: 0x???????????? }, 1:IUnsafeVar { name: \"y\", ptr: 0x???????????? } } }", 3.0);
        unsafe_comp_chk("x() + y", "CompileSlab{ instrs:{ 0:IUnsafeVar { name: \"x\", ptr: 0x???????????? }, 1:IUnsafeVar { name: \"y\", ptr: 0x???????????? } } }", 3.0);
        unsafe_comp_chk("x(x,y,z) + y", "CompileSlab{ instrs:{ 0:IUnsafeVar { name: \"x\", ptr: 0x???????????? }, 1:IUnsafeVar { name: \"y\", ptr: 0x???????????? } } }", 3.0);
    }

    // IFunc
    comp_chk("foo(2.7)", IConst(27.0), "CompileSlab{ instrs:{} }", 27.0);
    comp_chk(
        "foo(2.7, 3.4)",
        IConst(27.0),
        "CompileSlab{ instrs:{} }",
        27.0,
    );

    // IFuncInt
    comp_chk("int(2.7)", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk(
        "int(y7)",
        IFuncInt(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.0,
    );
    comp_chk("int(-2.7)", IConst(-2.0), "CompileSlab{ instrs:{} }", -2.0);
    comp_chk(
        "int(-y7)",
        IFuncInt(InstructionI(1)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\"), 1:INeg(InstructionI(0)) } }",
        -2.0,
    );

    // IFuncCeil
    comp_chk("ceil(2.7)", IConst(3.0), "CompileSlab{ instrs:{} }", 3.0);
    comp_chk(
        "ceil(y7)",
        IFuncCeil(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        3.0,
    );
    comp_chk("ceil(-2.7)", IConst(-2.0), "CompileSlab{ instrs:{} }", -2.0);
    comp_chk(
        "ceil(-y7)",
        IFuncCeil(InstructionI(1)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\"), 1:INeg(InstructionI(0)) } }",
        -2.0,
    );

    // IFuncFloor
    comp_chk("floor(2.7)", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);
    comp_chk(
        "floor(y7)",
        IFuncFloor(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.0,
    );
    comp_chk(
        "floor(-2.7)",
        IConst(-3.0),
        "CompileSlab{ instrs:{} }",
        -3.0,
    );
    comp_chk(
        "floor(-y7)",
        IFuncFloor(InstructionI(1)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\"), 1:INeg(InstructionI(0)) } }",
        -3.0,
    );

    // IFuncAbs
    comp_chk("abs(2.7)", IConst(2.7), "CompileSlab{ instrs:{} }", 2.7);
    comp_chk(
        "abs(y7)",
        IFuncAbs(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.7,
    );
    comp_chk("abs(-2.7)", IConst(2.7), "CompileSlab{ instrs:{} }", 2.7);
    comp_chk(
        "abs(-y7)",
        IFuncAbs(InstructionI(1)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\"), 1:INeg(InstructionI(0)) } }",
        2.7,
    );

    // IFuncSign
    comp_chk("sign(2.7)", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "sign(y7)",
        IFuncSign(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        1.0,
    );
    comp_chk("sign(-2.7)", IConst(-1.0), "CompileSlab{ instrs:{} }", -1.0);
    comp_chk(
        "sign(-y7)",
        IFuncSign(InstructionI(1)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\"), 1:INeg(InstructionI(0)) } }",
        -1.0,
    );

    // IFuncLog
    comp_chk("log(1)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk("log(10)", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "log(2, 10)",
        IConst(std::f32::consts::LOG2_10), // 3.321928094887362
        "CompileSlab{ instrs:{} }",
        std::f32::consts::LOG2_10,
    );
    comp_chk(
        "log(e(), 10)",
        IConst(std::f32::consts::LN_10 + 0.0000003), //fix for rounding erros in f32 // 2.302585092994046
        "CompileSlab{ instrs:{} }",
        std::f32::consts::LN_10 + 0.0000003,
    );
    comp_chk(
        "log(x)",
        IFuncLog {
            base: IC::C(10.0),
            of: IC::I(InstructionI(0)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );
    comp_chk(
        "log(y,x)",
        IFuncLog {
            base: IC::I(InstructionI(0)),
            of: IC::I(InstructionI(1)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"y\"), 1:IVar(\"x\") } }",
        0.0,
    );

    // IFuncRound
    comp_chk("round(2.7)", IConst(3.0), "CompileSlab{ instrs:{} }", 3.0);
    comp_chk(
        "round(-2.7)",
        IConst(-3.0),
        "CompileSlab{ instrs:{} }",
        -3.0,
    );
    comp_chk(
        "round(y7)",
        IFuncRound {
            modulus: IC::C(1.0),
            of: IC::I(InstructionI(0)),
        },
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        3.0,
    );

    // IFuncMin
    comp_chk("min(2.7)", IConst(2.7), "CompileSlab{ instrs:{} }", 2.7);
    comp_chk(
        "min(2.7, 3.7)",
        IConst(2.7),
        "CompileSlab{ instrs:{} }",
        2.7,
    );
    comp_chk(
        "min(4.7, 3.7, 2.7)",
        IConst(2.7),
        "CompileSlab{ instrs:{} }",
        2.7,
    );
    comp_chk(
        "min(y7)",
        IVar(String::from("y7")),
        "CompileSlab{ instrs:{} }",
        2.7,
    );
    comp_chk(
        "min(4.7, y7, 3.7)",
        IFuncMin(InstructionI(0), IC::C(3.7)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.7,
    );
    comp_chk(
        "min(3.7, y7, 4.7)",
        IFuncMin(InstructionI(0), IC::C(3.7)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.7,
    );
    comp_chk_str(
        "min(NaN, y7, 4.7)",
        "IFuncMin(InstructionI(0), C(NaN))",
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        f32::NAN,
    );
    comp_chk_str(
        "min(NaN, 4.7)",
        "IConst(NaN)",
        "CompileSlab{ instrs:{} }",
        f32::NAN,
    );
    comp_chk_str(
        "min(inf, y7, 4.7)",
        "IFuncMin(InstructionI(0), C(4.7))",
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.7,
    );
    comp_chk_str(
        "min(inf, 4.7)",
        "IConst(4.7)",
        "CompileSlab{ instrs:{} }",
        4.7,
    );
    comp_chk_str(
        "min(-inf, y7, 4.7)",
        "IFuncMin(InstructionI(0), C(-inf))",
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        f32::NEG_INFINITY,
    );
    comp_chk_str(
        "min(-inf, 4.7)",
        "IConst(-inf)",
        "CompileSlab{ instrs:{} }",
        f32::NEG_INFINITY,
    );

    // IFuncMax
    comp_chk("max(2.7)", IConst(2.7), "CompileSlab{ instrs:{} }", 2.7);
    comp_chk(
        "max(2.7, 1.7)",
        IConst(2.7),
        "CompileSlab{ instrs:{} }",
        2.7,
    );
    comp_chk(
        "max(0.7, 1.7, 2.7)",
        IConst(2.7),
        "CompileSlab{ instrs:{} }",
        2.7,
    );
    comp_chk(
        "max(y7)",
        IVar(String::from("y7")),
        "CompileSlab{ instrs:{} }",
        2.7,
    );
    comp_chk(
        "max(0.7, y7, 1.7)",
        IFuncMax(InstructionI(0), IC::C(1.7)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.7,
    );
    comp_chk(
        "max(1.7, y7, 0.7)",
        IFuncMax(InstructionI(0), IC::C(1.7)),
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        2.7,
    );
    comp_chk_str(
        "max(NaN, y7, 0.7)",
        "IFuncMax(InstructionI(0), C(NaN))",
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        f32::NAN,
    );
    comp_chk_str(
        "max(NaN, 0.7)",
        "IConst(NaN)",
        "CompileSlab{ instrs:{} }",
        f32::NAN,
    );
    comp_chk_str(
        "max(inf, y7, 4.7)",
        "IFuncMax(InstructionI(0), C(inf))",
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        f32::INFINITY,
    );
    comp_chk_str(
        "max(inf, 4.7)",
        "IConst(inf)",
        "CompileSlab{ instrs:{} }",
        f32::INFINITY,
    );
    comp_chk_str(
        "max(-inf, y7, 4.7)",
        "IFuncMax(InstructionI(0), C(4.7))",
        "CompileSlab{ instrs:{ 0:IVar(\"y7\") } }",
        4.7,
    );
    comp_chk_str(
        "max(-inf, 4.7)",
        "IConst(4.7)",
        "CompileSlab{ instrs:{} }",
        4.7,
    );

    // IFuncSin
    comp_chk("sin(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "round(0.000001, sin(pi()))",
        IConst(0.0),
        "CompileSlab{ instrs:{} }",
        0.0,
    );
    comp_chk("sin(pi()/2)", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "sin(w)",
        IFuncSin(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );
    comp_chk("sin(pi()/y)", IFuncSin(InstructionI(2)), "CompileSlab{ instrs:{ 0:IVar(\"y\"), 1:IInv(InstructionI(0)), 2:IMul(InstructionI(1), C(3.1415927)) } }", 1.0);

    // IFuncCos
    comp_chk("cos(0)", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk("cos(pi())", IConst(-1.0), "CompileSlab{ instrs:{} }", -1.0);
    comp_chk(
        "round(0.000001, cos(pi()/2))",
        IConst(0.0),
        "CompileSlab{ instrs:{} }",
        0.0,
    );
    comp_chk(
        "cos(w)",
        IFuncCos(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        1.0,
    );
    comp_chk("round(0.000001, cos(pi()/y))", IFuncRound { modulus: IC::C(0.000_001,), of: IC::I(InstructionI(3)) }, "CompileSlab{ instrs:{ 0:IVar(\"y\"), 1:IInv(InstructionI(0)), 2:IMul(InstructionI(1), C(3.1415927)), 3:IFuncCos(InstructionI(2)) } }", 0.0);

    // IFuncTan
    comp_chk("tan(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "tan(w)",
        IFuncTan(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IFuncASin
    comp_chk("asin(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "asin(w)",
        IFuncASin(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IFuncACos
    comp_chk(
        "acos(0)",
        IConst(std::f32::consts::FRAC_PI_2), // 1.5707963267948966
        "CompileSlab{ instrs:{} }",
        std::f32::consts::FRAC_PI_2,
    );
    comp_chk(
        "acos(w)",
        IFuncACos(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        std::f32::consts::FRAC_PI_2,
    );

    // IFuncATan
    comp_chk("atan(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "atan(w)",
        IFuncATan(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IFuncSinH
    comp_chk("sinh(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "sinh(w)",
        IFuncSinH(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IFuncCosH
    comp_chk("cosh(0)", IConst(1.0), "CompileSlab{ instrs:{} }", 1.0);
    comp_chk(
        "cosh(w)",
        IFuncCosH(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        1.0,
    );

    // IFuncTanH
    comp_chk("tanh(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "tanh(w)",
        IFuncTanH(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IFuncASinH
    comp_chk("asinh(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "asinh(w)",
        IFuncASinH(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IFuncACosH
    comp_chk("acosh(1)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "acosh(x)",
        IFuncACosH(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        0.0,
    );

    // IFuncATanH
    comp_chk("atanh(0)", IConst(0.0), "CompileSlab{ instrs:{} }", 0.0);
    comp_chk(
        "atanh(w)",
        IFuncATanH(InstructionI(0)),
        "CompileSlab{ instrs:{ 0:IVar(\"w\") } }",
        0.0,
    );

    // IPrintFunc
    comp_chk(
        r#"print("test",1.23)"#,
        IPrintFunc(PrintFunc(vec![
            EStr(String::from("test")),
            EExpr(ExpressionI(0)),
        ])),
        "CompileSlab{ instrs:{} }",
        1.23,
    );
}

#[test]
fn custom_func() {
    comp_chk(
        "x + 1",
        IAdd(InstructionI(0), IC::C(1.0)),
        "CompileSlab{ instrs:{ 0:IVar(\"x\") } }",
        2.0,
    );

    comp_chk("x() + 1", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);

    comp_chk("x(1,2,3) + 1", IConst(2.0), "CompileSlab{ instrs:{} }", 2.0);

    comp_chk(
        "x(1, 1+1, 1+1+1) + 1",
        IConst(2.0),
        "CompileSlab{ instrs:{} }",
        2.0,
    );
}

#[test]
fn eval_macro() {
    fn wrapped() -> Result<(), Error> {
        let mut ns = EmptyNamespace;
        let mut slab = Slab::new();

        let expr = Parser::new()
            .parse("5", &mut slab.ps)
            .unwrap()
            .from(&slab.ps);
        let instr = expr.compile(&slab.ps, &mut slab.cs, &mut ns);
        assert!((eval_compiled_ref!(&instr, &slab, &mut ns) - 5.0).abs() < f32::EPSILON);
        (|| -> Result<(), Error> {
            assert!((eval_compiled_ref!(&instr, &slab, &mut ns) - 5.0).abs() < f32::EPSILON);
            Ok(())
        })()
        .unwrap();
        assert!((eval_compiled!(instr, &slab, &mut ns) - 5.0).abs() < f32::EPSILON);

        #[cfg(feature = "unsafe-vars")]
        {
            let x = 1.0;
            unsafe { slab.ps.add_unsafe_var("x".to_string(), &x) }
            let expr = Parser::new()
                .parse("x", &mut slab.ps)
                .unwrap()
                .from(&slab.ps);
            let instr = expr.compile(&slab.ps, &mut slab.cs, &mut ns);
            assert_eq!(eval_compiled_ref!(&instr, &slab, &mut ns), 1.0);
            (|| -> Result<(), Error> {
                assert_eq!(eval_compiled_ref!(&instr, &slab, &mut ns), 1.0);
                Ok(())
            })()
            .unwrap();
            assert_eq!(eval_compiled!(instr, &slab, &mut ns), 1.0);
        }

        Ok(())
    }

    wrapped().unwrap();
}
