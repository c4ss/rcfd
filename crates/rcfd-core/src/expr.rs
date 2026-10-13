//! Expression language for setup and post-processing (F-104).
//!
//! Syntax (a small, conventional infix grammar written from scratch):
//!
//! ```text
//! 2.5 [m/s] * (1 - (y / 0.05 [m])^2)        literal with unit, power, grouping
//! sqrt(ux^2 + uy^2)                         functions
//! if(x < 1 [m], 300 [K], 350 [K])           comparisons and if(cond, a, b)
//! heat_flux_scale * step(t - 2 [s])         named quantities and expressions
//! ```
//!
//! * Numbers may carry a unit in square brackets (`10 [km/h]`); values are
//!   converted to SI immediately, every sub-expression carries a
//!   [`Dimension`] and `+`, `-`, comparisons, `min`, `max`, `if` branches
//!   require equal dimensions. Mismatches are rejected at compile time with
//!   the source span of the offending operator or argument.
//! * `^` is right-associative and binds tighter than unary minus. If the base
//!   is dimensional the exponent must be a compile-time constant that yields
//!   integer dimension exponents (so `x^0.5` is accepted only for dimensions
//!   with even exponents, as with `sqrt`).
//! * Names resolve, in order, to compile-time variable slots (`x`, `y`, `z`,
//!   `t`, field values, ...), constants (report values, `pi`, `e`) and other
//!   named expressions of an [`ExprSet`] (inlined; cycles are rejected).
//!
//! Compiled expressions evaluate on a slice of `f64` slot values, so
//! evaluating one per boundary face or cell is cheap.

use crate::error::Error;
use crate::units::{Dimension, Quantity, Unit};
use std::collections::BTreeMap;
use std::fmt;

/// A parse or dimension error with the byte span of the offending source text.
#[derive(Debug, Clone, PartialEq)]
pub struct ExprError {
    /// What is wrong.
    pub message: String,
    /// Start byte offset in the source (inclusive).
    pub start: usize,
    /// End byte offset in the source (exclusive).
    pub end: usize,
}

impl ExprError {
    fn new(message: impl Into<String>, start: usize, end: usize) -> Self {
        ExprError {
            message: message.into(),
            start,
            end,
        }
    }

    /// Two-line rendering of the source with a caret marker under the span.
    pub fn render(&self, src: &str) -> String {
        let start = self.start.min(src.len());
        let end = self.end.clamp(start, src.len());
        let width = src[start..end].chars().count().max(1);
        let pad = src[..start].chars().count();
        format!("{src}\n{}{}", " ".repeat(pad), "^".repeat(width))
    }
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (column {})", self.message, self.start + 1)
    }
}

impl std::error::Error for ExprError {}

