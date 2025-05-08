//! This module evaluates parsed `Expression`s and compiled `Instruction`s.
//!
//! Everything can be evaluated using the `.eval()` method, but compiled
//! `Instruction`s also have the option of using the `eval_compiled!()` macro
//! which is much faster for common cases.

use crate as fasteval3;

#[cfg(feature = "unsafe-vars")]
use crate::compiler::Instruction::IUnsafeVar;
use crate::compiler::{
    log,
    Instruction::{
        self, IAdd, IConst, IExp, IFunc, IFuncACos, IFuncACosH, IFuncASin, IFuncASinH, IFuncATan,
        IFuncATanH, IFuncAbs, IFuncCeil, IFuncCos, IFuncCosH, IFuncFloor, IFuncInt, IFuncLog,
        IFuncMax, IFuncMin, IFuncRound, IFuncSign, IFuncSin, IFuncSinH, IFuncTan, IFuncTanH, IInv,
        IMod, IMul, INeg, INot, IPrintFunc, IVar, IAND, IEQ, IGT, IGTE, ILT, ILTE, INE, IOR,
    },
    IC,
};
use crate::error::Error;
use crate::evalns::EvalNamespace;
#[cfg(feature = "unsafe-vars")]
use crate::parser::StdFunc::EUnsafeVar;
use crate::parser::{
    remove_no_panic,
    BinaryOp::{
        self, EAdd, EDiv, EExp, EMod, EMul, ESub, EAND, EEQ, EGT, EGTE, ELT, ELTE, ENE, EOR,
    },
    Expression,
    ExpressionOrString::{EExpr, EStr},
    PrintFunc,
    StdFunc::{
        self, EFunc, EFuncACos, EFuncACosH, EFuncASin, EFuncASinH, EFuncATan, EFuncATanH, EFuncAbs,
        EFuncCeil, EFuncCos, EFuncCosH, EFuncE, EFuncFloor, EFuncInt, EFuncLog, EFuncMax, EFuncMin,
        EFuncPi, EFuncRound, EFuncSign, EFuncSin, EFuncSinH, EFuncTan, EFuncTanH, EVar,
    },
    UnaryOp::{self, ENeg, ENot, EParentheses, EPos},
    Value::{self, EConstant, EPrintFunc, EStdFunc, EUnaryOp},
};
use crate::slab::Slab;

use std::f32::consts;
use std::fmt;
use std::{cell::RefCell, collections::BTreeSet};

/// The same as `evaler.eval(&slab, &mut ns)`, but more efficient for common cases.
///
/// This macro is exactly the same as [`eval_compiled_ref!()`](macro.eval_compiled_ref.html)
/// but is more efficient if you have ownership of the evaler.
///
/// Only use this for compiled expressions.  (If you use it for interpreted
/// expressions, it will work but will always be slower than calling `eval()` directly.)
///
/// This macro is able to eliminate function calls for constants and Unsafe Variables.
/// Since evaluation is a performance-critical operation, saving some function
/// calls actually makes a huge performance difference.
///
#[macro_export]
macro_rules! eval_compiled {
    ($evaler:ident, $slab_ref:expr, $ns_mut:expr) => {
        if let fasteval3::IConst(c) = $evaler {
            c
        } else {
            #[cfg(feature = "unsafe-vars")]
            {
                if let fasteval3::IUnsafeVar { ptr, .. } = $evaler {
                    unsafe { *ptr }
                } else {
                    $evaler.eval($slab_ref, $ns_mut)?
                }
            }

            #[cfg(not(feature = "unsafe-vars"))]
            $evaler.eval($slab_ref, $ns_mut)?
        }
    };
    ($evaler:expr, $slab_ref:expr, $ns_mut:expr) => {{
        let evaler = $evaler;
        eval_compiled!(evaler, $slab_ref, $ns_mut)
    }};
}

