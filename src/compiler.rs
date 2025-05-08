//! This module compiles parsed `Expression`s into an optimized AST node called an `Instruction`.
//! The compiled form is much faster, especially for constants.
//!
//! # Compile-time Optimizations
//!
//! ## Constant Folding
//! Operations with constants can be calculated at compile time so
//! they don't need to be calculated during `eval()`.
//!
//! For example, `1 + x + 1` will be compiled into `x + 2`, saving some time during `eval()`.
//!
//! If the entire expression only consists of constants (no variables),
//! then the expression can be reduced to a final result at compile time,
//! resulting in near-native speed during `eval()`.
//!
//! ## Algebraic Simplification
//! * Subtraction is converted to Addition.
//! * Division is converted to Multiplication.
//! * Built-in functions with constant arguments are evaluated.
//! * Constant terms are combined.
//! * Logical operator short-circuits are applied and no-op branches are discarded.
//!
//! ## Optimized Memory Layout and Execution
//! * Variable-length `Expression`/`Value` AST nodes are converted into constant-sized `Instruction` nodes.
//! * The `IC` enumeration helps to eliminate expensive function calls.

use std::cell::RefCell;

#[cfg(feature = "unsafe-vars")]
use crate::parser::StdFunc::EUnsafeVar;
use crate::slab::{CompileSlab, ParseSlab};
use crate::Error;
use crate::{
    parser::{
        BinaryOp::{
            self, EAdd, EDiv, EExp, EMod, EMul, ESub, EAND, EEQ, EGT, EGTE, ELT, ELTE, ENE, EOR,
        },
        ExprPair, Expression, PrintFunc,
        StdFunc::{
            self, EFunc, EFuncACos, EFuncACosH, EFuncASin, EFuncASinH, EFuncATan, EFuncATanH,
            EFuncAbs, EFuncCeil, EFuncCos, EFuncCosH, EFuncE, EFuncFloor, EFuncInt, EFuncLog,
            EFuncMax, EFuncMin, EFuncPi, EFuncRound, EFuncSign, EFuncSin, EFuncSinH, EFuncTan,
            EFuncTanH, EVar,
        },
        UnaryOp::{self, ENeg, ENot, EParentheses, EPos},
        Value,
    },
    ExpressionI,
};

/// `true` --> `1.0`,  `false` --> `0.0`
#[macro_export]
macro_rules! bool_to_f32 {
    ($b:expr) => {
        if $b {
            1.0_f32
        } else {
            0.0_f32
        }
    };
}

/// An `InstructionI` represents an index into `Slab.cs.instrs`.
///
/// It behaves much like a pointer or reference, but it is 'safe' (unlike a raw
/// pointer) and is not managed by the Rust borrow checker (unlike a reference).
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct InstructionI(pub usize);

/// This enumeration boosts performance because it eliminates expensive function calls for constant values.
#[derive(Debug, PartialEq)]
pub enum IC {
    I(InstructionI),
    C(f32),
}

macro_rules! instr_to_ic {
    ($cslab:ident, $instr:ident) => {
        match $instr {
            IConst(c) => IC::C(c),
            _ => IC::I($cslab.push_instr($instr)),
        }
    };
}
macro_rules! ic_to_instr {
    ($cslab:expr, $dst:ident, $ic:ident) => {
        match $ic {
            IC::C(c) => {
                $dst = IConst(*c);
                &$dst
            }
            IC::I(i) => get_instr!($cslab, i),
        }
    };
}

/// An `Instruction` is an optimized AST node resulting from compilation.
#[derive(Debug, PartialEq)]
pub enum Instruction {
    //---- Primitive Value Types:
    IConst(f32),

    //---- Unary Ops:
    // Parentheses is a noop
    // Pos is a noop
    INeg(InstructionI),
    INot(InstructionI),
    IInv(InstructionI),

    //---- Binary Math Ops:
    IAdd(InstructionI, IC),
    // A Sub(x) is converted to an Add(Neg(x)).
    IMul(InstructionI, IC),
    // A Div(n,d) is converted to a Mul(n,Inv(d)).
    IMod {
        dividend: IC,
        divisor: IC,
    },
    IExp {
        base: IC,
        power: IC,
    },

    //---- Binary Comparison Ops:
    ILT(IC, IC),
    ILTE(IC, IC),
    IEQ(IC, IC),
    INE(IC, IC),
    IGTE(IC, IC),
    IGT(IC, IC),

    //---- Binary Logic Ops:
    IOR(InstructionI, IC),
    IAND(InstructionI, IC),

    //---- Callables:
    IVar(String),
    #[cfg(feature = "unsafe-vars")]
    IUnsafeVar {
        name: String,
        ptr: *const f32,
    },
    IFunc {
        name: String,
        args: Vec<IC>,
    },

    IFuncInt(InstructionI),
    IFuncCeil(InstructionI),
    IFuncFloor(InstructionI),
    IFuncAbs(InstructionI),
    IFuncSign(InstructionI),
    IFuncLog {
        base: IC,
        of: IC,
    },
    IFuncRound {
        modulus: IC,
        of: IC,
    },
    IFuncMin(InstructionI, IC),
    IFuncMax(InstructionI, IC),