impl From<ExprError> for Error {
    fn from(e: ExprError) -> Self {
        Error::Other(e.to_string())
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64, Option<String>),
    Ident(String),
    Op(&'static str),
    LParen,
    RParen,
    Comma,
}

fn lex(src: &str) -> Result<Vec<(Tok, usize, usize)>, ExprError> {
    let b = src.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        if c.is_ascii_digit() || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                let mut j = i + 1;
                if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                    j += 1;
                }
                if j < b.len() && b[j].is_ascii_digit() {
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    i = j;
                }
            }
            let v: f64 = src[start..i]
                .parse()
                .map_err(|_| ExprError::new(format!("invalid number '{}'", &src[start..i]), start, i))?;
            // optional unit in brackets after optional whitespace
            let mut j = i;
            while j < b.len() && b[j] == b' ' {
                j += 1;
            }
            let mut unit = None;
            if j < b.len() && b[j] == b'[' {
                let close = src[j..]
                    .find(']')
                    .ok_or_else(|| ExprError::new("missing ']' after unit", j, b.len()))?;
                unit = Some(src[j + 1..j + close].trim().to_string());
                i = j + close + 1;
            }
            out.push((Tok::Num(v, unit), start, i));
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_' || b[i] == b'.') {
                i += 1;
            }
            out.push((Tok::Ident(src[start..i].to_string()), start, i));
            continue;
        }
        let two = src.get(i..i + 2).unwrap_or("");
        let op2 = ["<=", ">=", "==", "!=", "&&", "||"].iter().find(|o| **o == two);
        if let Some(o) = op2 {
            out.push((Tok::Op(o), start, i + 2));
            i += 2;
            continue;
        }
        let tok = match c {
            b'+' => Tok::Op("+"),
            b'-' => Tok::Op("-"),
            b'*' => Tok::Op("*"),
            b'/' => Tok::Op("/"),
            b'^' => Tok::Op("^"),
            b'<' => Tok::Op("<"),
            b'>' => Tok::Op(">"),
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b',' => Tok::Comma,
            _ => {
                let ch = src[i..].chars().next().unwrap_or('?');
                return Err(ExprError::new(
                    format!("unexpected character '{ch}'"),
                    i,
                    i + ch.len_utf8(),
                ));
            }
        };
        out.push((tok, start, i + 1));
        i += 1;
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
enum Node {
    Num(f64, Dimension, usize, usize),
    Var(String, usize, usize),
    Neg(Box<Node>, usize, usize),
    Bin(&'static str, Box<Node>, Box<Node>, usize, usize),
    Call(String, Vec<Node>, usize, usize),
}

impl Node {
    fn span(&self) -> (usize, usize) {
        match self {
            Node::Num(_, _, s, e) | Node::Var(_, s, e) | Node::Neg(_, s, e) => (*s, *e),
            Node::Bin(_, _, _, s, e) | Node::Call(_, _, s, e) => (*s, *e),
        }
    }
}

struct Parser {
    toks: Vec<(Tok, usize, usize)>,
    i: usize,
    end: usize,
}

fn binding_power(op: &str) -> Option<(u8, bool)> {
    // (precedence, right associative)
    Some(match op {
        "||" => (1, false),
        "&&" => (2, false),
        "<" | ">" | "<=" | ">=" | "==" | "!=" => (3, false),
        "+" | "-" => (4, false),
        "*" | "/" => (5, false),
        "^" => (8, true),
        _ => return None,
    })
}

impl Parser {
    fn peek(&self) -> Option<&(Tok, usize, usize)> {
        self.toks.get(self.i)
    }

    fn expr(&mut self, min_prec: u8) -> Result<Node, ExprError> {
        let mut lhs = self.prefix()?;
        while let Some((Tok::Op(op), _, _)) = self.peek().cloned() {
            let Some((prec, right)) = binding_power(op) else {
                break;
            };
            if prec < min_prec {
                break;
            }
            self.i += 1;
            let rhs = self.expr(if right { prec } else { prec + 1 })?;
            let (ls, _) = lhs.span();
            let (_, re) = rhs.span();
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs), ls, re);
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Node, ExprError> {
        let Some((tok, s, e)) = self.peek().cloned() else {
            return Err(ExprError::new("unexpected end of expression", self.end, self.end));
        };
        self.i += 1;
        match tok {
            Tok::Num(v, unit) => {
                let (v, dim) = match unit {
                    None => (v, Dimension::NONE),
                    Some(u) => {
                        let unit = Unit::parse(&u).map_err(|err| ExprError::new(err.to_string(), s, e))?;
                        (unit.to_si(v), unit.dim)
                    }
                };
                Ok(Node::Num(v, dim, s, e))
            }
            Tok::Ident(name) => {
                if matches!(self.peek(), Some((Tok::LParen, _, _))) {
                    self.i += 1;
                    let mut args = Vec::new();
                    if matches!(self.peek(), Some((Tok::RParen, _, _))) {
                        self.i += 1;
                    } else {
                        loop {
                            args.push(self.expr(1)?);
                            match self.peek().cloned() {
                                Some((Tok::Comma, _, _)) => self.i += 1,
                                Some((Tok::RParen, _, re)) => {
                                    self.i += 1;
                                    return Ok(Node::Call(name, args, s, re));
                                }
                                Some((_, ts, te)) => {
                                    return Err(ExprError::new("expected ',' or ')' in argument list", ts, te))
                                }
                                None => return Err(ExprError::new("missing ')'", self.end, self.end)),
                            }
                        }
                    }
                    let re = self.toks[self.i - 1].2;
                    return Ok(Node::Call(name, args, s, re));
                }
                Ok(Node::Var(name, s, e))
            }
            Tok::Op("-") => {
                let inner = self.expr(7)?;
                let (_, ie) = inner.span();
                Ok(Node::Neg(Box::new(inner), s, ie))
            }
            Tok::Op("+") => self.expr(7),
            Tok::LParen => {
                let inner = self.expr(1)?;
                match self.peek().cloned() {
                    Some((Tok::RParen, _, _)) => {
                        self.i += 1;
                        Ok(inner)
                    }
                    Some((_, ts, te)) => Err(ExprError::new("expected ')'", ts, te)),
                    None => Err(ExprError::new("missing ')'", self.end, self.end)),
                }
            }
            Tok::Op(o) => Err(ExprError::new(format!("unexpected operator '{o}'"), s, e)),
            Tok::RParen => Err(ExprError::new("unexpected ')'", s, e)),
            Tok::Comma => Err(ExprError::new("unexpected ','", s, e)),
        }
    }
}

/// A parsed (not yet dimension-checked) expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    src: String,
    root: Node,
}