/// The same as `evaler_ref.eval(&slab, &mut ns)`, but more efficient for common cases.
///
/// This macro is exactly the same as [`eval_compiled!()`](macro.eval_compiled.html) but
/// is useful when you hold a reference to the evaler, rather than having ownership of it.
///
/// Only use this for compiled expressions.  (If you use it for interpreted
/// expressions, it will work but will always be slower than calling `eval()` directly.)
///
/// This macro is able to eliminate function calls for constants and Unsafe Variables.
/// Since evaluation is a performance-critical operation, saving some function
/// calls actually makes a huge performance difference.
///
#[macro_export]
macro_rules! eval_compiled_ref {
    ($evaler:ident, $slab_ref:expr, $ns_mut:expr) => {
        if let fasteval3::IConst(c) = $evaler {
            *c
        } else {
            #[cfg(feature = "unsafe-vars")]
            {
                if let fasteval3::IUnsafeVar { ptr, .. } = $evaler {
                    unsafe { **ptr }
                } else {
                    $evaler.eval($slab_ref, $ns_mut)?
                }
            }

            #[cfg(not(feature = "unsafe-vars"))]
            $evaler.eval($slab_ref, $ns_mut)?
        }
    };
    ($evaler:expr, $slab_ref:expr, $ns_mut:expr) => {{
        let evaler = $evaler;
        eval_compiled_ref!(evaler, $slab_ref, $ns_mut)
    }};
}

macro_rules! eval_ic_ref {
    ($ic:ident, $slab_ref:ident, $ns_mut:expr) => {
        match $ic {
            IC::C(c) => *c,
            IC::I(i) => {
                let instr_ref = get_instr!($slab_ref.cs, i);

                #[cfg(feature = "unsafe-vars")]
                {
                    if let fasteval3::IUnsafeVar { ptr, .. } = instr_ref {
                        unsafe { **ptr }
                    } else {
                        instr_ref.eval($slab_ref, $ns_mut)?
                    }
                }

                #[cfg(not(feature = "unsafe-vars"))]
                instr_ref.eval($slab_ref, $ns_mut)?
            }
        }
    };
}

/// You must `use` this trait so you can call `.eval()`.
pub trait Evaler: fmt::Debug {
    /// Evaluate this `Expression`/`Instruction` and return an `f32`.
    ///
    /// # Errors
    ///
    /// Returns a `fasteval3::Error` if there are any problems, such as undefined variables.
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error>;

    /// Don't call this directly.  Use `var_names()` instead.
    ///
    /// This exists because of ternary short-circuits; they prevent us from
    /// getting a complete list of vars just by doing eval() with a clever
    /// callback.
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>);

    /// Returns a list of variables and custom functions that are used by this `Expression`/`Instruction`.
    fn var_names(&self, slab: &Slab) -> BTreeSet<String> {
        let mut set = BTreeSet::new();
        self._var_names(slab, &mut set);
        set
    }
}

