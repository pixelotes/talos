//! The expression language of `calc.arithmetic`: a tokenizer, a
//! recursive-descent parser and an exact evaluator, written in-tree.
//!
//! Precedence, loosest to tightest: `+ -`, then `* /` (left to right), then a
//! unary minus, then `^` (right to left, so `2^3^2` is 512 and `-2^2` is
//! -4), then postfix `squared`/`²`, then atoms (numbers, parentheses, roots).
//! Spoken operators of both languages are accepted; number words go through
//! the engine's extractor.

use talos_core::extract;
use talos_core::normalise::fold_char;
use talos_core::Lang;

use super::numbers::{ReadError, canonical_number, echo_number, read_digits};
use super::rat::{MathError, Rat};

/// A value with its flag: approximate when a root that does not terminate
/// was involved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Val {
    pub(crate) value: Rat,
    pub(crate) approx: bool,
}

/// A number whose reading the user should see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Note {
    pub(crate) raw: String,
    pub(crate) value: Rat,
}

/// Why an expression has no value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Failure {
    /// Not a calculation at all: the rule's other candidates may take it.
    NotMath,
    /// An operator without its operand ("2 +").
    Incomplete,
    /// Two operands side by side ("2(2+2)"): never guessed.
    ImplicitMul,
    /// A number written with malformed separators.
    BadNumber(String),
    /// A number with more than 100 digits.
    TooBig,
    /// The calculation has no result.
    Math(MathError),
    /// The square root of a negative number; carries the radicand.
    NoRealRoot(Rat),
}

impl From<MathError> for Failure {
    fn from(error: MathError) -> Self {
        Failure::Math(error)
    }
}

/// A parsed expression, ready to evaluate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parsed {
    node: Node,
    /// The expression in normalised form, in the turn's language.
    pub(crate) echo: String,
    /// The expression as the `expression` slot reports it (`(3+4)*5`).
    pub(crate) canonical: String,
    pub(crate) notes: Vec<Note>,
}

impl Parsed {
    pub(crate) fn value(&self) -> Result<Val, Failure> {
        eval(&self.node)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Num(Rat),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Squared,
    Cubed,
    Root { index: u32, call: bool },
    Percent,
    Stop,
    Word(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Num(Rat),
    Neg(Box<Node>),
    Add(Box<Node>, Box<Node>),
    Sub(Box<Node>, Box<Node>),
    Mul(Box<Node>, Box<Node>),
    Div(Box<Node>, Box<Node>),
    Pow(Box<Node>, Box<Node>),
    Root(u32, Box<Node>),
    Paren(Box<Node>),
}

/// Longest expression read: what a person types, not a stress test.
const MAX_EXPRESSION_CHARS: usize = 1000;

/// Parses the text of an `expression` slot.
pub(crate) fn parse(text: &str, lang: Lang) -> Result<Parsed, Failure> {
    if text.chars().count() > MAX_EXPRESSION_CHARS {
        return Err(Failure::TooBig);
    }
    // "2x+3=7" is algebra; a trailing "=" or "=?" only asks for the value.
    if let Some((_, after)) = text.split_once('=')
        && !after.trim().trim_matches('?').trim().is_empty()
    {
        return Err(Failure::NotMath);
    }
    let text = repair_parens(text);
    let mut notes = Vec::new();
    let tokens = tokenize(&text, lang, &mut notes)?;
    if tokens.contains(&Tok::Percent) || !tokens.iter().any(|t| matches!(t, Tok::Num(_))) {
        return Err(Failure::NotMath);
    }
    let mut parser = Parser {
        tokens,
        pos: 0,
        depth: 0,
    };
    let node = parser.expression()?;
    if matches!(
        parser.peek(),
        Some(Tok::Num(_) | Tok::LParen | Tok::Root { .. })
    ) {
        return Err(Failure::ImplicitMul);
    }
    if !has_operation(&node) {
        return Err(Failure::NotMath);
    }
    Ok(Parsed {
        echo: echo(&node, lang, false),
        canonical: canonical(&node),
        node,
        notes,
    })
}

/// Parses and evaluates.
#[cfg(test)]
pub(crate) fn evaluate(text: &str, lang: Lang) -> Result<Evaluated, Failure> {
    let parsed = parse(text, lang)?;
    let value = parsed.value()?;
    Ok(Evaluated {
        echo: parsed.echo,
        canonical: parsed.canonical,
        value,
        notes: parsed.notes,
    })
}

/// A parsed, evaluated expression.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Evaluated {
    pub(crate) echo: String,
    pub(crate) canonical: String,
    pub(crate) value: Val,
    pub(crate) notes: Vec<Note>,
}

/// The slot text had its edge parentheses trimmed by the term extractor
/// ("(1+2)*(3+4)" arrives as "1+2)*(3+4"); put back the minimum.
pub(crate) fn repair_parens(text: &str) -> String {
    let (mut depth, mut missing_open) = (0_usize, 0_usize);
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            ')' => missing_open += 1,
            _ => {}
        }
    }
    format!("{}{text}{}", "(".repeat(missing_open), ")".repeat(depth))
}

