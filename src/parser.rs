//! This module parses string expressions into an AST which can then be compiled or evaluated.
//!
//! # fasteval3 Algebra Grammar
//! ```text
//! Expression: Value (BinaryOp Value)*
//!
//! Value: Constant || UnaryOp || PrintFunc || StdFunc
//!
//! Constant: [+-]?[0-9]*(\.[0-9]+)?( ([eE][+-]?[0-9]+) || [pnuµmkKMGT] )?  || [+-]?(NaN || inf)
//!
//! UnaryOp: +Value || -Value || (Expression) || [Expression] || !Value
//!
//! BinaryOp: + || - || * || / || % || ^ || < || <= || == || != || >= || > || (or || '||') || (and || '&&')
//!
//! VarName: [a-zA-Z_][a-zA-Z_0-9]*
//!
//! StdFunc: VarName((Expression,)*)?  ||  VarName[(Expression,)*]?
//!
//! PrintFunc: print(ExpressionOrString,*)
//!
//! ExpressionOrString: Expression || String
//!
//! String: ".*"
//! ```

use crate::error::Error;
use crate::slab::ParseSlab;

use std::ptr;
use std::str::{from_utf8, from_utf8_unchecked};

/// An `ExpressionI` represents an index into `Slab.ps.exprs`.
///
/// It behaves much like a pointer or reference, but it is 'safe' (unlike a raw
/// pointer) and is not managed by the Rust borrow checker (unlike a reference).
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct ExpressionI(pub usize);

/// A `ValueI` represents an index into `Slab.ps.vals`.
///
/// It behaves much like a pointer or reference, but it is 'safe' (unlike a raw
/// pointer) and is not managed by the Rust borrow checker (unlike a reference).
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct ValueI(pub usize);

/// An `Expression` is the top node of a parsed AST.
///
/// It can be `compile()`d or `eval()`d.
#[derive(Debug, PartialEq, Default)]
pub struct Expression {
    pub(crate) first: Value,
    pub(crate) pairs: Vec<ExprPair>, // cap=8
}

#[derive(Debug, PartialEq)]
pub(crate) struct ExprPair(pub(crate) BinaryOp, pub(crate) Value);

/// A `Value` can be a Constant, a `UnaryOp`, a `StdFunc`, or a `PrintFunc`.
#[derive(Debug, PartialEq)]
pub enum Value {
    EConstant(f32),
    EUnaryOp(UnaryOp),
    EStdFunc(StdFunc),
    EPrintFunc(PrintFunc),
}
use self::Value::{EConstant, EPrintFunc, EStdFunc, EUnaryOp};

/// Unary Operators
#[derive(Debug, PartialEq, Eq)]
pub enum UnaryOp {
    EPos(ValueI),
    ENeg(ValueI),
    ENot(ValueI),
    EParentheses(ExpressionI),
}
use self::UnaryOp::{ENeg, ENot, EParentheses, EPos};

/// Binary Operators
#[derive(Debug, PartialEq, Eq, PartialOrd, Copy, Clone)]
pub enum BinaryOp {
    // Sorted in order of precedence (low-priority to high-priority):
    // Keep this order in-sync with evaler.rs.  (Search for 'rtol' and 'ltor'.)
    EOR = 1, // Lowest Priority
    EAND = 2,
    ENE = 3,
    EEQ = 4,
    EGTE = 5,
    ELTE = 6,
    EGT = 7,
    ELT = 8,
    EAdd = 9,
    ESub = 10,
    EMul = 11,
    EDiv = 12,
    EMod = 13,
    EExp = 14, // Highest Priority
}
use self::BinaryOp::{
    EAdd, EDiv, EExp, EMod, EMul, ESub, EAND, EEQ, EGT, EGTE, ELT, ELTE, ENE, EOR,
};

/// A Function Call with Standard Syntax.
#[derive(Debug, PartialEq, Eq)]
pub enum StdFunc {
    EVar(String),
    #[cfg(feature = "unsafe-vars")]
    EUnsafeVar {
        name: String,
        ptr: *const f32,
    },
    EFunc {
        name: String,
        args: Vec<ExpressionI>,
    }, // cap=4

    EFuncInt(ExpressionI),
    EFuncCeil(ExpressionI),
    EFuncFloor(ExpressionI),
    EFuncAbs(ExpressionI),
    EFuncSign(ExpressionI),
    EFuncLog {
        base: Option<ExpressionI>,
        expr: ExpressionI,
    },
    EFuncRound {
        modulus: Option<ExpressionI>,
        expr: ExpressionI,
    },
    EFuncMin {
        first: ExpressionI,
        rest: Vec<ExpressionI>,
    }, // cap=4
    EFuncMax {
        first: ExpressionI,
        rest: Vec<ExpressionI>,
    }, // cap=4

    EFuncE,
    EFuncPi,