#[allow(clippy::inline_always)] // TODO: Check to see if always inlining here is ok.
impl Evaler for Expression {
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>) {
        self.first._var_names(slab, dst);
        for pair in &self.pairs {
            pair.1._var_names(slab, dst);
        }
    }
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error> {
        #[inline(always)]
        fn rtol(vals: &mut Vec<f32>, ops: &mut Vec<BinaryOp>, search: BinaryOp) {
            for i in (0..ops.len()).rev() {
                let op = ops.get(i).map_or(EOR, |op| *op);
                if op == search {
                    let res = op.binaryop_eval(vals.get(i), vals.get(i + 1));
                    if let Some(value_ref) = vals.get_mut(i) {
                        *value_ref = res;
                    }
                    remove_no_panic(vals, i + 1);
                    remove_no_panic(ops, i);
                }
            }
        }
        #[inline(always)]
        fn ltor(vals: &mut Vec<f32>, ops: &mut Vec<BinaryOp>, search: BinaryOp) {
            let mut i = 0;
            loop {
                match ops.get(i) {
                    None => break,
                    Some(op) => {
                        if *op == search {
                            let res = op.binaryop_eval(vals.get(i), vals.get(i + 1));
                            if let Some(value_ref) = vals.get_mut(i) {
                                *value_ref = res;
                            }
                            remove_no_panic(vals, i + 1);
                            remove_no_panic(ops, i);
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }
        #[inline(always)]
        fn ltor_multi(vals: &mut Vec<f32>, ops: &mut Vec<BinaryOp>, search: &[BinaryOp]) {
            let mut i = 0;
            loop {
                match ops.get(i) {
                    None => break,
                    Some(op) => {
                        if search.contains(op) {
                            let res = op.binaryop_eval(vals.get(i), vals.get(i + 1));
                            if let Some(value_ref) = vals.get_mut(i) {
                                *value_ref = res;
                            }
                            remove_no_panic(vals, i + 1);
                            remove_no_panic(ops, i);
                        } else {
                            i += 1;
                        }
                    }
                }
            }
        }

        // Order of operations: 1) ^  2) */  3) +-
        // Exponentiation should be processed right-to-left.  Think of what 2^3^4 should mean:
        //     2^(3^4)=2417851639229258349412352   <--- I choose this one.  https://codeplea.com/exponentiation-associativity-options
        //     (2^3)^4=4096
        // Direction of processing doesn't matter for Addition and Multiplication:
        //     (((3+4)+5)+6)==(3+(4+(5+6))), (((3*4)*5)*6)==(3*(4*(5*6)))
        // ...But Subtraction and Division must be processed left-to-right:
        //     (((6-5)-4)-3)!=(6-(5-(4-3))), (((6/5)/4)/3)!=(6/(5/(4/3)))

        // // ---- Go code, for comparison ----
        // // vals,ops:=make([]float64, len(e)/2+1),make([]BinaryOp, len(e)/2)
        // // for i:=0; i<len(e); i+=2 {
        // //     vals[i/2]=ns.EvalBubble(e[i].(evaler))
        // //     if i<len(e)-1 { ops[i/2]=e[i+1].(BinaryOp) }
        // // }

        // if self.0.len()%2!=1 { return Err(KErr::new("Expression len should always be odd")) }
        // let mut vals : Vec<f32>      = Vec::with_capacity(self.0.len()/2+1);
        // let mut ops  : Vec<BinaryOp> = Vec::with_capacity(self.0.len()/2  );
        // for (i,tok) in self.0.iter().enumerate() {
        //     match tok {
        //         EValue(val) => {
        //             if i%2==1 { return Err(KErr::new("Found value at odd index")) }
        //             match ns.eval_bubble(val) {
        //                 Ok(f) => vals.push(f),
        //                 Err(e) => return Err(e.pre(&format!("eval_bubble({:?})",val))),
        //             }
        //         }
        //         EBinaryOp(bop) => {
        //             if i%2==0 { return Err(KErr::new("Found binaryop at even index")) }
        //             ops.push(*bop);
        //         }
        //     }
        // }

        // Code for new Expression data structure:
        let mut vals = Vec::<f32>::with_capacity(self.pairs.len() + 1);
        let mut ops = Vec::<BinaryOp>::with_capacity(self.pairs.len());
        vals.push(self.first.eval(slab, ns)?);
        for pair in &self.pairs {
            ops.push(pair.0);
            vals.push(pair.1.eval(slab, ns)?);
        }

        // ---- Go code, for comparison ----
        // evalOp:=func(i int) {
        //     result:=ops[i]._Eval(vals[i], vals[i+1])
        //     vals=append(append(vals[:i], result), vals[i+2:]...)
        //     ops=append(ops[:i], ops[i+1:]...)
        // }
        // rtol:=func(s BinaryOp) { for i:=len(ops)-1; i>=0; i-- { if ops[i]==s { evalOp(i) } } }
        // ltor:=func(s BinaryOp) {
        //     loop:
        //     for i:=0; i<len(ops); i++ { if ops[i]==s { evalOp(i); goto loop } }  // Need to restart processing when modifying from the left.
        // }

        // Keep the order of these statements in-sync with parser.rs BinaryOp priority values:
        rtol(&mut vals, &mut ops, EExp); // https://codeplea.com/exponentiation-associativity-options
        ltor(&mut vals, &mut ops, EMod);
        ltor(&mut vals, &mut ops, EDiv);
        rtol(&mut vals, &mut ops, EMul);
        ltor(&mut vals, &mut ops, ESub);
        rtol(&mut vals, &mut ops, EAdd);
        ltor_multi(&mut vals, &mut ops, &[ELT, EGT, ELTE, EGTE, EEQ, ENE]); // TODO: Implement Python-style a<b<c ternary comparison... might as well generalize to N comparisons.
        ltor(&mut vals, &mut ops, EAND);
        ltor(&mut vals, &mut ops, EOR);

        if !ops.is_empty() {
            return Err(Error::Unreachable);
        }
        if vals.len() != 1 {
            return Err(Error::Unreachable);
        }

        vals.first().map_or(Err(Error::Unreachable), |val| Ok(*val))
    }
}

impl Evaler for Value {
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>) {
        match self {
            EConstant(_) => (),
            EUnaryOp(u) => u._var_names(slab, dst),
            EStdFunc(f) => f._var_names(slab, dst),
            EPrintFunc(f) => f._var_names(slab, dst),
        };
    }
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error> {
        match self {
            EConstant(c) => Ok(*c),
            EUnaryOp(u) => u.eval(slab, ns),
            EStdFunc(f) => f.eval(slab, ns),
            EPrintFunc(f) => f.eval(slab, ns),
        }
    }
}

impl Evaler for UnaryOp {
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>) {
        match self {
            EPos(val_i) | ENeg(val_i) | ENot(val_i) => {
                get_val!(slab.ps, val_i)._var_names(slab, dst);
            }
            EParentheses(expr_i) => get_expr!(slab.ps, expr_i)._var_names(slab, dst),
        }
    }
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error> {
        match self {
            EPos(val_i) => get_val!(slab.ps, val_i).eval(slab, ns),
            ENeg(val_i) => Ok(-get_val!(slab.ps, val_i).eval(slab, ns)?),
            ENot(val_i) => Ok(bool_to_f32!(f32_eq!(
                get_val!(slab.ps, val_i).eval(slab, ns)?,
                0.0
            ))),
            EParentheses(expr_i) => get_expr!(slab.ps, expr_i).eval(slab, ns),
        }
    }
}

impl BinaryOp {
    // Non-standard eval interface (not generalized yet):
    fn binaryop_eval(self, left_opt: Option<&f32>, right_opt: Option<&f32>) -> f32 {
        // Passing 'self' by value is more efficient than pass-by-reference.
        let left = match left_opt {
            Some(l) => *l,
            None => return f32::NAN,
        };
        let right = match right_opt {
            Some(r) => *r,
            None => return f32::NAN,
        };
        match self {
            EAdd => left + right, // Floats don't overflow.
            ESub => left - right,
            EMul => left * right,
            EDiv => left / right,
            EMod => left % right, //left - (left/right).trunc()*right
            EExp => left.powf(right),
            ELT => bool_to_f32!(left < right),
            ELTE => bool_to_f32!(left <= right),
            EEQ => bool_to_f32!(f32_eq!(left, right)),
            ENE => bool_to_f32!(f32_ne!(left, right)),
            EGTE => bool_to_f32!(left >= right),
            EGT => bool_to_f32!(left > right),
            EOR => {
                if f32_ne!(left, 0.0) {
                    left
                } else {
                    right
                }
            }
            EAND => {
                if f32_eq!(left, 0.0) {
                    left
                } else {
                    right
                }
            }
        }
    }
}

#[macro_export]
macro_rules! eval_var {
    ($ns:ident, $name:ident, $args:expr, $keybuf:expr) => {
        match $ns.lookup($name, $args, $keybuf) {
            Some(f) => Ok(f),
            None => Err(Error::Undefined($name.to_string())),
        }
    };
}

impl Evaler for StdFunc {
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>) {
        match self {
            #[cfg(feature = "unsafe-vars")]
            EUnsafeVar { name, .. } => {
                dst.insert(name.clone());
            }

            EVar(s) => {
                dst.insert(s.clone());
            }
            EFunc { name, args } => {
                dst.insert(name.clone());
                for arg in args {
                    get_expr!(slab.ps, arg)._var_names(slab, dst);
                }
            }

            EFuncInt(xi) | EFuncCeil(xi) | EFuncFloor(xi) | EFuncAbs(xi) | EFuncSign(xi)
            | EFuncSin(xi) | EFuncCos(xi) | EFuncTan(xi) | EFuncASin(xi) | EFuncACos(xi)
            | EFuncATan(xi) | EFuncSinH(xi) | EFuncCosH(xi) | EFuncTanH(xi) | EFuncASinH(xi)
            | EFuncACosH(xi) | EFuncATanH(xi) => get_expr!(slab.ps, xi)._var_names(slab, dst),

            EFuncE | EFuncPi => (),
            EFuncLog { base: opt, expr } | EFuncRound { modulus: opt, expr } => {
                if let Some(xi) = opt.as_ref() {
                    get_expr!(slab.ps, xi)._var_names(slab, dst)
                }
                get_expr!(slab.ps, expr)._var_names(slab, dst);
            }
            EFuncMin { first, rest } | EFuncMax { first, rest } => {
                get_expr!(slab.ps, first)._var_names(slab, dst);
                for xi in rest {
                    get_expr!(slab.ps, xi)._var_names(slab, dst);
                }
            }
        };
    }

    #[allow(clippy::cognitive_complexity)]
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error> {
        let celled_slab = RefCell::from(slab.ps.char_buf.clone());
        match self {
            // These match arms are ordered in a way that I feel should deliver good performance.
            // (I don't think this ordering actually affects the generated code, though.)
            #[cfg(feature = "unsafe-vars")]
            EUnsafeVar { ptr, .. } => unsafe { Ok(**ptr) },

            EVar(name) => eval_var!(ns, name, Vec::new(), &mut *celled_slab.borrow_mut()),
            EFunc { name, args: xis } => {
                let mut args = Vec::with_capacity(xis.len());
                for xi in xis {
                    args.push(get_expr!(slab.ps, xi).eval(slab, ns)?);
                }
                eval_var!(ns, name, args, &mut *celled_slab.borrow_mut())
            }

            EFuncLog {
                base: base_opt,
                expr: expr_i,
            } => {
                let base = match base_opt {
                    Some(b_expr_i) => get_expr!(slab.ps, b_expr_i).eval(slab, ns)?,
                    None => 10.0,
                };
                let n = get_expr!(slab.ps, expr_i).eval(slab, ns)?;
                Ok(log(base, n))
            }

            EFuncSin(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.sin()),
            EFuncCos(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.cos()),
            EFuncTan(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.tan()),
            EFuncASin(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.asin()),
            EFuncACos(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.acos()),
            EFuncATan(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.atan()),
            EFuncSinH(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.sinh()),
            EFuncCosH(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.cosh()),
            EFuncTanH(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.tanh()),
            EFuncASinH(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.asinh()),
            EFuncACosH(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.acosh()),
            EFuncATanH(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.atanh()),

            EFuncRound {
                modulus: modulus_opt,
                expr: expr_i,
            } => {
                let modulus = match modulus_opt {
                    Some(m_expr_i) => get_expr!(slab.ps, m_expr_i).eval(slab, ns)?,
                    None => 1.0,
                };
                Ok((get_expr!(slab.ps, expr_i).eval(slab, ns)? / modulus).round() * modulus)
            }

            EFuncAbs(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.abs()),
            EFuncSign(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.signum()),
            EFuncInt(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.trunc()),
            EFuncCeil(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.ceil()),
            EFuncFloor(expr_i) => Ok(get_expr!(slab.ps, expr_i).eval(slab, ns)?.floor()),
            EFuncMin {
                first: first_i,
                rest,
            } => {
                let mut min = get_expr!(slab.ps, first_i).eval(slab, ns)?;
                let mut saw_nan = min.is_nan();
                for x_i in rest {
                    min = min.min(get_expr!(slab.ps, x_i).eval(slab, ns)?);
                    saw_nan = saw_nan || min.is_nan();
                }
                if saw_nan {
                    Ok(std::f32::NAN)
                } else {
                    Ok(min)
                }
            }
            EFuncMax {
                first: first_i,
                rest,
            } => {
                let mut max = get_expr!(slab.ps, first_i).eval(slab, ns)?;
                let mut saw_nan = max.is_nan();
                for x_i in rest {
                    max = max.max(get_expr!(slab.ps, x_i).eval(slab, ns)?);
                    saw_nan = saw_nan || max.is_nan();
                }
                if saw_nan {
                    Ok(f32::NAN)
                } else {
                    Ok(max)
                }
            }

            EFuncE => Ok(consts::E),
            EFuncPi => Ok(consts::PI),
        }
    }
}