    IFuncSin(InstructionI),
    IFuncCos(InstructionI),
    IFuncTan(InstructionI),
    IFuncASin(InstructionI),
    IFuncACos(InstructionI),
    IFuncATan(InstructionI),
    IFuncSinH(InstructionI),
    IFuncCosH(InstructionI),
    IFuncTanH(InstructionI),
    IFuncASinH(InstructionI),
    IFuncACosH(InstructionI),
    IFuncATanH(InstructionI),

    IPrintFunc(PrintFunc), // Not optimized (it would be pointless because of i/o bottleneck).
}
use crate::{eval_var, EvalNamespace};
#[cfg(feature = "unsafe-vars")]
use Instruction::IUnsafeVar;
use Instruction::{
    IAdd, IConst, IExp, IFunc, IFuncACos, IFuncACosH, IFuncASin, IFuncASinH, IFuncATan, IFuncATanH,
    IFuncAbs, IFuncCeil, IFuncCos, IFuncCosH, IFuncFloor, IFuncInt, IFuncLog, IFuncMax, IFuncMin,
    IFuncRound, IFuncSign, IFuncSin, IFuncSinH, IFuncTan, IFuncTanH, IInv, IMod, IMul, INeg, INot,
    IPrintFunc, IVar, IAND, IEQ, IGT, IGTE, ILT, ILTE, INE, IOR,
};

impl Default for Instruction {
    fn default() -> Self {
        IConst(f32::NAN)
    }
}

/// You must `use` the `Compiler` trait before you can call `.compile()` on parsed `Expression`s.
pub trait Compiler {
    /// Turns a parsed `Expression` into a compiled `Instruction`.
    ///
    /// Cannot fail, unless you run out of memory.
    fn compile(
        &self,
        pslab: &ParseSlab,
        cslab: &mut CompileSlab,
        ns: &mut impl EvalNamespace,
    ) -> Instruction;
}

#[derive(Debug)]
struct ExprSlice<'s> {
    first: &'s Value,
    pairs: Vec<&'s ExprPair>,
}