    EFuncSin(ExpressionI),
    EFuncCos(ExpressionI),
    EFuncTan(ExpressionI),
    EFuncASin(ExpressionI),
    EFuncACos(ExpressionI),
    EFuncATan(ExpressionI),
    EFuncSinH(ExpressionI),
    EFuncCosH(ExpressionI),
    EFuncTanH(ExpressionI),
    EFuncASinH(ExpressionI),
    EFuncACosH(ExpressionI),
    EFuncATanH(ExpressionI),
}
#[cfg(feature = "unsafe-vars")]
use StdFunc::EUnsafeVar;
use StdFunc::{
    EFunc, EFuncACos, EFuncACosH, EFuncASin, EFuncASinH, EFuncATan, EFuncATanH, EFuncAbs,
    EFuncCeil, EFuncCos, EFuncCosH, EFuncE, EFuncFloor, EFuncInt, EFuncLog, EFuncMax, EFuncMin,
    EFuncPi, EFuncRound, EFuncSign, EFuncSin, EFuncSinH, EFuncTan, EFuncTanH, EVar,
};

/// Represents a `print()` function call in the `fasteval3` expression AST.
#[derive(Debug, PartialEq, Eq)]
pub struct PrintFunc(pub Vec<ExpressionOrString>); // cap=8

/// Used by the `print()` function.  Can hold an `Expression` or a `String`.
#[derive(Debug, PartialEq, Eq)]
pub enum ExpressionOrString {
    EExpr(ExpressionI),
    EStr(String), // cap=64
}
use ExpressionOrString::{EExpr, EStr};

impl Clone for PrintFunc {
    fn clone(&self) -> Self {
        let mut vec = Vec::<ExpressionOrString>::with_capacity(self.0.len());
        for x_or_s in &self.0 {
            vec.push(match x_or_s {
                EExpr(i) => EExpr(*i),
                EStr(s) => EStr(s.clone()),
            });
        }
        Self(vec)
    }
}

enum Token<T> {
    Pass,
    Bite(T),
}
use Token::{Bite, Pass};

macro_rules! peek {
    ($bs:ident) => {
        $bs.first().copied()
    };
}
macro_rules! peek_n {
    ($bs:ident, $skip:literal) => {
        $bs.get($skip).copied()
    };
    ($bs:ident, $skip:ident) => {
        $bs.get($skip).copied()
    };
    ($bs:ident, $skip:expr) => {
        $bs.get($skip).copied()
    };
}
macro_rules! peek_is {
    ($bs:ident, $skip:literal, $val:literal) => {
        peek_n!($bs, $skip) == Some($val)
    };
    ($bs:ident, $skip:expr, $val:literal) => {
        peek_n!($bs, $skip) == Some($val)
    };
}

macro_rules! read {
    ($bs:ident) => {
        match $bs.first() {
            Some(b) => {
                *$bs = &$bs[1..];
                Ok(*b)
            }
            None => Err(Error::EOF),
        }
    };
    ($bs:ident, $parsing:literal) => {
        match $bs.first() {
            Some(b) => {
                *$bs = &$bs[1..];
                Ok(*b)
            }
            None => Err(Error::EofWhileParsing($parsing.to_owned())),
        }
    };
}

macro_rules! skip {
    ($bs:ident) => {
        *$bs = &$bs[1..];
    };
}
macro_rules! skip_n {
    ($bs:ident, $n:literal) => {
        *$bs = &$bs[$n..];
    };
    ($bs:ident, $n:ident) => {
        *$bs = &$bs[$n..];
    };
}

macro_rules! is_space {
    ($b:ident) => {
        if $b > b' ' {
            false
        } else {
            $b == b' ' || $b == b'\n' || $b == b'\t' || $b == b'\r'
        }
    };
}
macro_rules! spaces {
    ($bs:ident) => {
        while let Some(b) = peek!($bs) {
            if !is_space!(b) { break }
            skip!($bs);  // We normally don't have long strings of whitespace, so it is more efficient to put this single-skip inside this loop rather than a skip_n afterwards.
        }
    };
}

pub const DEFAULT_EXPR_LEN_LIMIT: usize = 4096;
pub const DEFAULT_EXPR_DEPTH_LIMIT: usize = 32;

pub struct Parser {
    pub expr_len_limit: usize,
    pub expr_depth_limit: usize,
}

impl Parser {
    #[inline]
    pub const fn new() -> Self {
        Self {
            expr_len_limit: DEFAULT_EXPR_LEN_LIMIT,
            expr_depth_limit: DEFAULT_EXPR_DEPTH_LIMIT,
        }
    }

    /// Checks if a given byte matches its character counterpart.
    const fn is_varname_byte(b: u8, i: usize) -> bool {
        // Might be parser-breaking
        /*(b'A' <= b && b <= b'Z')
        || (b'a' <= b && b <= b'z')
        || b == b'_'
        || (i > 0 && (b'0' <= b && b <= b'9'))*/

        b.is_ascii_uppercase()
            || b.is_ascii_lowercase()
            || b == b'_'
            || (i > 0 && b.is_ascii_digit())
    }