// --- Tokens ---------------------------------------------------------------------

fn tokenize(text: &str, lang: Lang, notes: &mut Vec<Note>) -> Result<Vec<Tok>, Failure> {
    let chars: Vec<char> = text
        .chars()
        .map(|c| {
            let lower = c.to_lowercase().next().unwrap_or(c);
            match fold_char(lower) {
                '×' | '·' => '*',
                '÷' => '/',
                '−' | '–' | '—' | '‒' => '-',
                other => other,
            }
        })
        .collect();
    let mut raw = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() {
                let d = chars[i];
                let followed_by_digit = chars.get(i + 1).is_some_and(char::is_ascii_digit);
                if d.is_ascii_digit() || ((d == '.' || d == ',') && followed_by_digit) {
                    i += 1;
                } else {
                    break;
                }
            }
            let sign = chars.get(i + 1).copied();
            let exponent = chars.get(i) == Some(&'e')
                && (sign.is_some_and(|s| s.is_ascii_digit())
                    || (matches!(sign, Some('+' | '-'))
                        && chars.get(i + 2).is_some_and(char::is_ascii_digit)));
            if exponent {
                i += 2;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let token: String = chars[start..i].iter().collect();
            raw.push(Tok::Num(read_number(&token, lang, notes)?));
        } else if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && chars[i].is_alphabetic() {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // "x" between operands is a times sign; anywhere else it is a
            // variable, which this calculator does not do.
            if word == "x" && x_is_times(&chars, i) {
                raw.push(Tok::Star);
            } else {
                raw.push(Tok::Word(word));
            }
        } else {
            i += 1;
            match c {
                '+' => raw.push(Tok::Plus),
                '-' => {
                    // "twenty-one": a hyphen inside a word is not a minus.
                    let inside_word = i >= 2
                        && chars[i - 2].is_alphabetic()
                        && chars.get(i).is_some_and(|n| n.is_alphabetic());
                    if !inside_word {
                        raw.push(Tok::Minus);
                    }
                }
                '*' if chars.get(i) == Some(&'*') => {
                    i += 1;
                    raw.push(Tok::Caret);
                }
                '*' => raw.push(Tok::Star),
                '/' => raw.push(Tok::Slash),
                '^' => raw.push(Tok::Caret),
                '(' => raw.push(Tok::LParen),
                ')' => raw.push(Tok::RParen),
                '²' => raw.push(Tok::Squared),
                '³' => raw.push(Tok::Cubed),
                '%' => raw.push(Tok::Percent),
                '=' | '.' | ',' | ';' | '?' | '!' | '¿' | '¡' => raw.push(Tok::Stop),
                other => raw.push(Tok::Word(other.to_string())),
            }
        }
    }
    merge_words(&raw, lang)
}

/// `x` is a times sign when what follows is a number or an opening
/// parenthesis (optionally signed).
fn x_is_times(chars: &[char], after: usize) -> bool {
    let spaced = chars[after..].first().is_some_and(|c| c.is_whitespace());
    let rest: Vec<char> = chars[after..]
        .iter()
        .copied()
        .skip_while(|c| c.is_whitespace())
        .collect();
    match rest.as_slice() {
        [c, ..] if c.is_ascii_digit() || *c == '(' => true,
        // "3 x -2", but not the algebra of "2x+3".
        [s, c, ..] if spaced && matches!(s, '-' | '+') && (c.is_ascii_digit() || *c == '(') => {
            true
        }
        _ => false,
    }
}

fn read_number(token: &str, lang: Lang, notes: &mut Vec<Note>) -> Result<Rat, Failure> {
    match read_digits(token, lang) {
        Ok(read) => {
            if read.unforced && !notes.iter().any(|n| n.raw == token) {
                notes.push(Note {
                    raw: token.to_owned(),
                    value: read.value.clone(),
                });
            }
            Ok(read.value)
        }
        Err(ReadError::Malformed) => Err(Failure::BadNumber(token.to_owned())),
        Err(ReadError::TooLarge) => Err(Failure::TooBig),
    }
}