impl<'s> ExprSlice<'s> {
    fn new(first: &Value) -> ExprSlice<'_> {
        ExprSlice {
            first,
            pairs: Vec::with_capacity(8),
        }
    }

    fn from_expr(expr: &Expression) -> ExprSlice<'_> {
        let mut sl = ExprSlice::new(&expr.first);
        for exprpairref in &expr.pairs {
            sl.pairs.push(exprpairref);
        }
        sl
    }

    fn split(&self, bop: BinaryOp, dst: &mut Vec<ExprSlice<'s>>) {
        dst.push(ExprSlice::new(self.first));
        for exprpair in &self.pairs {
            if exprpair.0 == bop {
                dst.push(ExprSlice::new(&exprpair.1));
            } else if let Some(cur) = dst.last_mut() {
                cur.pairs.push(exprpair);
            }
        }
    }

    fn split_multi(
        &self,
        search: &[BinaryOp],
        xsdst: &mut Vec<ExprSlice<'s>>,
        opdst: &mut Vec<&'s BinaryOp>,
    ) {
        xsdst.push(ExprSlice::new(self.first));
        for exprpair in &self.pairs {
            if search.contains(&exprpair.0) {
                xsdst.push(ExprSlice::new(&exprpair.1));
                opdst.push(&exprpair.0);
            } else if let Some(cur) = xsdst.last_mut() {
                cur.pairs.push(exprpair);
            }
        }
    }

    /// Comparison processing step during compilation
    #[inline]
    fn process_comparisons(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        let mut ops = Vec::<&BinaryOp>::with_capacity(4);
        let mut xss = Vec::<ExprSlice>::with_capacity(ops.len() + 1);
        self.split_multi(&[EEQ, ENE, ELT, EGT, ELTE, EGTE], &mut xss, &mut ops);
        let mut out: Instruction = xss.first().map_or(IConst(std::f32::NAN), |xs| {
            xs.compile(parsed_slab, compiled_slab, namespace)
        });

        for (i, op) in ops.into_iter().enumerate() {
            let instruction: Instruction = xss.get(i + 1).map_or(IConst(f32::NAN), |xs| {
                xs.compile(parsed_slab, compiled_slab, namespace)
            });

            if let IConst(l) = out {
                if let IConst(r) = instruction {
                    out = match op {
                        EEQ => IConst(bool_to_f32!(crate::f32_eq!(l, r))),
                        ENE => IConst(bool_to_f32!(crate::f32_ne!(l, r))),
                        ELT => IConst(bool_to_f32!(l < r)),
                        EGT => IConst(bool_to_f32!(l > r)),
                        ELTE => IConst(bool_to_f32!(l <= r)),
                        EGTE => IConst(bool_to_f32!(l >= r)),
                        _ => IConst(std::f32::NAN), // unreachable
                    };
                    continue;
                }
            }
            out = match op {
                EEQ => IEQ(
                    instr_to_ic!(compiled_slab, out),
                    instr_to_ic!(compiled_slab, instruction),
                ),
                ENE => INE(
                    instr_to_ic!(compiled_slab, out),
                    instr_to_ic!(compiled_slab, instruction),
                ),
                ELT => ILT(
                    instr_to_ic!(compiled_slab, out),
                    instr_to_ic!(compiled_slab, instruction),
                ),
                EGT => IGT(
                    instr_to_ic!(compiled_slab, out),
                    instr_to_ic!(compiled_slab, instruction),
                ),
                ELTE => ILTE(
                    instr_to_ic!(compiled_slab, out),
                    instr_to_ic!(compiled_slab, instruction),
                ),
                EGTE => IGTE(
                    instr_to_ic!(compiled_slab, out),
                    instr_to_ic!(compiled_slab, instruction),
                ),
                _ => IConst(std::f32::NAN), // unreachable
            };
        }
        out
    }

    /// OR processing step during compilation
    #[inline]
    fn process_or(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        let mut xss = Vec::<ExprSlice>::with_capacity(4);
        self.split(EOR, &mut xss);
        let mut out = IConst(0.0);
        let mut out_set = false;
        for xs in &xss {
            let instr = xs.compile(parsed_slab, compiled_slab, namespace);
            if out_set {
                out = IOR(
                    compiled_slab.push_instr(out),
                    instr_to_ic!(compiled_slab, instr),
                );
            } else if let IConst(c) = instr {
                if crate::f32_ne!(c, 0.0) {
                    return instr;
                }
            } else {
                out = instr;
                out_set = true;
            }
        }
        out
    }

    /// AND processing step during compilation
    #[inline]
    fn process_and(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        let mut xss = Vec::<ExprSlice>::with_capacity(4);
        self.split(EAND, &mut xss);
        let mut out = IConst(1.0);
        let mut out_set = false;
        for xs in &xss {
            let instr = xs.compile(parsed_slab, compiled_slab, namespace);
            if let IConst(c) = instr {
                if crate::f32_eq!(c, 0.0) {
                    return instr;
                }
            }
            if out_set {
                if let IConst(_) = out {
                    // If we get here, we know that the const is non-zero.
                    out = instr;
                } else {
                    out = IAND(
                        compiled_slab.push_instr(out),
                        instr_to_ic!(compiled_slab, instr),
                    );
                }
            } else {
                out = instr;
                out_set = true;
            }
        }
        out
    }

    /// Addition processing step during compilation
    #[inline]
    fn process_addition(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        let mut xss = Vec::<ExprSlice>::with_capacity(4);
        self.split(EAdd, &mut xss);
        let mut instrs = Vec::<Instruction>::with_capacity(xss.len());
        for xs in xss {
            let instr = xs.compile(parsed_slab, compiled_slab, namespace);
            if let IAdd(li, ric) = instr {
                push_add_leaves(&mut instrs, compiled_slab, li, &ric); // Flatten nested structures like "x - 1 + 2 - 3".
            } else {
                instrs.push(instr);
            }
        }
        compile_add(instrs, compiled_slab)
    }

    /// Subtraction processing step during compilation
    #[inline]
    fn process_subtraction(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        // Note: We don't need to push_add_leaves from here because Sub has a higher precedence than Add.

        let mut xss = Vec::<ExprSlice>::with_capacity(4);
        self.split(ESub, &mut xss);
        let mut instrs = Vec::<Instruction>::with_capacity(xss.len());
        for (i, xs) in xss.into_iter().enumerate() {
            let instr = xs.compile(parsed_slab, compiled_slab, namespace);
            if i == 0 {
                instrs.push(instr);
            } else {
                instrs.push(neg_wrap(instr, compiled_slab));
            }
        }
        compile_add(instrs, compiled_slab)
    }

    /// Multiplication processing step during compilation.
    #[inline]
    fn process_multiplication(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        let mut xss = Vec::<ExprSlice>::with_capacity(4);
        self.split(EMul, &mut xss);
        let mut instrs = Vec::<Instruction>::with_capacity(xss.len());
        for xs in xss {
            let instr = xs.compile(parsed_slab, compiled_slab, namespace);
            if let IMul(li, ric) = instr {
                push_mul_leaves(&mut instrs, compiled_slab, li, &ric); // Flatten nested structures like "deg/360 * 2*pi()".
            } else {
                instrs.push(instr);
            }
        }
        compile_mul(instrs, compiled_slab)
    }
}

/// Creates process functions dynamically.
/// Each match arm provides different variations of similar function styles.
/// The first is used to define Trigonometric functions.
macro_rules! process_fn {
    ($name:ident, $operation:ident, $fallback:ident) => {
        #[inline]
        fn $name(
            parsed_slab: &ParseSlab,
            compiled_slab: &mut CompileSlab,
            namespace: &mut impl EvalNamespace,
            expr: ExpressionI,
        ) -> Instruction {
            let instruction =
                get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
            if let IConst(target) = instruction {
                IConst(target.$operation())
            } else {
                $fallback(compiled_slab.push_instr(instruction))
            }
        }
    };
}