impl Evaler for PrintFunc {
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>) {
        for x_or_s in &self.0 {
            match x_or_s {
                EExpr(xi) => get_expr!(slab.ps, xi)._var_names(slab, dst),
                EStr(_) => (),
            };
        }
    }
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error> {
        fn process_str(s: &str) -> String {
            s.replace("\\n", "\n").replace("\\t", "\t")
        }

        let mut val = 0f32;

        if let Some(EStr(fmtstr)) = self.0.first() {
            if fmtstr.contains('%') {
                // printf mode:

                //let fmtstr = process_str(fmtstr);

                return Err(Error::WrongArgs(String::from(
                    "printf formatting is not yet implemented",
                ))); // TODO: Make a pure-rust sprintf libarary.

                //return Ok(val);
            }
        }

        // Normal Mode:
        let mut out = String::with_capacity(16);
        for (i, a) in self.0.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            match a {
                EExpr(e_i) => {
                    val = get_expr!(slab.ps, e_i).eval(slab, ns)?;
                    out.push_str(&val.to_string());
                }
                EStr(s) => out.push_str(&process_str(s)),
            }
        }
        eprintln!("{out}");

        Ok(val)
    }
}

impl Evaler for Instruction {
    fn _var_names(&self, slab: &Slab, dst: &mut BTreeSet<String>) {
        match self {
            #[cfg(feature = "unsafe-vars")]
            IUnsafeVar { name, .. } => {
                dst.insert(name.clone());
            }

            IVar(s) => {
                dst.insert(s.clone());
            }
            IFunc { name, args } => {
                dst.insert(name.clone());
                for ic in args {
                    let iconst: Self;
                    ic_to_instr!(slab.cs, iconst, ic)._var_names(slab, dst);
                }
            }

            IConst(_) => (),

            INeg(ii) | INot(ii) | IInv(ii) | IFuncInt(ii) | IFuncCeil(ii) | IFuncFloor(ii)
            | IFuncAbs(ii) | IFuncSign(ii) | IFuncSin(ii) | IFuncCos(ii) | IFuncTan(ii)
            | IFuncASin(ii) | IFuncACos(ii) | IFuncATan(ii) | IFuncSinH(ii) | IFuncCosH(ii)
            | IFuncTanH(ii) | IFuncASinH(ii) | IFuncACosH(ii) | IFuncATanH(ii) => {
                get_instr!(slab.cs, ii)._var_names(slab, dst);
            }

            ILT(left_ic, right_ic)
            | ILTE(left_ic, right_ic)
            | IEQ(left_ic, right_ic)
            | INE(left_ic, right_ic)
            | IGTE(left_ic, right_ic)
            | IGT(left_ic, right_ic)
            | IMod {
                dividend: left_ic,
                divisor: right_ic,
            }
            | IExp {
                base: left_ic,
                power: right_ic,
            }
            | IFuncLog {
                base: left_ic,
                of: right_ic,
            }
            | IFuncRound {
                modulus: left_ic,
                of: right_ic,
            } => {
                let mut iconst: Self;
                ic_to_instr!(slab.cs, iconst, left_ic)._var_names(slab, dst);
                ic_to_instr!(slab.cs, iconst, right_ic)._var_names(slab, dst);
            }

            IAdd(li, ric)
            | IMul(li, ric)
            | IOR(li, ric)
            | IAND(li, ric)
            | IFuncMin(li, ric)
            | IFuncMax(li, ric) => {
                get_instr!(slab.cs, li)._var_names(slab, dst);
                let iconst: Self;
                ic_to_instr!(slab.cs, iconst, ric)._var_names(slab, dst);
            }

            IPrintFunc(pf) => pf._var_names(slab, dst),
        }
    }