    /// Checks if a given bytes matches its character counterpart, with the byte itself possibly being none.
    fn is_varname_byte_opt(bo: Option<u8>, i: usize) -> bool {
        bo.map_or(false, |byte| Self::is_varname_byte(byte, i))
    }

    /// Use this function to parse an expression String.  The `Slab` will be cleared first.
    ///
    /// # Errors
    ///
    /// Will return `Err` if length of `expr_str` exceeds limit.
    #[inline]
    pub fn parse(&self, expr_str: &str, slab: &mut ParseSlab) -> Result<ExpressionI, Error> {
        slab.clear();
        self.parse_noclear(expr_str, slab)
    }

    /// This is exactly the same as `parse()` but the `Slab` will NOT be cleared.
    /// This is useful in performance-critical sections, when you know that you
    /// already have an empty `Slab`.
    ///
    /// This function cannot return Result<&Expression> because it would
    /// prolong the mut ref.  / That's why we return an `ExpressionI` instead.
    ///
    /// # Errors
    ///
    /// Will return `Err` if length of `expr_str` exceeds limit.
    #[inline]
    pub fn parse_noclear(
        &self,
        expr_str: &str,
        slab: &mut ParseSlab,
    ) -> Result<ExpressionI, Error> {
        if expr_str.len() > self.expr_len_limit {
            return Err(Error::TooLong);
        } // Restrict length for safety
        let mut bs = expr_str.as_bytes();
        self.read_expression(slab, &mut bs, 0, true)
    }

    fn read_expression(
        &self,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
        expect_eof: bool,
    ) -> Result<ExpressionI, Error> {
        if depth > self.expr_depth_limit {
            return Err(Error::TooDeep);
        }

        let first = self.read_value(slab, bs, depth)?;
        let mut pairs = Vec::<ExprPair>::with_capacity(8);
        loop {
            match Self::read_binaryop(bs)? {
                Pass => break,
                Bite(bop) => {
                    let val = self.read_value(slab, bs, depth)?;
                    pairs.push(ExprPair(bop, val));
                }
            }
        }
        spaces!(bs);
        if expect_eof && !bs.is_empty() {
            let bs_str = match from_utf8(bs) {
                Ok(s) => s,
                Err(..) => "Utf8Error while handling UnparsedTokensRemaining error",
            };
            return Err(Error::UnparsedTokensRemaining(bs_str.to_owned()));
        }
        slab.push_expr(Expression { first, pairs })
    }

    fn read_value(
        &self,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
    ) -> Result<Value, Error> {
        if depth > self.expr_depth_limit {
            return Err(Error::TooDeep);
        }

        match Self::read_const(slab, bs)? {
            Pass => {}
            Bite(c) => return Ok(EConstant(c)),
        }
        match self.read_unaryop(slab, bs, depth)? {
            Pass => {}
            Bite(u) => return Ok(EUnaryOp(u)),
        }
        match self.read_callable(slab, bs, depth)? {
            Pass => {}
            Bite(c) => return Ok(c),
        }

        // Improve the precision of this error case:
        if bs.is_empty() {
            return Err(Error::EofWhileParsing(String::from("value")));
        }

        Err(Error::InvalidValue)
    }

    fn read_const(slab: &mut ParseSlab, bs: &mut &[u8]) -> Result<Token<f32>, Error> {
        spaces!(bs);

        let mut toklen = 0;
        let mut sign_ok = true;
        let mut specials_ok = true;
        let mut suffix_ok = true;
        let mut saw_val = false;
        loop {
            match peek_n!(bs, toklen) {
                None => break,
                Some(b) => {
                    if b.is_ascii_digit() || b == b'.' {
                        saw_val = true;
                        sign_ok = false;
                        specials_ok = false;
                        toklen += 1;
                    } else if sign_ok && (b == b'-' || b == b'+') {
                        sign_ok = false;
                        toklen += 1;
                    } else if saw_val && (b == b'e' || b == b'E') {
                        suffix_ok = false;
                        sign_ok = true;
                        toklen += 1;
                    } else if specials_ok
                        && (b == b'N'
                            && peek_is!(bs, toklen + 1, b'a')
                            && peek_is!(bs, toklen + 2, b'N')
                            || b == b'i'
                                && peek_is!(bs, toklen + 1, b'n')
                                && peek_is!(bs, toklen + 2, b'f'))
                    {
                        #[cfg(feature = "alpha-keywords")]
                        {
                            saw_val = true;
                            suffix_ok = false;
                            toklen += 3;
                        }
                        break;
                    } else {
                        break;
                    }
                }
            }
        }

        if !saw_val {
            return Ok(Pass);
        }

        let mut tok = unsafe { from_utf8_unchecked(&bs[..toklen]) };
        if suffix_ok {
            match peek_n!(bs, toklen) {
                None => (),
                Some(b) => {
                    let (exp, suffixlen) = match b {
                        b'k' | b'K' => (3, 1),
                        b'M' => (6, 1),
                        b'G' => (9, 1),
                        b'T' => (12, 1),
                        b'm' => (-3, 1),
                        b'u' | b'\xb5' => (-6, 1), // ASCII-encoded 'µ'
                        b'\xc2' if peek_is!(bs, toklen + 1, b'\xb5') => (-6, 2), // UTF8-encoded 'µ'
                        b'n' => (-9, 1),
                        b'p' => (-12, 1),
                        _ => (0, 0),
                    };
                    if exp != 0 {
                        slab.char_buf.clear();
                        slab.char_buf.push_str(tok);
                        slab.char_buf.push('e');
                        slab.char_buf.push_str(&exp.to_string());
                        tok = &slab.char_buf;

                        toklen += suffixlen;
                    }
                }
            }
        }

        let val = tok
            .parse::<f32>()
            .map_err(|_| Error::ParseF32(tok.to_owned()))?;
        skip_n!(bs, toklen);

        Ok(Bite(val))
    }