/// Uses [`EPSILON`](https://doc.rust-lang.org/core/f32/constant.EPSILON.html) to determine equality of two `f32`s.
#[macro_export]
macro_rules! f32_eq {
    ($l:ident, $r:literal) => {
        ($l - $r).abs() <= 8.0 * f32::EPSILON
    };
    ($l:ident, $r:ident) => {
        ($l - $r).abs() <= 8.0 * f32::EPSILON
    };
    ($l:expr, $r:literal) => {
        ($l - $r).abs() <= 8.0 * f32::EPSILON
    };
    ($l:expr, $r:expr) => {
        (($l) - ($r)).abs() <= 8.0 * f32::EPSILON
    };
}

/// Uses [`EPSILON`](https://doc.rust-lang.org/core/f32/constant.EPSILON.html) to determine inequality of two `f32`s.
///
/// This is exactly the same as saying `!f32_eq(x,y)` but it is slightly more efficient.
#[macro_export]
macro_rules! f32_ne {
    ($l:ident, $r:literal) => {
        ($l - $r).abs() > 8.0 * f32::EPSILON
    };
    ($l:ident, $r:ident) => {
        ($l - $r).abs() > 8.0 * f32::EPSILON
    };
    ($l:expr, $r:literal) => {
        ($l - $r).abs() > 8.0 * f32::EPSILON
    };
    ($l:expr, $r:expr) => {
        (($l) - ($r)).abs() > 8.0 * f32::EPSILON
    };
}
fn neg_wrap(instr: Instruction, cslab: &mut CompileSlab) -> Instruction {
    if let IConst(c) = instr {
        IConst(-c)
    } else if let INeg(i) = instr {
        cslab.take_instr(i)
    } else {
        INeg(cslab.push_instr(instr))
    }
}
fn not_wrap(instr: Instruction, cslab: &mut CompileSlab) -> Instruction {
    if let IConst(c) = instr {
        IConst(bool_to_f32!(f32_eq!(c, 0.0)))
    } else if let INot(i) = instr {
        cslab.take_instr(i)
    } else {
        INot(cslab.push_instr(instr))
    }
}
fn inv_wrap(instr: Instruction, cslab: &mut CompileSlab) -> Instruction {
    if let IConst(c) = instr {
        IConst(1.0 / c)
    } else if let IInv(i) = instr {
        cslab.take_instr(i)
    } else {
        IInv(cslab.push_instr(instr))
    }
}
fn compile_mul(instrs: Vec<Instruction>, cslab: &mut CompileSlab) -> Instruction {
    let mut out = IConst(1.0);
    let mut out_set = false;
    let mut const_prod = 1.0;
    for instr in instrs {
        if let IConst(c) = instr {
            const_prod *= c; // Floats don't overflow.
        } else if out_set {
            out = IMul(cslab.push_instr(out), IC::I(cslab.push_instr(instr)));
        } else {
            out = instr;
            out_set = true;
        }
    }
    if f32_ne!(const_prod, 1.0) {
        if out_set {
            out = IMul(cslab.push_instr(out), IC::C(const_prod));
        } else {
            out = IConst(const_prod);
        }
    }
    out
}
fn compile_add(instrs: Vec<Instruction>, cslab: &mut CompileSlab) -> Instruction {
    let mut out = IConst(0.0);
    let mut out_set = false;
    let mut const_sum = 0.0;
    for instr in instrs {
        if let IConst(c) = instr {
            const_sum += c; // Floats don't overflow.
        } else if out_set {
            out = IAdd(cslab.push_instr(out), IC::I(cslab.push_instr(instr)));
        } else {
            out = instr;
            out_set = true;
        }
    }
    if f32_ne!(const_sum, 0.0) {
        if out_set {
            out = IAdd(cslab.push_instr(out), IC::C(const_sum));
        } else {
            out = IConst(const_sum);
        }
    }
    out
}
pub(crate) fn log(base: f32, n: f32) -> f32 {
    // Can't use floating point in 'match' patterns.  :(
    if f32_eq!(base, 2.0) {
        return n.log2();
    }
    if f32_eq!(base, 10.0) {
        return n.log10();
    }
    n.log(base)
}