    #[allow(clippy::too_many_lines, clippy::cognitive_complexity)] // This is pretty simple on its own.
    fn eval(&self, slab: &Slab, ns: &mut impl EvalNamespace) -> Result<f32, Error> {
        let celled_slab = RefCell::from(slab.ps.char_buf.clone());
        match self {
            // I have manually ordered these match arms in a way that I feel should deliver good performance.
            // (I don't think this ordering actually affects the generated code, though.)
            IMul(li, ric) => {
                Ok(eval_compiled_ref!(get_instr!(slab.cs, li), slab, ns)
                    * eval_ic_ref!(ric, slab, ns))
            }
            IAdd(li, ric) => {
                Ok(eval_compiled_ref!(get_instr!(slab.cs, li), slab, ns)
                    + eval_ic_ref!(ric, slab, ns))
            }
            IExp { base, power } => {
                Ok(eval_ic_ref!(base, slab, ns).powf(eval_ic_ref!(power, slab, ns)))
            }

            INeg(i) => Ok(-eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns)),
            IInv(i) => Ok(1.0 / eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns)),

            IVar(name) => eval_var!(ns, name, Vec::new(), &mut celled_slab.borrow_mut()),
            IFunc { name, args: ics } => {
                let mut args = Vec::with_capacity(ics.len());
                for ic in ics {
                    args.push(eval_ic_ref!(ic, slab, ns));
                }
                eval_var!(ns, name, args, &mut celled_slab.borrow_mut())
            }