    // // This implementation is beautiful and correct, but it is slow due to the fact that I am first parsing everything,
    // // and then I'm calling parse::<f32> which repeats the entire process.
    // // I wish I could just call dec2flt::convert() ( https://doc.rust-lang.org/src/core/num/dec2flt/mod.rs.html#247 )
    // // with all the pieces I already parsed, but, alas, that function is private.
    // //
    // // Also, I have decided that I really do need to support 'NaN' and 'inf'.  I could add them below, but instead,
    // // I think I will just switch back to a drastically-simplified parser which isn't as "correct", but re-uses
    // // more functionality from the stdlib.
    // //
    // // As a side-note, It's surprising how similar these algorithms are (which I created from scratch at 3am with no reference),
    // // compared to the dec2flt::parse module.
    // fn read_const(&mut self, bs:&mut &[u8]) -> Result<Token<f32>, KErr> {
    //     spaces!(bs);
    //
    //     // Grammar: [+-]?[0-9]*(\.[0-9]+)?( ([eE][+-]?[0-9]+) || [pnuµmkKMGT] )?
    //     fn peek_digits(bs:&[u8]) -> usize {
    //         let mut i = 0;
    //         while i<bs.len() && b'0'<=bs[i] && bs[i]<=b'9' { i+=1; }
    //         i
    //     }
    //     fn peek_exp(bs:&[u8]) -> Result<usize, KErr> {
    //         if bs.is_empty() { return Err(KErr::new("peek_exp empty")); }
    //         let mut i = 0;
    //         if bs[i]==b'-' || bs[i]==b'+' { i+=1; }
    //         let digits = peek_digits(&bs[i..]);
    //         if digits==0 { return Err(KErr::new("peek_exp no digits")); }
    //         Ok(i+digits)
    //     }
    //     fn peek_tail(bs:&[u8]) -> Result<(/*read:*/usize, /*skip:*/usize, /*exp:*/i32), KErr> {
    //         if bs.is_empty() { return Ok((0,0,0)); }
    //         match bs[0] {
    //             b'k' | b'K' => Ok((0,1,3)),
    //             b'M' => Ok((0,1,6)),
    //             b'G' => Ok((0,1,9)),
    //             b'T' => Ok((0,1,12)),
    //             b'm' => Ok((0,1,-3)),
    //             b'u' | b'\xb5' => Ok((0,1,-6)),  // ASCII-encoded 'µ'
    //             b'\xc2' if bs.len()>1 && bs[1]==b'\xb5' => Ok((0,2,-6)),  // UTF8-encoded 'µ'
    //             b'n' => Ok((0,1,-9)),
    //             b'p' => Ok((0,1,-12)),
    //             b'e' | b'E' => peek_exp(&bs[1..]).map(|size| (1+size,0,0)),
    //             _ => Ok((0,0,0)),
    //         }
    //     }
    //
    //     let mut toread=0;  let mut toskip=0;  let mut exp=0;
    //
    //     match peek(bs, 0) {
    //         None => return Ok(Pass),
    //         Some(b) => {
    //             if b==b'-' || b==b'+' { toread+=1; }
    //         }
    //
    //     }
    //
    //     let predec = peek_digits(&bs[toread..]);
    //     toread+=predec;
    //
    //     match peek(bs, toread) {
    //         None => {
    //             if predec==0 { return Ok(Pass); }
    //         }
    //         Some(b) => {
    //             if b==b'.' {
    //                 toread+=1;
    //                 let postdec = peek_digits(&bs[toread..]);
    //                 if predec==0 && postdec==0 { return Err(KErr::new("decimal without pre- or post-digits")); }
    //                 toread+=postdec;
    //             } else {
    //                 if predec==0 { return Ok(Pass); }
    //             }
    //             let (rd,sk,ex) = peek_tail(&bs[toread..])?;
    //             toread+=rd;  toskip=sk;  exp=ex;
    //         }
    //     }
    //
    //     self.char_buf.clear();
    //     for _ in 0..toread { self.char_buf.push(read(bs)? as char); }
    //     for _ in 0..toskip { read(bs)?; }
    //     if exp!=0 { self.char_buf.push('e'); self.char_buf.push_str(&exp.to_string()); }
    //
    //     let val = self.char_buf.parse::<f32>().map_err(|_| {
    //         KErr::new("parse<f32> error").pre(&self.char_buf)
    //     })?;
    //     Ok(Bite(val))
    // }