// Can't inline recursive functions:
fn push_mul_leaves(
    instrs: &mut Vec<Instruction>,
    cslab: &mut CompileSlab,
    li: InstructionI,
    ric: &IC,
) {
    // Take 'r' before 'l' for a chance for more efficient memory usage:
    match *ric {
        IC::I(ri) => {
            let instr = cslab.take_instr(ri);
            if let IMul(rli, rric) = instr {
                push_mul_leaves(instrs, cslab, rli, &rric);
            } else {
                instrs.push(instr);
            }
        }
        IC::C(c) => instrs.push(IConst(c)),
    };

    let instr = cslab.take_instr(li);
    if let IMul(lli, lric) = instr {
        push_mul_leaves(instrs, cslab, lli, &lric);
    } else {
        instrs.push(instr);
    }
}
fn push_add_leaves(
    instrs: &mut Vec<Instruction>,
    cslab: &mut CompileSlab,
    li: InstructionI,
    ric: &IC,
) {
    // Take 'r' before 'l' for a chance for more efficient memory usage:
    match *ric {
        IC::I(ri) => {
            let instr = cslab.take_instr(ri);
            if let IAdd(rli, rric) = instr {
                push_add_leaves(instrs, cslab, rli, &rric);
            } else {
                instrs.push(instr);
            }
        }
        IC::C(c) => instrs.push(IConst(c)),
    };

    let instr = cslab.take_instr(li);
    if let IAdd(lli, lric) = instr {
        push_add_leaves(instrs, cslab, lli, &lric);
    } else {
        instrs.push(instr);
    }
}

impl Compiler for ExprSlice<'_> {
    fn compile(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        // Associative:  (2+3)+4 = 2+(3+4)
        // Commutative:  1+2 = 2+1
        //
        //          Only         Only
        // Neither  Associative  Commutative  Both
        // -------  -----------  -----------  ----
        // GTE      (none)       (none)       OR
        // LTE                                AND
        // GT                                 NE
        // LT                                 EQ
        // Minus (opt with neg & add)         Plus
        // Div (opt with inv & mul)           Mul
        // Mod
        // Exp

        // Find the lowest-priority BinaryOp:
        let mut lowest_op = match self.pairs.first() {
            Some(p0) => p0.0,
            None => return self.first.compile(parsed_slab, compiled_slab, namespace),
        };
        for exprpair in &self.pairs {
            if exprpair.0 < lowest_op {
                lowest_op = exprpair.0;
            }
        }

        // All comparisons have equal precedence:
        if lowest_op == EEQ
            || lowest_op == ENE
            || lowest_op == ELT
            || lowest_op == EGT
            || lowest_op == ELTE
            || lowest_op == EGTE
        {
            return self.process_comparisons(parsed_slab, compiled_slab, namespace);
        }

        match lowest_op {
            EOR => self.process_or(parsed_slab, compiled_slab, namespace),
            EAND => self.process_and(parsed_slab, compiled_slab, namespace),
            EAdd => self.process_addition(parsed_slab, compiled_slab, namespace),
            ESub => self.process_subtraction(parsed_slab, compiled_slab, namespace),
            EMul => self.process_multiplication(parsed_slab, compiled_slab, namespace),
            EDiv => {
                // Note: We don't need to push_mul_leaves from here because Div has a higher precedence than Mul.

                let mut xss = Vec::<ExprSlice>::with_capacity(4);
                self.split(EDiv, &mut xss);
                let mut instrs = Vec::<Instruction>::with_capacity(xss.len());
                for (i, xs) in xss.into_iter().enumerate() {
                    let instr = xs.compile(parsed_slab, compiled_slab, namespace);
                    if i == 0 {
                        instrs.push(instr);
                    } else {
                        instrs.push(inv_wrap(instr, compiled_slab));
                    }
                }
                compile_mul(instrs, compiled_slab)
            }
            //          EDiv => {
            //              let mut xss = Vec::<ExprSlice>::with_capacity(4);
            //              self.split(EDiv, &mut xss);
            //              let mut out = IConst(1.0); let mut out_set = false;
            //              let mut const_prod = 1.0;
            //              let mut is_first = true;
            //              for xs in xss.iter() {
            //                  let instr = xs.compile(pslab,cslab,ns);
            //                  if let IConst(c) = instr {
            //                      if is_first {
            //                          const_prod *= c;  // Floats don't overflow.
            //                      } else {
            //                          const_prod /= c;
            //                      }
            //                  } else {
            //                      if is_first {
            //                          if out_set {
            //                              out = IMul(cslab.push_instr(out), cslab.push_instr(instr));
            //                          } else {
            //                              out = instr;
            //                              out_set = true;
            //                          }
            //                      } else {
            //                          let instr = inv_wrap(instr,cslab);
            //                          if out_set {
            //                              out = IMul(cslab.push_instr(out), cslab.push_instr(instr));
            //                          } else {
            //                              out = instr;
            //                              out_set = true;
            //                          }
            //                      }
            //                  }
            //                  is_first = false;
            //              }
            //              if f32_ne!(const_prod,1.0) {
            //                  if out_set {
            //                      out = IMul(cslab.push_instr(out), cslab.push_instr(IConst(const_prod)));
            //                  } else {
            //                      out = IConst(const_prod);
            //                  }
            //              }
            //              out
            //          }
            EMod => {
                let mut xss = Vec::<ExprSlice>::with_capacity(2);
                self.split(EMod, &mut xss);
                let mut out = IConst(0.0);
                let mut out_set = false;
                for xs in &xss {
                    let instr = xs.compile(parsed_slab, compiled_slab, namespace);
                    if out_set {
                        if let IConst(dividend) = out {
                            if let IConst(divisor) = instr {
                                out = IConst(dividend % divisor);
                                continue;
                            }
                        }
                        out = IMod {
                            dividend: instr_to_ic!(compiled_slab, out),
                            divisor: instr_to_ic!(compiled_slab, instr),
                        };
                    } else {
                        out = instr;
                        out_set = true;
                    }
                }
                out
            }
            EExp => {
                // Right-to-Left Associativity
                let mut xss = Vec::<ExprSlice>::with_capacity(2);
                self.split(EExp, &mut xss);
                let mut out = IConst(0.0);
                let mut out_set = false;
                for xs in xss.into_iter().rev() {
                    let instr = xs.compile(parsed_slab, compiled_slab, namespace);
                    if out_set {
                        if let IConst(power) = out {
                            if let IConst(base) = instr {
                                out = IConst(base.powf(power));
                                continue;
                            }
                        }
                        out = IExp {
                            base: instr_to_ic!(compiled_slab, instr),
                            power: instr_to_ic!(compiled_slab, out),
                        };
                    } else {
                        out = instr;
                        out_set = true;
                    }
                }
                out
            }
            //          EExp => {  // Left-to-Right Associativity
            //              let mut xss = Vec::<ExprSlice>::with_capacity(2);
            //              self.split(EExp, &mut xss);
            //              let mut pow_instrs = Vec::<Instruction>::with_capacity(xss.len()-1);
            //              let mut base = IConst(0.0);
            //              for (i,xs) in xss.into_iter().enumerate() {
            //                  let instr = xs.compile(pslab,cslab,ns);
            //                  if i==0 {
            //                      base = instr;
            //                  } else {
            //                      pow_instrs.push(instr);
            //                  }
            //              }
            //              let power = compile_mul(pow_instrs,cslab);
            //              if let IConst(b) = base {
            //                  if let IConst(p) = power {
            //                      return IConst(b.powf(p));
            //                  }
            //              }
            //              IExp{base:cslab.push_instr(base), power:cslab.push_instr(power)}
            //          }
            ENE | EEQ | EGTE | ELTE | EGT | ELT => IConst(std::f32::NAN), // unreachable
        }
    }
}