impl Expr {
    /// Parse `src`; syntax errors carry the offending byte span.
    pub fn parse(src: &str) -> Result<Expr, ExprError> {
        let toks = lex(src)?;
        if toks.is_empty() {
            return Err(ExprError::new("empty expression", 0, 0));
        }
        let mut p = Parser {
            toks,
            i: 0,
            end: src.len(),
        };
        let root = p.expr(1)?;
        if let Some((_, s, e)) = p.peek() {
            return Err(ExprError::new(
                "unexpected token after the end of the expression",
                *s,
                *e,
            ));
        }
        Ok(Expr {
            src: src.to_string(),
            root,
        })
    }

    /// The source text.
    pub fn source(&self) -> &str {
        &self.src
    }

    /// Free identifiers (variables, constants, named expressions and function-free names), sorted and unique.
    pub fn identifiers(&self) -> Vec<String> {
        fn walk(n: &Node, out: &mut Vec<String>) {
            match n {
                Node::Var(v, _, _) => out.push(v.clone()),
                Node::Neg(a, _, _) => walk(a, out),
                Node::Bin(_, a, b, _, _) => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Call(_, args, _, _) => args.iter().for_each(|a| walk(a, out)),
                Node::Num(..) => {}
            }
        }
        let mut v = Vec::new();
        walk(&self.root, &mut v);
        v.sort();
        v.dedup();
        v
    }

    /// Dimension-check and compile against `ctx`.
    pub fn compile(&self, ctx: &Context) -> Result<Compiled, ExprError> {
        let mut stack = Vec::new();
        compile_node(&self.root, ctx, &mut stack).map(|(c, _)| c)
    }

    /// Convenience: compile with `vars` as slots (in order) and evaluate once.
    pub fn eval_with(&self, vars: &[(&str, Quantity)]) -> Result<Quantity, ExprError> {
        let mut ctx = Context::new();
        for (n, q) in vars {
            ctx.add_var(n, q.dim);
        }
        let c = self.compile(&ctx)?;
        let vals: Vec<f64> = vars.iter().map(|(_, q)| q.si).collect();
        Ok(Quantity::from_si(c.eval(&vals), c.dimension()))
    }
}

/// Compile-time environment: variable slots, constants and named expressions.
#[derive(Debug, Clone, Default)]
pub struct Context {
    vars: Vec<(String, Dimension)>,
    consts: BTreeMap<String, Quantity>,
    named: BTreeMap<String, Expr>,
}

impl Context {
    /// Context with the built-in constants `pi` and `e`.
    pub fn new() -> Self {
        let mut c = Context::default();
        c.consts
            .insert("pi".into(), Quantity::from_si(std::f64::consts::PI, Dimension::NONE));
        c.consts
            .insert("e".into(), Quantity::from_si(std::f64::consts::E, Dimension::NONE));
        c
    }

    /// Declare a variable; its slot index is the order of declaration.
    pub fn add_var(&mut self, name: &str, dim: Dimension) -> usize {
        self.vars.push((name.to_string(), dim));
        self.vars.len() - 1
    }

    /// Declare a compile-time constant (for example a report value or a material property).
    pub fn add_const(&mut self, name: &str, q: Quantity) {
        self.consts.insert(name.to_string(), q);
    }

    /// Declare a named expression.
    pub fn add_named(&mut self, name: &str, e: Expr) {
        self.named.insert(name.to_string(), e);
    }

    /// Slot of a variable.
    pub fn slot(&self, name: &str) -> Option<usize> {
        self.vars.iter().position(|(n, _)| n == name)
    }
}

/// Set of named expressions (`settings expressions.<name>`), validated together.
#[derive(Debug, Clone, Default)]
pub struct ExprSet {
    defs: BTreeMap<String, Expr>,
}