    fn read_unaryop(
        &self,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
    ) -> Result<Token<UnaryOp>, Error> {
        spaces!(bs);
        match peek!(bs) {
            None => Ok(Pass), // Err(KErr::new("EOF at UnaryOp position")), -- Instead of erroring, let the higher level decide what to do.
            Some(b) => match b {
                b'+' => {
                    skip!(bs);
                    let v = self.read_value(slab, bs, depth + 1)?;
                    Ok(Bite(EPos(slab.push_val(v)?)))
                }
                b'-' => {
                    skip!(bs);
                    let v = self.read_value(slab, bs, depth + 1)?;
                    Ok(Bite(ENeg(slab.push_val(v)?)))
                }
                b'(' => {
                    skip!(bs);
                    let xi = self.read_expression(slab, bs, depth + 1, false)?;
                    spaces!(bs);
                    if read!(bs, "parentheses")? != b')' {
                        return Err(Error::Expected(String::from(")")));
                    }
                    Ok(Bite(EParentheses(xi)))
                }
                b'[' => {
                    skip!(bs);
                    let xi = self.read_expression(slab, bs, depth + 1, false)?;
                    spaces!(bs);
                    if read!(bs, "square brackets")? != b']' {
                        return Err(Error::Expected(String::from("]")));
                    }
                    Ok(Bite(EParentheses(xi)))
                }
                b'!' => {
                    skip!(bs);
                    let v = self.read_value(slab, bs, depth + 1)?;
                    Ok(Bite(ENot(slab.push_val(v)?)))
                }
                _ => Ok(Pass),
            },
        }
    }

    fn read_binaryop(bs: &mut &[u8]) -> Result<Token<BinaryOp>, Error> {
        spaces!(bs);
        peek!(bs).map_or(Ok(Pass), |b| match b {
            b'+' => {
                skip!(bs);
                Ok(Bite(EAdd))
            }
            b'-' => {
                skip!(bs);
                Ok(Bite(ESub))
            }
            b'*' => {
                skip!(bs);
                Ok(Bite(EMul))
            }
            b'/' => {
                skip!(bs);
                Ok(Bite(EDiv))
            }
            b'%' => {
                skip!(bs);
                Ok(Bite(EMod))
            }
            b'^' => {
                skip!(bs);
                Ok(Bite(EExp))
            }
            b'<' => {
                skip!(bs);
                if peek_is!(bs, 0, b'=') {
                    skip!(bs);
                    Ok(Bite(ELTE))
                } else {
                    Ok(Bite(ELT))
                }
            }
            b'>' => {
                skip!(bs);
                if peek_is!(bs, 0, b'=') {
                    skip!(bs);
                    Ok(Bite(EGTE))
                } else {
                    Ok(Bite(EGT))
                }
            }
            b'=' if peek_is!(bs, 1, b'=') => {
                skip_n!(bs, 2);
                Ok(Bite(EEQ))
            }
            b'!' if peek_is!(bs, 1, b'=') => {
                skip_n!(bs, 2);
                Ok(Bite(ENE))
            }
            #[cfg(feature = "alpha-keywords")]
            b'o' if peek_is!(bs, 1, b'r') => {
                skip_n!(bs, 2);
                Ok(Bite(EOR))
            }
            b'|' if peek_is!(bs, 1, b'|') => {
                skip_n!(bs, 2);
                Ok(Bite(EOR))
            }
            #[cfg(feature = "alpha-keywords")]
            b'a' if peek_is!(bs, 1, b'n') && peek_is!(bs, 2, b'd') => {
                skip_n!(bs, 3);
                Ok(Bite(EAND))
            }
            b'&' if peek_is!(bs, 1, b'&') => {
                skip_n!(bs, 2);
                Ok(Bite(EAND))
            }
            _ => Ok(Pass),
        })
    }

    fn read_callable(
        &self,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
    ) -> Result<Token<Value>, Error> {
        match Self::read_varname(bs)? {
            Pass => Ok(Pass),
            Bite(varname) => {
                match Self::read_open_parenthesis(bs)? {
                    Pass => {
                        // VarNames without Parenthesis are always treated as custom 0-arg functions.

                        #[cfg(feature = "unsafe-vars")]
                        match slab.unsafe_vars.get(&varname) {
                            None => Ok(Bite(EStdFunc(EVar(varname)))),
                            Some(&ptr) => Ok(Bite(EStdFunc(EUnsafeVar { name: varname, ptr }))),
                        }

                        #[cfg(not(feature = "unsafe-vars"))]
                        Ok(Bite(EStdFunc(EVar(varname))))
                    }
                    Bite(open_parenth) => {
                        // VarNames with Parenthesis are first matched against builtins, then custom.
                        match varname.as_ref() {
                            "print" => Ok(Bite(EPrintFunc(self.read_printfunc(
                                slab,
                                bs,
                                depth,
                                open_parenth,
                            )?))),
                            _ => Ok(Bite(EStdFunc(self.read_func(
                                varname,
                                slab,
                                bs,
                                depth,
                                open_parenth,
                            )?))),
                        }
                    }
                }
            }
        }
    }