impl Compiler for Expression {
    fn compile(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        ns: &mut impl EvalNamespace,
    ) -> Instruction {
        let top = ExprSlice::from_expr(self);
        top.compile(parsed_slab, compiled_slab, ns)
    }
}

impl Compiler for Value {
    fn compile(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        ns: &mut impl EvalNamespace,
    ) -> Instruction {
        match self {
            Self::EConstant(c) => IConst(*c),
            Self::EUnaryOp(u) => u.compile(parsed_slab, compiled_slab, ns),
            Self::EStdFunc(f) => f.compile(parsed_slab, compiled_slab, ns),
            Self::EPrintFunc(pf) => IPrintFunc(pf.clone()),
        }
    }
}

impl Compiler for UnaryOp {
    fn compile(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        ns: &mut impl EvalNamespace,
    ) -> Instruction {
        match self {
            EPos(i) => get_val!(parsed_slab, i).compile(parsed_slab, compiled_slab, ns),
            ENeg(i) => {
                let instr = get_val!(parsed_slab, i).compile(parsed_slab, compiled_slab, ns);
                if let IConst(c) = instr {
                    IConst(-c)
                } else {
                    neg_wrap(instr, compiled_slab)
                }
            }
            ENot(i) => {
                let instr = get_val!(parsed_slab, i).compile(parsed_slab, compiled_slab, ns);
                if let IConst(c) = instr {
                    IConst(bool_to_f32!(f32_eq!(c, 0.0)))
                } else {
                    not_wrap(instr, compiled_slab)
                }
            }
            EParentheses(i) => get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, ns),
        }
    }
}

impl StdFunc {
    /// Custom Function processing step during compilation.
    #[inline]
    fn process_custom_fn(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        name: &String,
        expressions: &Vec<ExpressionI>,
        celled_parsed_slab: &RefCell<String>,
    ) -> Instruction {
        let mut args = Vec::<IC>::with_capacity(expressions.len());
        let mut f32_args = Vec::<f32>::with_capacity(expressions.len());
        let mut is_all_const = true;
        for expr in expressions {
            let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
            if let IConst(c) = instr {
                f32_args.push(c);
            } else {
                is_all_const = false;
            }
            args.push(instr_to_ic!(compiled_slab, instr));
        }
        if is_all_const {
            let computed_value = eval_var!(
                namespace,
                name,
                f32_args,
                &mut celled_parsed_slab.borrow_mut()
            );
            computed_value.map_or_else(
                |_| IFunc {
                    name: name.clone(),
                    args,
                },
                IConst,
            )
        } else {
            IFunc {
                name: name.clone(),
                args,
            }
        }
    }

    /// Integer Function processing step during compilation.
    #[inline]
    fn process_int_fn(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        expression: ExpressionI,
    ) -> Instruction {
        let instr =
            get_expr!(parsed_slab, expression).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(c) = instr {
            IConst(c.trunc())
        } else {
            IFuncInt(compiled_slab.push_instr(instr))
        }
    }