/// Words that are not numbers or operators stay `Word`s and end the
/// expression; operator phrases and number-word runs become tokens.
fn merge_words(raw: &[Tok], lang: Lang) -> Result<Vec<Tok>, Failure> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if !matches!(raw[i], Tok::Word(_)) {
            out.push(raw[i].clone());
            i += 1;
            continue;
        }
        if let Some((token, used)) = phrase(raw, i) {
            out.push(token);
            i += used;
        } else if let Some((value, used)) = number_run(raw, i, lang) {
            out.push(Tok::Num(value?));
            i += used;
        } else {
            out.push(raw[i].clone());
            i += 1;
        }
    }
    Ok(out)
}

fn word_at(toks: &[Tok], i: usize) -> &str {
    match toks.get(i) {
        Some(Tok::Word(w)) => w,
        _ => "",
    }
}

/// The operator a spoken phrase starting at `i` stands for, and how many
/// tokens it uses. Both languages are accepted.
fn phrase(toks: &[Tok], i: usize) -> Option<(Tok, usize)> {
    let w = |k: usize| word_at(toks, i + k);
    let root = |index, used, call: bool| Some((Tok::Root { index, call }, used));
    let next_is_paren = matches!(toks.get(i + 1), Some(Tok::LParen));
    match (w(0), w(1), w(2), w(3)) {
        ("raiz", "cuadrada", "de", _) | ("square", "root", "of", _) => root(2, 3, false),
        ("raiz", "cubica", "de", _) | ("cube", "root", "of", _) => root(3, 3, false),
        ("raiz", "de", _, _) | ("sqrt", "of", _, _) => root(2, 2, false),
        ("raiz" | "sqrt", _, _, _) if next_is_paren => root(2, 1, true),
        ("cbrt", "of", _, _) => root(3, 2, false),
        ("cbrt", _, _, _) if next_is_paren => root(3, 1, true),
        ("dividido", "por" | "entre", _, _) | ("divided", "by", _, _) => Some((Tok::Slash, 2)),
        ("multiplicado", "por", _, _) | ("multiplied", "by", _, _) => Some((Tok::Star, 2)),
        ("elevado", "al", "cuadrado", _) => Some((Tok::Squared, 3)),
        ("elevado", "al", "cubo", _) => Some((Tok::Cubed, 3)),
        ("elevado", "a", "la", _) => Some((Tok::Caret, 3)),
        ("elevado", "a", _, _) | ("raised", "to", _, _) => {
            let skip = if w(2) == "the" && w(3) == "power" { 4 } else { 2 };
            let skip = if skip == 4 && word_at(toks, i + 4) == "of" { 5 } else { skip };
            Some((Tok::Caret, skip))
        }
        ("to", "the", "power", "of") => Some((Tok::Caret, 4)),
        ("to", "the", "power", _) => Some((Tok::Caret, 3)),
        ("al", "cuadrado", _, _) => Some((Tok::Squared, 2)),
        ("al", "cubo", _, _) => Some((Tok::Cubed, 2)),
        ("squared", _, _, _) => Some((Tok::Squared, 1)),
        ("cubed", _, _, _) => Some((Tok::Cubed, 1)),
        ("plus" | "mas", _, _, _) => Some((Tok::Plus, 1)),
        ("minus" | "menos", _, _, _) => Some((Tok::Minus, 1)),
        ("por", "ciento", _, _) => Some((Tok::Percent, 2)),
        ("times" | "por", _, _, _) => Some((Tok::Star, 1)),
        ("over" | "entre" | "dividido", _, _, _) => Some((Tok::Slash, 1)),
        _ => None,
    }
}

fn is_article(word: &str) -> bool {
    matches!(word, "a" | "an" | "un" | "una")
}

/// Whether `word` is a number word of `lang` that is not an article.
fn numeric_word(word: &str, lang: Lang) -> bool {
    !word.is_empty() && !is_article(word) && extract::number(word, lang).is_ok()
}

/// A run of number words ("dos mil quinientos", "two and a half million")
/// starting at `i`, read by the engine's extractor.
fn number_run(toks: &[Tok], i: usize, lang: Lang) -> Option<(Result<Rat, Failure>, usize)> {
    let numeric_at = |k: usize| numeric_word(word_at(toks, k), lang);
    let starts = |k: usize| numeric_at(k) || (is_article(word_at(toks, k)) && numeric_at(k + 1));
    if !starts(i) {
        return None;
    }
    let mut end = i + 1;
    loop {
        let w = word_at(toks, end);
        if w.is_empty() {
            break;
        }
        let connector = matches!((lang, w), (Lang::Es, "y") | (Lang::En, "and"));
        if (connector && starts(end + 1)) || (!connector && (numeric_at(end) || starts(end))) {
            end += 1;
        } else {
            break;
        }
    }
    let text = (i..end)
        .map(|k| word_at(toks, k))
        .collect::<Vec<_>>()
        .join(" ");
    let value = extract::number(&text, lang)
        .map(Rat::from_number)
        .map_err(|_| Failure::BadNumber(text));
    Some((value, end - i))
}