    fn read_varname(bs: &mut &[u8]) -> Result<Token<String>, Error> {
        spaces!(bs);

        let mut toklen = 0;
        while Self::is_varname_byte_opt(peek_n!(bs, toklen), toklen) {
            toklen += 1;
        }

        if toklen == 0 {
            return Ok(Pass);
        }

        let out = unsafe { from_utf8_unchecked(&bs[..toklen]) }.to_owned();
        skip_n!(bs, toklen);
        Ok(Bite(out))
    }

    fn read_open_parenthesis(bs: &mut &[u8]) -> Result<Token<u8>, Error> {
        spaces!(bs);

        match peek!(bs) {
            Some(b'(' | b'[') => Ok(Bite(match read!(bs) {
                Ok(b) => b,
                Err(..) => return Err(Error::Unreachable),
            })),
            _ => Ok(Pass),
        }
    }

    #[allow(clippy::cognitive_complexity, clippy::too_many_lines)] // Might revisit later.
    fn read_func(
        &self,
        fname: String,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
        open_parenth: u8,
    ) -> Result<StdFunc, Error> {
        let close_parenth = match open_parenth {
            b'(' => b')',
            b'[' => b']',
            _ => return Err(Error::Expected(String::from("'(' or '['"))),
        };
        let mut args = Vec::<ExpressionI>::with_capacity(4);
        loop {
            spaces!(bs);
            match peek!(bs) {
                Some(b) => {
                    if b == close_parenth {
                        skip!(bs);
                        break;
                    }
                }
                None => return Err(Error::EofWhileParsing(fname)),
            }
            if !args.is_empty() {
                match read!(bs) {
                    // I accept ',' or ';' because the TV API disallows the ',' char in symbols... so I'm using ';' as a compromise.
                    Ok(b',' | b';') => {}
                    _ => return Err(Error::Expected(String::from("',' or ';'"))),
                }
            }
            args.push(self.read_expression(slab, bs, depth + 1, false)?);
        }

        let fname_str = fname.as_str();
        match fname_str {
            "int" => {
                if args.len() == 1 {
                    Ok(EFuncInt(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("int: expected one arg")))
                }
            }
            "ceil" => {
                if args.len() == 1 {
                    Ok(EFuncCeil(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("ceil: expected one arg")))
                }
            }
            "floor" => {
                if args.len() == 1 {
                    Ok(EFuncFloor(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("floor: expected one arg")))
                }
            }
            "abs" => {
                if args.len() == 1 {
                    Ok(EFuncAbs(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("abs: expected one arg")))
                }
            }
            "sign" => {
                if args.len() == 1 {
                    Ok(EFuncSign(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("sign: expected one arg")))
                }
            }
            "log" => {
                if args.len() == 1 {
                    Ok(EFuncLog {
                        base: None,
                        expr: match args.pop() {
                            Some(xi) => xi,
                            None => return Err(Error::Unreachable),
                        },
                    })
                } else if args.len() == 2 {
                    let Some(expr) = args.pop() else {
                        return Err(Error::Unreachable);
                    };
                    Ok(EFuncLog {
                        base: Some(match args.pop() {
                            Some(xi) => xi,
                            None => return Err(Error::Unreachable),
                        }),
                        expr,
                    })
                } else {
                    Err(Error::WrongArgs(String::from(
                        "expected log(x) or log(base,x)",
                    )))
                }
            }
            "round" => {
                if args.len() == 1 {
                    Ok(EFuncRound {
                        modulus: None,
                        expr: match args.pop() {
                            Some(xi) => xi,
                            None => return Err(Error::Unreachable),
                        },
                    })
                } else if args.len() == 2 {
                    let Some(expr) = args.pop() else {
                        return Err(Error::Unreachable);
                    };
                    Ok(EFuncRound {
                        modulus: Some(match args.pop() {
                            Some(xi) => xi,
                            None => return Err(Error::Unreachable),
                        }),
                        expr,
                    })
                } else {
                    Err(Error::WrongArgs(String::from(
                        "round: expected round(x) or round(modulus,x)",
                    )))
                }
            }
            "min" => {
                if args.is_empty() {
                    Err(Error::WrongArgs(String::from(
                        "min: expected one or more args",
                    )))
                } else {
                    remove_no_panic(&mut args, 0).map_or(Err(Error::Unreachable), |first| {
                        Ok(EFuncMin { first, rest: args })
                    })
                }
            }
            "max" => {
                if args.is_empty() {
                    Err(Error::WrongArgs(String::from(
                        "max: expected one or more args",
                    )))
                } else {
                    remove_no_panic(&mut args, 0).map_or(Err(Error::Unreachable), |first| {
                        Ok(EFuncMax { first, rest: args })
                    })
                }
            }

            "e" => {
                if args.is_empty() {
                    Ok(EFuncE)
                } else {
                    Err(Error::WrongArgs(String::from("e: expected no args")))
                }
            }
            "pi" => {
                if args.is_empty() {
                    Ok(EFuncPi)
                } else {
                    Err(Error::WrongArgs(String::from("pi: expected no args")))
                }
            }

            "sin" => {
                if args.len() == 1 {
                    Ok(EFuncSin(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("sin: expected one arg")))
                }
            }
            "cos" => {
                if args.len() == 1 {
                    Ok(EFuncCos(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("cos: expected one arg")))
                }
            }
            "tan" => {
                if args.len() == 1 {
                    Ok(EFuncTan(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("tan: expected one arg")))
                }
            }
            "asin" => {
                if args.len() == 1 {
                    Ok(EFuncASin(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("asin: expected one arg")))
                }
            }
            "acos" => {
                if args.len() == 1 {
                    Ok(EFuncACos(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("acos: expected one arg")))
                }
            }
            "atan" => {
                if args.len() == 1 {
                    Ok(EFuncATan(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("atan: expected one arg")))
                }
            }
            "sinh" => {
                if args.len() == 1 {
                    Ok(EFuncSinH(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("sinh: expected one arg")))
                }
            }
            "cosh" => {
                if args.len() == 1 {
                    Ok(EFuncCosH(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("cosh: expected one arg")))
                }
            }
            "tanh" => {
                if args.len() == 1 {
                    Ok(EFuncTanH(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("tanh: expected one arg")))
                }
            }
            "asinh" => {
                if args.len() == 1 {
                    Ok(EFuncASinH(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("asinh: expected one arg")))
                }
            }
            "acosh" => {
                if args.len() == 1 {
                    Ok(EFuncACosH(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("acosh: expected one arg")))
                }
            }
            "atanh" => {
                if args.len() == 1 {
                    Ok(EFuncATanH(match args.pop() {
                        Some(xi) => xi,
                        None => return Err(Error::Unreachable),
                    }))
                } else {
                    Err(Error::WrongArgs(String::from("atanh: expected one arg")))
                }
            }

            _ => {
                #[cfg(feature = "unsafe-vars")]
                match slab.unsafe_vars.get(fname_str) {
                    None => Ok(EFunc { name: fname, args }),
                    Some(&ptr) => Ok(EUnsafeVar { name: fname, ptr }),
                }

                #[cfg(not(feature = "unsafe-vars"))]
                Ok(EFunc { name: fname, args })
            }
        }
    }