    /// Ceiling processing step during compilation.
    #[inline]
    fn process_ceil_fn(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        expr: ExpressionI,
    ) -> Instruction {
        let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(c) = instr {
            IConst(c.ceil())
        } else {
            IFuncCeil(compiled_slab.push_instr(instr))
        }
    }

    /// Flooring processing step during compilation.
    #[inline]
    fn process_floor_fn(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        expr: ExpressionI,
    ) -> Instruction {
        let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(c) = instr {
            IConst(c.floor())
        } else {
            IFuncFloor(compiled_slab.push_instr(instr))
        }
    }

    /// Absolute Value processing step during compilation.
    #[inline]
    fn process_abs_fn(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        expr: ExpressionI,
    ) -> Instruction {
        let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(c) = instr {
            IConst(c.abs())
        } else {
            IFuncAbs(compiled_slab.push_instr(instr))
        }
    }

    /// Sign processing step during compilation
    #[inline]
    fn process_signum(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        expr: ExpressionI,
    ) -> Instruction {
        let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(c) = instr {
            IConst(c.signum())
        } else {
            IFuncSign(compiled_slab.push_instr(instr))
        }
    }

    /// Logarithm processing step during compilation.
    #[inline]
    fn process_log(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        base_options: &Option<ExpressionI>,
        expr: ExpressionI,
    ) -> Instruction {
        let base: Instruction = base_options.as_ref().map_or(IConst(10.0), |bi| {
            get_expr!(parsed_slab, bi).compile(parsed_slab, compiled_slab, namespace)
        });
        let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(b) = base {
            if let IConst(n) = instr {
                return IConst(log(b, n));
            }
        }
        IFuncLog {
            base: instr_to_ic!(compiled_slab, base),
            of: instr_to_ic!(compiled_slab, instr),
        }
    }

    /// Rounding processing step during compilation.
    #[inline]
    fn process_round(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        mod_option: &Option<ExpressionI>,
        expr: ExpressionI,
    ) -> Instruction {
        let modulus: Instruction = mod_option.as_ref().map_or(IConst(1.0), |mi| {
            get_expr!(parsed_slab, mi).compile(parsed_slab, compiled_slab, namespace)
        });
        let instr = get_expr!(parsed_slab, expr).compile(parsed_slab, compiled_slab, namespace);
        if let IConst(m) = modulus {
            if let IConst(n) = instr {
                return IConst((n / m).round() * m); // Floats don't overflow.
            }
        }
        IFuncRound {
            modulus: instr_to_ic!(compiled_slab, modulus),
            of: instr_to_ic!(compiled_slab, instr),
        }
    }

    /// Min processing step during compilation.
    #[inline]
    fn process_min(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        fi: ExpressionI,
        is: &Vec<ExpressionI>,
    ) -> Instruction {
        let first = get_expr!(parsed_slab, fi).compile(parsed_slab, compiled_slab, namespace);
        let mut rest = Vec::<Instruction>::with_capacity(is.len());
        for i in is {
            rest.push(get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace));
        }
        let mut out = IConst(0.0);
        let mut out_set = false;
        let mut const_min = 0.0;
        let mut const_min_set = false;
        if let IConst(f) = first {
            const_min = f;
            const_min_set = true;
        } else {
            out = first;
            out_set = true;
        }
        for instr in rest {
            if let IConst(f) = instr {
                if const_min_set {
                    if f < const_min {
                        const_min = f;
                    }
                } else {
                    const_min = f;
                    const_min_set = true;
                }
            } else if out_set {
                out = IFuncMin(
                    compiled_slab.push_instr(out),
                    IC::I(compiled_slab.push_instr(instr)),
                );
            } else {
                out = instr;
                out_set = true;
            }
        }
        if const_min_set {
            if out_set {
                out = IFuncMin(compiled_slab.push_instr(out), IC::C(const_min));
            } else {
                out = IConst(const_min);
                // out_set = true;  // Comment out so the compiler doesn't complain about unused assignments.
            }
        }
        //assert!(out_set);
        out
    }

    /// Max processing step during compilation.
    #[inline]
    fn process_max(
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
        fi: ExpressionI,
        is: &Vec<ExpressionI>,
    ) -> Instruction {
        let first = get_expr!(parsed_slab, fi).compile(parsed_slab, compiled_slab, namespace);
        let mut rest = Vec::<Instruction>::with_capacity(is.len());
        for i in is {
            rest.push(get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace));
        }
        let mut out = IConst(0.0);
        let mut out_set = false;
        let mut const_max = 0.0;
        let mut const_max_set = false;
        if let IConst(f) = first {
            const_max = f;
            const_max_set = true;
        } else {
            out = first;
            out_set = true;
        }
        for instr in rest {
            if let IConst(f) = instr {
                if const_max_set {
                    if f > const_max {
                        const_max = f;
                    }
                } else {
                    const_max = f;
                    const_max_set = true;
                }
            } else if out_set {
                out = IFuncMax(
                    compiled_slab.push_instr(out),
                    IC::I(compiled_slab.push_instr(instr)),
                );
            } else {
                out = instr;
                out_set = true;
            }
        }
        if const_max_set {
            if out_set {
                out = IFuncMax(compiled_slab.push_instr(out), IC::C(const_max));
            } else {
                out = IConst(const_max);
                // out_set = true;  // Comment out so the compiler doesn't complain about unused assignments.
            }
        }
        //assert!(out_set);
        out
    }

    process_fn!(process_sin, sin, IFuncSin);
    process_fn!(process_cos, cos, IFuncCos);
    process_fn!(process_tan, tan, IFuncTan);
    process_fn!(process_asin, asin, IFuncASin);
    process_fn!(process_acos, acos, IFuncACos);
    process_fn!(process_atan, atan, IFuncATan);
}