            IFuncLog {
                base: baseic,
                of: ofic,
            } => {
                let base = eval_ic_ref!(baseic, slab, ns);
                let of = eval_ic_ref!(ofic, slab, ns);
                Ok(log(base, of))
            }

            IFuncSin(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).sin()),
            IFuncCos(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).cos()),
            IFuncTan(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).tan()),
            IFuncASin(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).asin()),
            IFuncACos(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).acos()),
            IFuncATan(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).atan()),
            IFuncSinH(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).sinh()),
            IFuncCosH(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).cosh()),
            IFuncTanH(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).tanh()),
            IFuncASinH(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).asinh()),
            IFuncACosH(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).acosh()),
            IFuncATanH(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).atanh()),

            IFuncRound {
                modulus: modic,
                of: ofic,
            } => {
                let modulus = eval_ic_ref!(modic, slab, ns);
                let of = eval_ic_ref!(ofic, slab, ns);
                Ok((of / modulus).round() * modulus)
            }
            IMod { dividend, divisor } => {
                Ok(eval_ic_ref!(dividend, slab, ns) % eval_ic_ref!(divisor, slab, ns))
            }

            IFuncAbs(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).abs()),
            IFuncSign(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).signum()),
            IFuncInt(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).trunc()),
            IFuncCeil(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).ceil()),
            IFuncFloor(i) => Ok(eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns).floor()),
            IFuncMin(li, ric) => {
                let left = eval_compiled_ref!(get_instr!(slab.cs, li), slab, ns);
                let right = eval_ic_ref!(ric, slab, ns);
                if left.is_nan() || right.is_nan() {
                    return Ok(std::f32::NAN);
                } // I need to implement NAN checks myself because the f32.min() function says that if one number is NaN, the other will be returned.
                if left < right {
                    Ok(left)
                } else {
                    Ok(right)
                }
            }
            IFuncMax(li, ric) => {
                let left = eval_compiled_ref!(get_instr!(slab.cs, li), slab, ns);
                let right = eval_ic_ref!(ric, slab, ns);
                if left.is_nan() || right.is_nan() {
                    return Ok(std::f32::NAN);
                }
                if left > right {
                    Ok(left)
                } else {
                    Ok(right)
                }
            }

            IEQ(left, right) => Ok(bool_to_f32!(f32_eq!(
                eval_ic_ref!(left, slab, ns),
                eval_ic_ref!(right, slab, ns)
            ))),
            INE(left, right) => Ok(bool_to_f32!(f32_ne!(
                eval_ic_ref!(left, slab, ns),
                eval_ic_ref!(right, slab, ns)
            ))),
            ILT(left, right) => Ok(bool_to_f32!(
                eval_ic_ref!(left, slab, ns) < eval_ic_ref!(right, slab, ns)
            )),
            ILTE(left, right) => Ok(bool_to_f32!(
                eval_ic_ref!(left, slab, ns) <= eval_ic_ref!(right, slab, ns)
            )),
            IGTE(left, right) => Ok(bool_to_f32!(
                eval_ic_ref!(left, slab, ns) >= eval_ic_ref!(right, slab, ns)
            )),
            IGT(left, right) => Ok(bool_to_f32!(
                eval_ic_ref!(left, slab, ns) > eval_ic_ref!(right, slab, ns)
            )),

            INot(i) => Ok(bool_to_f32!(f32_eq!(
                eval_compiled_ref!(get_instr!(slab.cs, i), slab, ns),
                0.0
            ))),
            IAND(lefti, rightic) => {
                let left = eval_compiled_ref!(get_instr!(slab.cs, lefti), slab, ns);
                if f32_eq!(left, 0.0) {
                    Ok(left)
                } else {
                    Ok(eval_ic_ref!(rightic, slab, ns))
                }
            }
            IOR(lefti, rightic) => {
                let left = eval_compiled_ref!(get_instr!(slab.cs, lefti), slab, ns);
                if f32_ne!(left, 0.0) {
                    Ok(left)
                } else {
                    Ok(eval_ic_ref!(rightic, slab, ns))
                }
            }

            IPrintFunc(pf) => pf.eval(slab, ns),

            // Put these last because you should be using the eval_compiled*!() macros to eliminate function calls.
            IConst(c) => Ok(*c),
            #[cfg(feature = "unsafe-vars")]
            IUnsafeVar { ptr, .. } => unsafe { Ok(**ptr) },
        }
    }
}