    fn read_printfunc(
        &self,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
        open_parenth: u8,
    ) -> Result<PrintFunc, Error> {
        let close_parenth = match open_parenth {
            b'(' => b')',
            b'[' => b']',
            _ => return Err(Error::Expected(String::from("'(' or '['"))),
        };
        let mut args = Vec::<ExpressionOrString>::with_capacity(8);
        loop {
            spaces!(bs);
            match peek!(bs) {
                Some(b) => {
                    if b == close_parenth {
                        skip!(bs);
                        break;
                    }
                }
                None => {
                    return Err(Error::EofWhileParsing(String::from("print")));
                }
            }
            if !args.is_empty() {
                match read!(bs) {
                    Ok(b',' | b';') => {}
                    _ => {
                        return Err(Error::Expected(String::from("',' or ';'")));
                    }
                }
            }
            args.push(self.read_expressionorstring(slab, bs, depth + 1)?);
        }

        Ok(PrintFunc(args))
    }

    fn read_expressionorstring(
        &self,
        slab: &mut ParseSlab,
        bs: &mut &[u8],
        depth: usize,
    ) -> Result<ExpressionOrString, Error> {
        match Self::read_string(bs)? {
            Pass => {}
            Bite(s) => return Ok(EStr(s)),
        }
        Ok(EExpr(self.read_expression(slab, bs, depth + 1, false)?))
    }