impl Compiler for StdFunc {
    fn compile(
        &self,
        parsed_slab: &ParseSlab,
        compiled_slab: &mut CompileSlab,
        namespace: &mut impl EvalNamespace,
    ) -> Instruction {
        let celled_parsed_slab = RefCell::from(parsed_slab.char_buf.clone());
        match self {
            EVar(name) => IVar(name.clone()),
            #[cfg(feature = "unsafe-vars")]
            EUnsafeVar { name, ptr } => IUnsafeVar {
                name: name.clone(),
                ptr: *ptr,
            },
            EFunc { name, args } => Self::process_custom_fn(
                parsed_slab,
                compiled_slab,
                namespace,
                name,
                args,
                &celled_parsed_slab,
            ),

            EFuncInt(expr) => Self::process_int_fn(parsed_slab, compiled_slab, namespace, *expr),
            EFuncCeil(expr) => Self::process_ceil_fn(parsed_slab, compiled_slab, namespace, *expr),
            EFuncFloor(expr) => {
                Self::process_floor_fn(parsed_slab, compiled_slab, namespace, *expr)
            }
            EFuncAbs(expr) => Self::process_abs_fn(parsed_slab, compiled_slab, namespace, *expr),
            EFuncSign(expr) => Self::process_signum(parsed_slab, compiled_slab, namespace, *expr),
            EFuncLog {
                base: base_option,
                expr,
            } => Self::process_log(parsed_slab, compiled_slab, namespace, base_option, *expr),
            EFuncRound {
                modulus: mod_option,
                expr,
            } => Self::process_round(parsed_slab, compiled_slab, namespace, mod_option, *expr),
            EFuncMin {
                first: fi,
                rest: is,
            } => Self::process_min(parsed_slab, compiled_slab, namespace, *fi, is),
            EFuncMax {
                first: fi,
                rest: is,
            } => Self::process_max(parsed_slab, compiled_slab, namespace, *fi, is),

            EFuncE => IConst(std::f32::consts::E),
            EFuncPi => IConst(std::f32::consts::PI),

            EFuncSin(expr) => Self::process_sin(parsed_slab, compiled_slab, namespace, *expr),
            EFuncCos(expr) => Self::process_cos(parsed_slab, compiled_slab, namespace, *expr),
            EFuncTan(expr) => Self::process_tan(parsed_slab, compiled_slab, namespace, *expr),
            EFuncASin(expr) => Self::process_asin(parsed_slab, compiled_slab, namespace, *expr),
            EFuncACos(expr) => Self::process_acos(parsed_slab, compiled_slab, namespace, *expr),
            EFuncATan(expr) => Self::process_atan(parsed_slab, compiled_slab, namespace, *expr),
            EFuncSinH(i) => {
                let instr =
                    get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace);
                if let IConst(c) = instr {
                    IConst(c.sinh())
                } else {
                    IFuncSinH(compiled_slab.push_instr(instr))
                }
            }
            EFuncCosH(i) => {
                let instr =
                    get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace);
                if let IConst(c) = instr {
                    IConst(c.cosh())
                } else {
                    IFuncCosH(compiled_slab.push_instr(instr))
                }
            }
            EFuncTanH(i) => {
                let instr =
                    get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace);
                if let IConst(c) = instr {
                    IConst(c.tanh())
                } else {
                    IFuncTanH(compiled_slab.push_instr(instr))
                }
            }
            EFuncASinH(i) => {
                let instr =
                    get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace);
                if let IConst(c) = instr {
                    IConst(c.asinh())
                } else {
                    IFuncASinH(compiled_slab.push_instr(instr))
                }
            }
            EFuncACosH(i) => {
                let instr =
                    get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace);
                if let IConst(c) = instr {
                    IConst(c.acosh())
                } else {
                    IFuncACosH(compiled_slab.push_instr(instr))
                }
            }
            EFuncATanH(i) => {
                let instr =
                    get_expr!(parsed_slab, i).compile(parsed_slab, compiled_slab, namespace);
                if let IConst(c) = instr {
                    IConst(c.atanh())
                } else {
                    IFuncATanH(compiled_slab.push_instr(instr))
                }
            }
        }
    }
}