impl ExprSet {
    /// Empty set.
    pub fn new() -> Self {
        ExprSet::default()
    }

    /// Add or replace a named expression, parsing `src`.
    pub fn define(&mut self, name: &str, src: &str) -> Result<(), ExprError> {
        self.defs.insert(name.to_string(), Expr::parse(src)?);
        Ok(())
    }

    /// Look a definition up.
    pub fn get(&self, name: &str) -> Option<&Expr> {
        self.defs.get(name)
    }

    /// Names in the set.
    pub fn names(&self) -> impl Iterator<Item = &String> {
        self.defs.keys()
    }

    /// Install every definition into `ctx`.
    pub fn install(&self, ctx: &mut Context) {
        for (n, e) in &self.defs {
            ctx.add_named(n, e.clone());
        }
    }

    /// Compile every expression against `ctx` (plus the others of the set), returning
    /// `(name, dimension)` or the first error with its name.
    pub fn check_all(&self, ctx: &Context) -> Result<Vec<(String, Dimension)>, (String, ExprError)> {
        let mut c = ctx.clone();
        self.install(&mut c);
        let mut out = Vec::new();
        for (n, e) in &self.defs {
            let comp = e.compile(&c).map_err(|err| (n.clone(), err))?;
            out.push((n.clone(), comp.dimension()));
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Func {
    Sqrt,
    Abs,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Exp,
    Ln,
    Log10,
    Pow,
    Min,
    Max,
    Step,
    If,
    Hypot,
    Clamp,
    Sign,
}

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Const(f64),
    Slot(usize),
    Neg(Box<Op>),
    Bin(char, Box<Op>, Box<Op>),
    Cmp(&'static str, Box<Op>, Box<Op>),
    Call(Func, Vec<Op>),
}

/// A dimension-checked expression ready for repeated evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct Compiled {
    dim: Dimension,
    constant: bool,
    op: Op,
}

impl Compiled {
    /// Dimension of the result.
    pub fn dimension(&self) -> Dimension {
        self.dim
    }

    /// Whether the value does not depend on any variable.
    pub fn is_constant(&self) -> bool {
        self.constant
    }

    /// Evaluate on variable slot values (SI).
    pub fn eval(&self, slots: &[f64]) -> f64 {
        eval_op(&self.op, slots)
    }

    /// The constant value, when [`is_constant`](Self::is_constant).
    pub fn constant_value(&self) -> Option<f64> {
        self.constant.then(|| self.eval(&[]))
    }
}

fn eval_op(op: &Op, s: &[f64]) -> f64 {
    match op {
        Op::Const(v) => *v,
        Op::Slot(i) => s[*i],
        Op::Neg(a) => -eval_op(a, s),
        Op::Bin(c, a, b) => {
            let (x, y) = (eval_op(a, s), eval_op(b, s));
            match c {
                '+' => x + y,
                '-' => x - y,
                '*' => x * y,
                '/' => x / y,
                _ => x.powf(y),
            }
        }
        Op::Cmp(o, a, b) => {
            let (x, y) = (eval_op(a, s), eval_op(b, s));
            let t = match *o {
                "<" => x < y,
                ">" => x > y,
                "<=" => x <= y,
                ">=" => x >= y,
                "==" => x == y,
                "!=" => x != y,
                "&&" => x != 0.0 && y != 0.0,
                _ => x != 0.0 || y != 0.0,
            };
            f64::from(u8::from(t))
        }
        Op::Call(f, args) => {
            let a: Vec<f64> = args.iter().map(|x| eval_op(x, s)).collect();
            match f {
                Func::Sqrt => a[0].sqrt(),
                Func::Abs => a[0].abs(),
                Func::Sin => a[0].sin(),
                Func::Cos => a[0].cos(),
                Func::Tan => a[0].tan(),
                Func::Asin => a[0].asin(),
                Func::Acos => a[0].acos(),
                Func::Atan => a[0].atan(),
                Func::Atan2 => a[0].atan2(a[1]),
                Func::Exp => a[0].exp(),
                Func::Ln => a[0].ln(),
                Func::Log10 => a[0].log10(),
                Func::Pow => a[0].powf(a[1]),
                Func::Min => a[0].min(a[1]),
                Func::Max => a[0].max(a[1]),
                Func::Step => f64::from(u8::from(a[0] >= 0.0)),
                Func::If => {
                    if a[0] != 0.0 {
                        a[1]
                    } else {
                        a[2]
                    }
                }
                Func::Hypot => a[0].hypot(a[1]),
                Func::Clamp => a[0].max(a[1]).min(a[2]),
                Func::Sign => {
                    if a[0] > 0.0 {
                        1.0
                    } else if a[0] < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                }
            }
        }
    }
}

fn dim_pow(d: Dimension, p: f64, span: (usize, usize)) -> Result<Dimension, ExprError> {
    let mut out = [0i8; 7];
    for (o, e) in out.iter_mut().zip(d.0) {
        let v = f64::from(e) * p;
        if (v - v.round()).abs() > 1e-9 {
            return Err(ExprError::new(
                format!("power {p} of a quantity with dimension '{d}' gives a fractional dimension"),
                span.0,
                span.1,
            ));
        }
        *o = v.round() as i8;
    }
    Ok(Dimension(out))
}

fn compile_node(n: &Node, ctx: &Context, stack: &mut Vec<String>) -> Result<(Compiled, ()), ExprError> {
    let mk = |dim, constant, op| Compiled { dim, constant, op };
    match n {
        Node::Num(v, d, _, _) => Ok((mk(*d, true, Op::Const(*v)), ())),
        Node::Var(name, s, e) => {
            if let Some(i) = ctx.slot(name) {
                return Ok((mk(ctx.vars[i].1, false, Op::Slot(i)), ()));
            }
            if let Some(q) = ctx.consts.get(name) {
                return Ok((mk(q.dim, true, Op::Const(q.si)), ()));
            }
            if let Some(def) = ctx.named.get(name) {
                if stack.contains(name) {
                    return Err(ExprError::new(
                        format!(
                            "named expression '{name}' is defined in terms of itself ({})",
                            stack.join(" -> ")
                        ),
                        *s,
                        *e,
                    ));
                }
                stack.push(name.clone());
                let r = compile_node(&def.root, ctx, stack);
                stack.pop();
                return r.map_err(|inner| {
                    ExprError::new(
                        format!(
                            "in named expression '{name}' (column {}): {}",
                            inner.start + 1,
                            inner.message
                        ),
                        *s,
                        *e,
                    )
                });
            }
            Err(ExprError::new(format!("unknown name '{name}'"), *s, *e))
        }
        Node::Neg(a, _, _) => {
            let (c, _) = compile_node(a, ctx, stack)?;
            Ok((mk(c.dim, c.constant, Op::Neg(Box::new(c.op))), ()))
        }
        Node::Bin(op, a, b, s, e) => {
            let (ca, _) = compile_node(a, ctx, stack)?;
            let (cb, _) = compile_node(b, ctx, stack)?;
            let constant = ca.constant && cb.constant;
            match *op {
                "+" | "-" => {
                    if ca.dim != cb.dim {
                        return Err(ExprError::new(
                            format!(
                                "cannot {} quantities with different dimensions: '{}' and '{}'",
                                if *op == "+" { "add" } else { "subtract" },
                                ca.dim,
                                cb.dim
                            ),
                            *s,
                            *e,
                        ));
                    }
                    let c = if *op == "+" { '+' } else { '-' };
                    Ok((mk(ca.dim, constant, Op::Bin(c, Box::new(ca.op), Box::new(cb.op))), ()))
                }
                "*" => Ok((
                    mk(
                        ca.dim * cb.dim,
                        constant,
                        Op::Bin('*', Box::new(ca.op), Box::new(cb.op)),
                    ),
                    (),
                )),
                "/" => Ok((
                    mk(
                        ca.dim / cb.dim,
                        constant,
                        Op::Bin('/', Box::new(ca.op), Box::new(cb.op)),
                    ),
                    (),
                )),
                "^" => {
                    let (bs, be) = b.span();
                    if !cb.dim.is_dimensionless() {
                        return Err(ExprError::new(
                            format!("exponent must be dimensionless, found '{}'", cb.dim),
                            bs,
                            be,
                        ));
                    }
                    let dim = if ca.dim.is_dimensionless() {
                        Dimension::NONE
                    } else {
                        let p = cb.constant_value().ok_or_else(|| {
                            ExprError::new("the exponent of a dimensional quantity must be a constant", bs, be)
                        })?;
                        dim_pow(ca.dim, p, (*s, *e))?
                    };
                    Ok((mk(dim, constant, Op::Bin('^', Box::new(ca.op), Box::new(cb.op))), ()))
                }
                cmp => {
                    let logical = cmp == "&&" || cmp == "||";
                    if !logical && ca.dim != cb.dim {
                        return Err(ExprError::new(
                            format!(
                                "cannot compare quantities with different dimensions: '{}' and '{}'",
                                ca.dim, cb.dim
                            ),
                            *s,
                            *e,
                        ));
                    }
                    Ok((
                        mk(
                            Dimension::NONE,
                            constant,
                            Op::Cmp(
                                match cmp {
                                    "<" => "<",
                                    ">" => ">",
                                    "<=" => "<=",
                                    ">=" => ">=",
                                    "==" => "==",
                                    "!=" => "!=",
                                    "&&" => "&&",
                                    _ => "||",
                                },
                                Box::new(ca.op),
                                Box::new(cb.op),
                            ),
                        ),
                        (),
                    ))
                }
            }
        }
        Node::Call(name, args, s, e) => {
            let (f, arity) = match name.as_str() {
                "sqrt" => (Func::Sqrt, 1),
                "abs" => (Func::Abs, 1),
                "sin" => (Func::Sin, 1),
                "cos" => (Func::Cos, 1),
                "tan" => (Func::Tan, 1),
                "asin" => (Func::Asin, 1),
                "acos" => (Func::Acos, 1),
                "atan" => (Func::Atan, 1),
                "atan2" => (Func::Atan2, 2),
                "exp" => (Func::Exp, 1),
                "ln" => (Func::Ln, 1),
                "log10" => (Func::Log10, 1),
                "pow" => (Func::Pow, 2),
                "min" => (Func::Min, 2),
                "max" => (Func::Max, 2),
                "step" => (Func::Step, 1),
                "if" => (Func::If, 3),
                "hypot" => (Func::Hypot, 2),
                "clamp" => (Func::Clamp, 3),
                "sign" => (Func::Sign, 1),
                other => return Err(ExprError::new(format!("unknown function '{other}'"), *s, *e)),
            };
            if args.len() != arity {
                return Err(ExprError::new(
                    format!("{name} takes {arity} argument(s), found {}", args.len()),
                    *s,
                    *e,
                ));
            }
            let mut cs = Vec::new();
            for a in args {
                cs.push((compile_node(a, ctx, stack)?.0, a.span()));
            }
            let constant = cs.iter().all(|(c, _)| c.constant);
            let need_dimless = |c: &Compiled, sp: (usize, usize)| {
                if c.dim.is_dimensionless() {
                    Ok(())
                } else {
                    Err(ExprError::new(
                        format!("{name} needs a dimensionless argument, found '{}'", c.dim),
                        sp.0,
                        sp.1,
                    ))
                }
            };
            let same = |cs: &[(Compiled, (usize, usize))], idx: &[usize]| {
                let d0 = cs[idx[0]].0.dim;
                for &i in &idx[1..] {
                    if cs[i].0.dim != d0 {
                        return Err(ExprError::new(
                            format!(
                                "{name} needs arguments with equal dimensions, found '{d0}' and '{}'",
                                cs[i].0.dim
                            ),
                            cs[i].1 .0,
                            cs[i].1 .1,
                        ));
                    }
                }
                Ok(d0)
            };
            let dim = match f {
                Func::Sqrt => dim_pow(cs[0].0.dim, 0.5, (*s, *e))?,
                Func::Abs => cs[0].0.dim,
                Func::Sin | Func::Cos | Func::Tan | Func::Exp | Func::Ln | Func::Log10 => {
                    need_dimless(&cs[0].0, cs[0].1)?;
                    Dimension::NONE
                }
                Func::Asin | Func::Acos | Func::Atan => {
                    need_dimless(&cs[0].0, cs[0].1)?;
                    Dimension::NONE
                }
                Func::Atan2 => {
                    same(&cs, &[0, 1])?;
                    Dimension::NONE
                }
                Func::Pow => {
                    need_dimless(&cs[1].0, cs[1].1)?;
                    if cs[0].0.dim.is_dimensionless() {
                        Dimension::NONE
                    } else {
                        let p = cs[1].0.constant_value().ok_or_else(|| {
                            ExprError::new(
                                "the exponent of a dimensional quantity must be a constant",
                                cs[1].1 .0,
                                cs[1].1 .1,
                            )
                        })?;
                        dim_pow(cs[0].0.dim, p, (*s, *e))?
                    }
                }
                Func::Min | Func::Max | Func::Hypot => same(&cs, &[0, 1])?,
                Func::Clamp => same(&cs, &[0, 1, 2])?,
                Func::Step | Func::Sign => Dimension::NONE,
                Func::If => {
                    need_dimless(&cs[0].0, cs[0].1)?;
                    same(&cs, &[1, 2])?
                }
            };
            let dim = if matches!(f, Func::Sign) { Dimension::NONE } else { dim };
            Ok((
                mk(dim, constant, Op::Call(f, cs.into_iter().map(|(c, _)| c.op).collect())),
                (),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(v: f64, d: Dimension) -> Quantity {
        Quantity::from_si(v, d)
    }

    #[test]
    fn arithmetic_precedence_and_units() {
        let e = Expr::parse("2 + 3 * 4 ^ 2 / 8").unwrap();
        assert_eq!(e.eval_with(&[]).unwrap().si, 8.0);
        // unary minus binds looser than ^
        assert_eq!(Expr::parse("-2^2").unwrap().eval_with(&[]).unwrap().si, -4.0);
        assert_eq!(Expr::parse("2^3^2").unwrap().eval_with(&[]).unwrap().si, 512.0);
        let v = Expr::parse("36 [km/h] + 5 [m/s]").unwrap().eval_with(&[]).unwrap();
        assert!((v.si - 15.0).abs() < 1e-12 && v.dim == Dimension::VELOCITY);
        let t = Expr::parse("20 [degC]").unwrap().eval_with(&[]).unwrap();
        assert!((t.si - 293.15).abs() < 1e-12 && t.dim == Dimension::TEMPERATURE);
        let a = Expr::parse("90 [deg]").unwrap().eval_with(&[]).unwrap();
        assert!((a.si - std::f64::consts::FRAC_PI_2).abs() < 1e-12 && a.dim.is_dimensionless());
    }

    #[test]
    fn variables_functions_and_conditionals() {
        let e = Expr::parse("2.5 [m/s] * (1 - (y / 0.05 [m])^2)").unwrap();
        let mut ctx = Context::new();
        ctx.add_var("y", Dimension::LENGTH);
        let c = e.compile(&ctx).unwrap();
        assert_eq!(c.dimension(), Dimension::VELOCITY);
        assert!(!c.is_constant());
        assert!((c.eval(&[0.0]) - 2.5).abs() < 1e-12);
        assert!(c.eval(&[0.05]).abs() < 1e-12);
        assert!((c.eval(&[0.025]) - 1.875).abs() < 1e-12);
        let r = Expr::parse("sqrt(ux^2 + uy^2)")
            .unwrap()
            .eval_with(&[("ux", q(3.0, Dimension::VELOCITY)), ("uy", q(4.0, Dimension::VELOCITY))])
            .unwrap();
        assert!((r.si - 5.0).abs() < 1e-12 && r.dim == Dimension::VELOCITY);
        let i = Expr::parse("if(x < 1 [m], 300 [K], 350 [K])").unwrap();
        let mut ctx = Context::new();
        ctx.add_var("x", Dimension::LENGTH);
        let c = i.compile(&ctx).unwrap();
        assert_eq!((c.eval(&[0.5]), c.eval(&[2.0])), (300.0, 350.0));
        assert_eq!(c.dimension(), Dimension::TEMPERATURE);
        let st = Expr::parse("10 * step(t - 2 [s])").unwrap();
        let mut ctx = Context::new();
        ctx.add_var("t", Dimension::TIME);
        let c = st.compile(&ctx).unwrap();
        assert_eq!((c.eval(&[1.0]), c.eval(&[3.0])), (0.0, 10.0));
        let h = Expr::parse("pi * sin(30 [deg]) + hypot(3 [m], 4 [m]) / 1 [m]")
            .unwrap()
            .eval_with(&[])
            .unwrap();
        assert!((h.si - (std::f64::consts::PI * 0.5 + 5.0)).abs() < 1e-12);
    }

    #[test]
    fn dimension_mismatches_are_rejected_with_located_errors() {
        let ctx = |names: &[(&str, Dimension)]| {
            let mut c = Context::new();
            for (n, d) in names {
                c.add_var(n, *d);
            }
            c
        };
        let c = ctx(&[("y", Dimension::LENGTH), ("T", Dimension::TEMPERATURE)]);
        let src = "3 [m/s] + y";
        let err = Expr::parse(src).unwrap().compile(&c).unwrap_err();
        assert!(
            err.message.contains("cannot add") && err.message.contains("m s^-1"),
            "{err}"
        );
        assert_eq!((err.start, err.end), (0, src.len()));
        let rendered = err.render(src);
        assert!(rendered.ends_with("^^^^^^^^^^^"), "{rendered}");
        let err = Expr::parse("sin(y)").unwrap().compile(&c).unwrap_err();
        assert!(err.message.contains("dimensionless") && err.start == 4, "{err:?}");
        assert!(Expr::parse("y < T").unwrap().compile(&c).is_err());
        assert!(Expr::parse("max(y, 1 [s])").unwrap().compile(&c).is_err());
        assert!(Expr::parse("y ^ 0.5").unwrap().compile(&c).is_err());
        assert!(Expr::parse("y ^ T").unwrap().compile(&c).is_err());
        assert!(Expr::parse("(y*y) ^ 0.5").unwrap().compile(&c).is_ok());
        assert_eq!(
            Expr::parse("sqrt(y*y)").unwrap().compile(&c).unwrap().dimension(),
            Dimension::LENGTH
        );
        let err = Expr::parse("foo + 1").unwrap().compile(&c).unwrap_err();
        assert!(err.message.contains("unknown name 'foo'") && err.start == 0);
        let err = Expr::parse("nope(1)").unwrap().compile(&c).unwrap_err();
        assert!(err.message.contains("unknown function"));
        let err = Expr::parse("sqrt(1, 2)").unwrap().compile(&c).unwrap_err();
        assert!(err.message.contains("takes 1"));
    }

    #[test]
    fn syntax_errors_are_located() {
        for (src, col, frag) in [
            ("1 + * 2", 4, "unexpected operator"),
            ("(1 + 2", 6, "missing ')'"),
            ("1 + ", 4, "end of expression"),
            ("3 $ 4", 2, "unexpected character"),
            ("2 [furlong]", 0, "unknown unit"),
            ("1 2", 2, "after the end"),
            ("f(1 2)", 4, "expected ','"),
            ("", 0, "empty"),
        ] {
            let err = Expr::parse(src).unwrap_err();
            assert_eq!(err.start, col, "{src}: {err:?}");
            assert!(err.message.contains(frag), "{src}: {err:?}");
        }
    }

    #[test]
    fn named_expressions_constants_and_cycles() {
        let mut set = ExprSet::new();
        set.define("u_mean", "1.5 [m/s]").unwrap();
        set.define("u_in", "u_mean * (1 - (y / h)^2)").unwrap();
        set.define("re", "u_mean * 2 * h / nu").unwrap();
        let mut ctx = Context::new();
        let iy = ctx.add_var("y", Dimension::LENGTH);
        ctx.add_const("h", q(0.5, Dimension::LENGTH));
        ctx.add_const("nu", q(0.01, Dimension::KINEMATIC_VISCOSITY));
        let dims = set.check_all(&ctx).unwrap();
        assert_eq!(dims.iter().find(|d| d.0 == "u_in").unwrap().1, Dimension::VELOCITY);
        assert!(dims.iter().find(|d| d.0 == "re").unwrap().1.is_dimensionless());
        set.install(&mut ctx);
        let c = Expr::parse("u_in").unwrap().compile(&ctx).unwrap();
        let mut slots = vec![0.0; 1];
        slots[iy] = 0.25;
        assert!((c.eval(&slots) - 1.125).abs() < 1e-12);
        let re = Expr::parse("re").unwrap().compile(&ctx).unwrap();
        assert!(re.is_constant() && (re.constant_value().unwrap() - 150.0).abs() < 1e-9);
        // cycle detection
        let mut bad = ExprSet::new();
        bad.define("a", "b + 1").unwrap();
        bad.define("b", "a * 2").unwrap();
        let (name, err) = bad.check_all(&Context::new()).unwrap_err();
        assert!(err.message.contains("itself"), "{name}: {err:?}");
        // errors in a referenced expression are attributed to it
        let mut bad = ExprSet::new();
        bad.define("v", "1 [m/s] + 2 [K]").unwrap();
        bad.define("w", "v * 2").unwrap();
        let (name, err) = bad.check_all(&Context::new()).unwrap_err();
        assert!(
            name == "v" || err.message.contains("named expression 'v'"),
            "{name}: {err:?}"
        );
    }
}