    // TODO: Improve this logic, especially to handle embedded quotes:
    fn read_string(bs: &mut &[u8]) -> Result<Token<String>, Error> {
        spaces!(bs);

        match peek!(bs) {
            None => {
                return Err(Error::EofWhileParsing(String::from(
                    "opening quote of string",
                )))
            }
            Some(b'"') => {
                skip!(bs);
            }
            Some(_) => return Ok(Pass),
        }

        let mut toklen = 0;
        while match peek_n!(bs, toklen) {
            None | Some(b'"') => false,
            Some(_) => true,
        } {
            toklen += 1;
        }

        let out = from_utf8(&bs[..toklen])
            .map_err(|_| Error::Utf8ErrorWhileParsing(String::from("string")))?;
        skip_n!(bs, toklen);
        match read!(bs) {
            Err(Error::EOF) => Err(Error::EofWhileParsing(String::from("string"))),
            Ok(b'"') => Ok(Bite(out.to_owned())),
            Err(_) | Ok(_) => Err(Error::Unreachable),
        }
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for Value {
    fn default() -> Self {
        EConstant(std::f32::NAN)
    }
}

// A version of Vec::remove that doesn't panic:
// (Mostly copy-pasted from https://doc.rust-lang.org/src/alloc/vec.rs.html#991-1010 .)
pub(crate) fn remove_no_panic<T>(vself: &mut Vec<T>, index: usize) -> Option<T> {
    let len = vself.len();
    if index >= len {
        return None;
    }
    unsafe {
        // infallible
        let ret;
        {
            // the place we are taking from.
            let ptr = vself.as_mut_ptr().add(index);
            // copy it out, unsafely having a copy of the value on
            // the stack and in the vector at the same time.
            ret = ptr::read(ptr);

            // Shift everything down to fill in that spot.
            ptr::copy(ptr.offset(1), ptr, len - index - 1);
        }
        vself.set_len(len - 1);
        Some(ret)
    }
}

#[cfg(test)]
mod internal_tests {
    use super::*;
    use crate::slab::Slab;

    // Commented so I can compile with stable Rust.
    // extern crate test;
    // use test::{Bencher, black_box};

    #[test]
    fn rem_no_panic() {
        let mut v = vec![1u8, 2, 3];
        assert_eq!(format!("{v:?}"), "[1, 2, 3]");
        assert_eq!(remove_no_panic(&mut v, 1), Some(2));
        assert_eq!(remove_no_panic(&mut v, 10), None);
        assert_eq!(format!("{v:?}"), "[1, 3]");
    }

    #[test]
    fn util() {
        match (|| -> Result<(), Error> {
            let bsarr = [1, 2, 3];
            let bs = &mut &bsarr[..];

            assert_eq!(peek!(bs), Some(1));
            assert_eq!(peek_n!(bs, 1), Some(2));
            assert_eq!(peek_n!(bs, 2), Some(3));
            assert_eq!(peek_n!(bs, 3), None);

            assert_eq!(read!(bs)?, 1);
            skip!(bs);
            assert_eq!(read!(bs)?, 3);
            match read!(bs).err() {
                Some(Error::EOF) => {}
                _ => panic!("I expected an EOF"),
            }

            Ok(())
        })() {
            Ok(()) => {}
            Err(_) => {
                unimplemented!();
            }
        }

        assert!(([0u8; 0]).is_empty());
        assert!(!([1]).is_empty());
        assert!((b"").is_empty());
        assert!(!(b"x").is_empty());

        let b = b' ';
        assert!(is_space!(b));
        let b = b'\t';
        assert!(is_space!(b));
        let b = b'\r';
        assert!(is_space!(b));
        let b = b'\n';
        assert!(is_space!(b));
        let b = b'a';
        assert!(!is_space!(b));
        let b = b'1';
        assert!(!is_space!(b));
        let b = b'.';
        assert!(!is_space!(b));

        {
            let bsarr = b"  abc 123   ";
            let bs = &mut &bsarr[..];
            spaces!(bs);
            assert_eq!(bs, b"abc 123   ");
        }
    }

    #[test]
    fn priv_tests() {
        assert!(Parser::is_varname_byte_opt(Some(b'a'), 0));

        let mut slab = Slab::new();

        {
            let bsarr = b"12.34";
            let bs = &mut &bsarr[..];
            assert_eq!(
                Parser::new().read_value(&mut slab.ps, bs, 0),
                Ok(EConstant(12.34))
            );
        }
    }

    //// Commented so I can compile this library with stable Rust.
    // #[bench]
    // #[allow(non_snake_case)]
    // fn spaces_1M(bencher:&mut Bencher) {
    //     let zero = "abc".as_bytes();
    //     let one = " abc".as_bytes();
    //     let two = "  abc".as_bytes();
    //     bencher.iter(|| {
    //         let (z1,z2,z3,z4) = (&zero[..], &zero[..], &zero[..], &zero[..]);  // Localize
    //         let (o1,o2) = (&one[..], &one[..]);
    //         let t1 = &two[..];
    //         for _ in 0..1000 {
    //             let (z1,z2,z3,z4) = (&mut &z1[..], &mut &z2[..], &mut &z3[..], &mut &z4[..]);
    //             let (o1,o2) = (&mut &o1[..], &mut &o2[..]);
    //             let t1 = &mut &t1[..];
    //             spaces!(z1);
    //             spaces!(z2);
    //             spaces!(z3);
    //             spaces!(z4);
    //             spaces!(o1);
    //             spaces!(o2);
    //             spaces!(t1);
    //             black_box(z1);
    //             black_box(z2);
    //             black_box(z3);
    //             black_box(z4);
    //             black_box(o1);
    //             black_box(o2);
    //             black_box(t1);
    //         }
    //     });
    // }
}