// --- Parser ---------------------------------------------------------------------

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
    /// Nesting of parentheses and unary signs being parsed.
    depth: usize,
}

/// Deepest nesting read; the parser and the evaluator recurse with it.
const MAX_DEPTH: usize = 60;

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// A missing operand: at the very start the text is not a calculation;
    /// later it is an incomplete one.
    fn missing(&self) -> Failure {
        if self.pos == 0 {
            Failure::NotMath
        } else {
            Failure::Incomplete
        }
    }

    fn expression(&mut self) -> Result<Node, Failure> {
        let mut left = self.term()?;
        loop {
            if self.eat(&Tok::Plus) {
                left = Node::Add(Box::new(left), Box::new(self.term()?));
            } else if self.eat(&Tok::Minus) {
                left = Node::Sub(Box::new(left), Box::new(self.term()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn term(&mut self) -> Result<Node, Failure> {
        let mut left = self.unary()?;
        loop {
            if self.eat(&Tok::Star) {
                left = Node::Mul(Box::new(left), Box::new(self.unary()?));
            } else if self.eat(&Tok::Slash) {
                left = Node::Div(Box::new(left), Box::new(self.unary()?));
            } else {
                return Ok(left);
            }
        }
    }

    fn unary(&mut self) -> Result<Node, Failure> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(Failure::TooBig);
        }
        let node = if self.eat(&Tok::Minus) {
            self.unary().map(|inner| Node::Neg(Box::new(inner)))
        } else if self.eat(&Tok::Plus) {
            self.unary()
        } else {
            self.power()
        };
        self.depth -= 1;
        node
    }

    fn power(&mut self) -> Result<Node, Failure> {
        let base = self.postfix()?;
        if self.eat(&Tok::Caret) {
            // Right to left, and the exponent may carry a sign: 2^-2.
            return Ok(Node::Pow(Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }

    fn postfix(&mut self) -> Result<Node, Failure> {
        let mut node = self.atom()?;
        loop {
            if self.eat(&Tok::Squared) {
                node = Node::Pow(Box::new(node), Box::new(Node::Num(Rat::from_int(2))));
            } else if self.eat(&Tok::Cubed) {
                node = Node::Pow(Box::new(node), Box::new(Node::Num(Rat::from_int(3))));
            } else {
                return Ok(node);
            }
        }
    }

    fn atom(&mut self) -> Result<Node, Failure> {
        match self.peek().cloned() {
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(Node::Num(n))
            }
            Some(Tok::LParen) => {
                self.pos += 1;
                let inner = self.expression()?;
                if self.eat(&Tok::RParen) {
                    Ok(Node::Paren(Box::new(inner)))
                } else {
                    Err(Failure::Incomplete)
                }
            }
            Some(Tok::Root { index, call }) => {
                self.pos += 1;
                if call {
                    if !self.eat(&Tok::LParen) {
                        return Err(Failure::Incomplete);
                    }
                    let inner = self.expression()?;
                    if !self.eat(&Tok::RParen) {
                        return Err(Failure::Incomplete);
                    }
                    Ok(Node::Root(index, Box::new(Node::Paren(Box::new(inner)))))
                } else {
                    Ok(Node::Root(index, Box::new(self.unary()?)))
                }
            }
            _ => Err(self.missing()),
        }
    }
}

fn has_operation(node: &Node) -> bool {
    match node {
        Node::Num(_) => false,
        Node::Neg(inner) | Node::Paren(inner) => has_operation(inner),
        _ => true,
    }
}

// --- Evaluation -----------------------------------------------------------------

fn eval(node: &Node) -> Result<Val, Failure> {
    let exact = |value: Rat| Val {
        value,
        approx: false,
    };
    match node {
        Node::Num(n) => Ok(exact(n.clone())),
        Node::Paren(inner) => eval(inner),
        Node::Neg(inner) => {
            let v = eval(inner)?;
            Ok(Val {
                value: v.value.neg(),
                approx: v.approx,
            })
        }
        Node::Add(a, b) | Node::Sub(a, b) | Node::Mul(a, b) | Node::Div(a, b) => {
            let (x, y) = (eval(a)?, eval(b)?);
            let value = match node {
                Node::Add(..) => x.value.add(&y.value)?,
                Node::Sub(..) => x.value.sub(&y.value)?,
                Node::Mul(..) => x.value.mul(&y.value)?,
                _ => x.value.div(&y.value)?,
            };
            Ok(Val {
                value: value.within_limit()?,
                approx: x.approx || y.approx,
            })
        }
        Node::Pow(base, exp) => {
            let (b, e) = (eval(base)?, eval(exp)?);
            power(&b, &e)
        }
        Node::Root(index, arg) => {
            let v = eval(arg)?;
            root(&v, *index)
        }
    }
}

fn root(v: &Val, index: u32) -> Result<Val, Failure> {
    if v.value.is_neg() {
        if index == 2 {
            return Err(Failure::NoRealRoot(v.value.clone()));
        }
        let positive = root(
            &Val {
                value: v.value.abs(),
                approx: v.approx,
            },
            index,
        )?;
        return Ok(Val {
            value: positive.value.neg(),
            approx: positive.approx,
        });
    }
    let (value, exact) = v.value.root(index)?;
    Ok(Val {
        value,
        approx: v.approx || !exact,
    })
}

fn power(base: &Val, exp: &Val) -> Result<Val, Failure> {
    let approx = base.approx || exp.approx;
    if exp.value.is_integer() {
        let value = base.value.powi(&exp.value)?.within_limit()?;
        return Ok(Val { value, approx });
    }
    // x^(p/2) and x^(p/3): a root, then an integer power.
    let Some((p, q)) = exp.value.small_fraction() else {
        return Err(MathError::Unsupported.into());
    };
    let rooted = root(base, q)?;
    let value = rooted.value.powi(&Rat::from_int(p))?.within_limit()?;
    Ok(Val {
        value,
        approx: approx || rooted.approx,
    })
}

// --- Writing the expression back ------------------------------------------------

fn is_small_int(node: &Node, n: i64) -> bool {
    matches!(node, Node::Num(r) if *r == Rat::from_int(n))
}

/// The expression as the reply echoes it: spaced operators, `×` and `÷`,
/// `²` and `³` for a square or a cube, the user's parentheses.
fn echo(node: &Node, lang: Lang, in_exponent: bool) -> String {
    let go = |n: &Node| echo(n, lang, false);
    match node {
        Node::Num(n) if in_exponent && n.is_integer() => canonical_number(n),
        Node::Num(n) => echo_number(n, lang, "\u{2212}").text,
        Node::Neg(inner) => format!("\u{2212}{}", go(inner)),
        Node::Paren(inner) => format!("({})", go(inner)),
        Node::Add(a, b) => format!("{} + {}", go(a), go(b)),
        Node::Sub(a, b) => format!("{} \u{2212} {}", go(a), go(b)),
        Node::Mul(a, b) => format!("{} × {}", go(a), go(b)),
        Node::Div(a, b) => format!("{} ÷ {}", go(a), go(b)),
        Node::Pow(base, exp) => {
            let atomic = matches!(**base, Node::Num(_) | Node::Paren(_) | Node::Root(..));
            let b = if atomic {
                go(base)
            } else {
                format!("({})", go(base))
            };
            if !in_exponent && is_small_int(exp, 2) {
                format!("{b}²")
            } else if !in_exponent && is_small_int(exp, 3) {
                format!("{b}³")
            } else {
                format!("{b}^{}", echo(exp, lang, true))
            }
        }
        Node::Root(index, arg) => {
            let sign = if *index == 2 { '√' } else { '∛' };
            let operand = match &**arg {
                Node::Paren(inner) if matches!(**inner, Node::Num(_)) => go(inner),
                Node::Paren(_) | Node::Num(_) | Node::Neg(_) => go(arg),
                other => format!("({})", go(other)),
            };
            format!("{sign}{operand}")
        }
    }
}

/// The expression as the slot reports it: no spaces, ASCII operators.
fn canonical(node: &Node) -> String {
    match node {
        Node::Num(n) => canonical_number(n),
        Node::Neg(inner) => format!("-{}", canonical(inner)),
        Node::Paren(inner) => format!("({})", canonical(inner)),
        Node::Add(a, b) => format!("{}+{}", canonical(a), canonical(b)),
        Node::Sub(a, b) => format!("{}-{}", canonical(a), canonical(b)),
        Node::Mul(a, b) => format!("{}*{}", canonical(a), canonical(b)),
        Node::Div(a, b) => format!("{}/{}", canonical(a), canonical(b)),
        Node::Pow(base, exp) => {
            let atomic = matches!(**base, Node::Num(_) | Node::Paren(_) | Node::Root(..));
            if atomic {
                format!("{}^{}", canonical(base), canonical(exp))
            } else {
                format!("({})^{}", canonical(base), canonical(exp))
            }
        }
        Node::Root(index, arg) => {
            let name = if *index == 2 { "sqrt" } else { "cbrt" };
            match &**arg {
                Node::Paren(inner) => format!("{name}({})", canonical(inner)),
                other => format!("{name}({})", canonical(other)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, lang: Lang) -> Result<(String, String, String), Failure> {
        let e = evaluate(text, lang)?;
        let shown = super::super::rat::show_significant(
            &e.value.value,
            10,
            e.value.approx,
            lang,
            "-",
        );
        Ok((
            e.canonical,
            format!("{}{}", if shown.approx { "~" } else { "" }, shown.text),
            e.echo,
        ))
    }

    fn value(text: &str) -> String {
        run(text, Lang::En).unwrap().1
    }

    fn canonical_of(text: &str, lang: Lang) -> String {
        run(text, lang).unwrap().0
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(value("2 + 3 * 4"), "14");
        assert_eq!(value("(2 + 3) * 4"), "20");
        assert_eq!(value("7 + 8 * 2"), "23");
        assert_eq!(value("10 - 4 - 3"), "3", "subtraction is left to right");
        assert_eq!(value("100 / 10 / 5"), "2", "division is left to right");
        assert_eq!(value("2 ^ 3 ^ 2"), "512", "powers are right to left");
        assert_eq!(value("-2^2"), "-4");
        assert_eq!(value("(-2)^2"), "4");
        assert_eq!(value("2^-2"), "0.25");
        assert_eq!(value("2 * -3"), "-6");
        assert_eq!(value("- -3 + 1"), "4");
        assert_eq!(value("-3 squared"), "-9");
        assert_eq!(value("2 * 3 ^ 2"), "18");
        assert_eq!(value("2 ** 10"), "1,024");
        assert_eq!(value("((1 + 2) * (3 + 4))"), "21");
        assert_eq!(value("1 - 2 * 3 + 4 / 2"), "-3");
    }

    #[test]
    fn exact_decimals_and_fractions() {
        assert_eq!(value("0.1 + 0.2"), "0.3");
        assert_eq!(value("1/3*3"), "1");
        assert_eq!(value("1/3 + 1/3 + 1/3"), "1");
        assert_eq!(value("0.1 * 3"), "0.3");
        assert_eq!(value("1 / 3"), "~0.3333333333");
        assert_eq!(value("2 / 3"), "~0.6666666667");
        assert_eq!(value("1 / 7"), "~0.1428571429");
        assert_eq!(value("22 / 7"), "~3.142857143");
        assert_eq!(value("10 / 4"), "2.5");
        assert_eq!(value("1 / 8"), "0.125");
        assert_eq!(value("123456789.123456789 + 1"), "~123,456,790.1");
        assert_eq!(value("0.000001 * 0.000001"), "0.000000000001");
        assert_eq!(value("99999999999 * 99999999999"), "9,999,999,999,800,000,000,001");
    }

    #[test]
    fn huge_values_are_exact_scientific_or_refused() {
        assert_eq!(value("2^64"), "18,446,744,073,709,551,616");
        assert_eq!(value("2^200"), "~1.606938044 × 10^60");
        assert_eq!(value("10^400"), "1 × 10^400");
        assert_eq!(value("10^1000"), "1 × 10^1000");
        assert_eq!(
            run("10^1001", Lang::En),
            Err(Failure::Math(MathError::TooLarge))
        );
        assert_eq!(run("9^9^9", Lang::En), Err(Failure::Math(MathError::TooLarge)));
        assert_eq!(
            run("10^1000 * 10", Lang::En),
            Err(Failure::Math(MathError::TooLarge))
        );
        assert_eq!(value("10^1000 / 10^999"), "10");
        assert_eq!(run("2^99999999999999999999", Lang::En), Err(Failure::Math(MathError::TooLarge)));
        let hundred = "9".repeat(100);
        assert!(run(&format!("{hundred} + 1"), Lang::En).is_ok());
        assert_eq!(
            run(&format!("{hundred}9 + 1"), Lang::En),
            Err(Failure::TooBig)
        );
        assert_eq!(value("1 / 10^400"), "1 × 10^-400");
    }

    #[test]
    fn no_result_cases() {
        assert_eq!(
            run("5 / 0", Lang::En),
            Err(Failure::Math(MathError::DivZero { zero_over_zero: false }))
        );
        assert_eq!(
            run("0/0", Lang::En),
            Err(Failure::Math(MathError::DivZero { zero_over_zero: true }))
        );
        assert_eq!(
            run("1 / (2 - 2)", Lang::En),
            Err(Failure::Math(MathError::DivZero { zero_over_zero: false }))
        );
        assert_eq!(
            run("0 ^ -1", Lang::En),
            Err(Failure::Math(MathError::DivZero { zero_over_zero: false }))
        );
        assert!(matches!(run("sqrt(-4)", Lang::En), Err(Failure::NoRealRoot(_))));
        assert!(matches!(run("raiz de -4", Lang::Es), Err(Failure::NoRealRoot(_))));
        assert_eq!(value("cube root of -27"), "-3");
        assert_eq!(
            run("2 ^ 0.7", Lang::En),
            Err(Failure::Math(MathError::Unsupported))
        );
    }

    #[test]
    fn roots_and_fractional_powers() {
        assert_eq!(value("sqrt(144)"), "12");
        assert_eq!(value("sqrt(2)"), "~1.414213562");
        assert_eq!(value("sqrt(2) * sqrt(2)"), "~2");
        assert_eq!(value("square root of 81"), "9");
        assert_eq!(value("raiz cuadrada de 144"), "12");
        assert_eq!(value("raiz cubica de 27"), "3");
        assert_eq!(value("sqrt(16) + 1"), "5");
        assert_eq!(value("sqrt(0.25)"), "0.5");
        assert_eq!(value("4 ^ 0.5"), "2");
        assert_eq!(value("8 ^ (1/3)"), "2");
        assert_eq!(value("9 ^ 1.5"), "27");
        assert_eq!(value("square root of 144 plus 25"), "37");
    }

    #[test]
    fn spoken_operators_in_both_languages() {
        let es = |t: &str| canonical_of(t, Lang::Es);
        let en = |t: &str| canonical_of(t, Lang::En);
        assert_eq!(es("tres por cuatro"), "3*4");
        assert_eq!(es("10 entre 4"), "10/4");
        assert_eq!(es("10 dividido por 4"), "10/4");
        assert_eq!(es("5 mas 3 menos 1"), "5+3-1");
        assert_eq!(es("2 elevado a 10"), "2^10");
        assert_eq!(es("9 al cuadrado"), "9^2");
        assert_eq!(es("3 al cubo"), "3^3");
        assert_eq!(es("raíz cuadrada de 144"), "sqrt(144)");
        assert_eq!(es("2 × 3 ÷ 4 − 1"), "2*3/4-1");
        assert_eq!(es("dos mil quinientos mas trescientos"), "2500+300");
        assert_eq!(es("veintiuno por tres"), "21*3");
        assert_eq!(es("un billón entre mil"), "1000000000000/1000");
        assert_eq!(en("three times four"), "3*4");
        assert_eq!(en("100 divided by 8"), "100/8");
        assert_eq!(en("2 to the power of 8"), "2^8");
        assert_eq!(en("9 squared"), "9^2");
        assert_eq!(en("27 cubed"), "27^3");
        assert_eq!(en("twenty-one times three"), "21*3");
        assert_eq!(en("one billion divided by a thousand"), "1000000000/1000");
        assert_eq!(en("two and a half million divided by five"), "2500000/5");
        assert_eq!(en("3 x 4"), "3*4");
        assert_eq!(es("3,5 x 2"), "3.5*2");
        assert_eq!(en("3 minus 2 over 4"), "3-2/4");
        assert_eq!(en("2 multiplied by 3"), "2*3");
        assert_eq!(en("2 raised to the power of 3"), "2^3");
    }

    #[test]
    fn the_echo_is_normalised() {
        let echo = |t: &str, l: Lang| run(t, l).unwrap().2;
        assert_eq!(echo("(3+4)*5", Lang::Es), "(3 + 4) × 5");
        assert_eq!(echo("-2^2", Lang::En), "\u{2212}2²");
        assert_eq!(echo("(-2)^2", Lang::En), "(\u{2212}2)²");
        assert_eq!(echo("2^3^2", Lang::En), "2^3^2");
        assert_eq!(echo("9 squared", Lang::En), "9²");
        assert_eq!(echo("sqrt(81)", Lang::En), "√81");
        assert_eq!(echo("sqrt(1 + 3)", Lang::En), "√(1 + 3)");
        assert_eq!(echo("10 / 4", Lang::Es), "10 ÷ 4");
        assert_eq!(echo("(1,250 + 750) / 4", Lang::En), "(1,250 + 750) ÷ 4");
        assert_eq!(echo("3.5 + 1", Lang::Es), "3,5 + 1");
        assert_eq!(echo("9 squared squared", Lang::En), "(9²)²");
        assert_eq!(echo("2^10", Lang::Es), "2^10");
    }

    #[test]
    fn parentheses_trimmed_by_the_term_extractor_are_restored() {
        assert_eq!(repair_parens("1+2)*(3+4"), "(1+2)*(3+4)");
        assert_eq!(repair_parens("3 + 4) * 5"), "(3 + 4) * 5");
        assert_eq!(repair_parens("2 * (3 + 4"), "2 * (3 + 4)");
        assert_eq!(repair_parens("1 + 2"), "1 + 2");
        assert_eq!(value("1+2)*(3+4"), "21");
    }

    #[test]
    fn malformed_and_non_math_input() {
        assert_eq!(run("2 +", Lang::En), Err(Failure::Incomplete));
        assert_eq!(run("2 * * 3", Lang::En), Err(Failure::Incomplete));
        assert_eq!(run("(2 + 3", Lang::En).map(|r| r.1), Ok("5".to_owned()));
        assert_eq!(run("8 / 2(2+2)", Lang::En), Err(Failure::ImplicitMul));
        assert_eq!(run("2 3", Lang::En), Err(Failure::ImplicitMul));
        assert_eq!(run("hello there", Lang::En), Err(Failure::NotMath));
        assert_eq!(run("42", Lang::En), Err(Failure::NotMath));
        assert_eq!(run("5 km", Lang::En), Err(Failure::NotMath));
        assert_eq!(run("50 + 10%", Lang::En), Err(Failure::NotMath));
        assert_eq!(run("2x+3", Lang::En), Err(Failure::NotMath));
        assert_eq!(run("pi * 2", Lang::En), Err(Failure::NotMath));
        assert_eq!(
            run("1,500,000 + 1", Lang::Es),
            Err(Failure::BadNumber("1,500,000".to_owned()))
        );
        assert_eq!(run("()", Lang::En), Err(Failure::NotMath));
        assert_eq!(run("", Lang::En), Err(Failure::NotMath));
    }

    #[test]
    fn absurd_input_is_refused_not_computed() {
        let long = format!("1{}", " + 1".repeat(400));
        assert_eq!(run(&long, Lang::En), Err(Failure::TooBig));
        let nested = |n: usize| format!("{}1{} + 1", "(".repeat(n), ")".repeat(n));
        assert_eq!(run(&nested(20), Lang::En).map(|r| r.1), Ok("2".to_owned()));
        assert_eq!(run(&nested(300), Lang::En), Err(Failure::TooBig));
        assert_eq!(value(&format!("{}1 + 0", "-".repeat(40))), "1");
        assert_eq!(run(&format!("{}1", "-".repeat(400)), Lang::En), Err(Failure::TooBig));
        let chain = format!("1{}", " + 1".repeat(240));
        assert_eq!(value(&chain), "241", "a long flat chain is fine");
        assert_eq!(value("2^3^2^1"), "512");
    }

    #[test]
    fn trailing_words_are_ignored_and_the_first_expression_wins() {
        assert_eq!(value("2+2 is 5, right"), "4");
        assert_eq!(value("2+2 es 5"), "4");
        assert_eq!(value("3 times 4 in miles"), "12");
        assert_eq!(value("2 + 2 and 3 + 3"), "4");
        assert_eq!(value("2+2=?"), "4");
        assert_eq!(value("2+2, 3+3"), "4");
    }

    #[test]
    fn readings_that_were_not_forced_are_noted() {
        let notes = |t: &str, l: Lang| {
            evaluate(t, l)
                .unwrap()
                .notes
                .into_iter()
                .map(|n| n.raw)
                .collect::<Vec<_>>()
        };
        assert_eq!(notes("1.000 + 1", Lang::Es), ["1.000"]);
        assert_eq!(notes("1,000 + 1", Lang::Es), ["1,000"]);
        assert_eq!(notes("1,000 + 1", Lang::En), Vec::<String>::new());
        assert_eq!(notes("1,5 + 1", Lang::En), ["1,5"]);
        assert_eq!(notes("3.5 + 1", Lang::Es), Vec::<String>::new());
        assert_eq!(notes("1.000,50 + 1", Lang::Es), Vec::<String>::new());
        assert_eq!(notes("1.000 + 1.000 + 2,000", Lang::Es), ["1.000", "2,000"]);
    }

    #[test]
    fn number_words_and_digits_mix() {
        assert_eq!(value("two thousand plus 500"), "2,500");
        assert_eq!(canonical_of("mil quinientos mas 250", Lang::Es), "1500+250");
        assert_eq!(canonical_of("1.500 mas 250", Lang::Es), "1500+250");
        assert_eq!(canonical_of("1,5 por 4", Lang::Es), "1.5*4");
        assert_eq!(value("1,500 plus 250"), "1,750");
        assert_eq!(value("1.5 times 4"), "6");
    }
}
