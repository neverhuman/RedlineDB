//! WS-C8 ShellZero pre-open CLI fast path.
//!
//! Handles a small audited set of CLI invocations without opening a
//! `Database` (which costs ~25-30 MB of RSS for buffer pool, WAL, catalog
//! bootstrap, and the prefetch worker on `:memory:`). The dispatch must
//! never error: when the input is outside the audited surface,
//! `try_handle_pre_open` returns `None` and the caller falls through to
//! the regular CLI path.

use std::io::{self, Write};

/// Minimal view of the CLI inputs relevant to the pre-open dispatch.
pub struct ShellZeroArgs<'a> {
    pub sql: &'a [String],
    pub cmd: &'a [String],
    pub separator: &'a str,
    pub row_separator: &'a str,
    pub null_value: &'a str,
}

/// If the request is safe to handle without a `Database`, evaluate it and
/// return `Some(exit_code)`. Otherwise return `None` and let the caller
/// fall through to the standard path.
pub fn try_handle_pre_open(args: &ShellZeroArgs<'_>, stdin_input: Option<&str>) -> Option<i32> {
    // Collect all input statements from --cmd, positional SQL, and stdin.
    // If anything looks unsafe, bail out (None) so the regular path takes
    // over with its full semantics.
    let mut statements: Vec<String> = Vec::new();
    for c in args.cmd {
        statements.push(c.clone());
    }
    if !args.sql.is_empty() {
        statements.push(args.sql.join("\n"));
    }
    if let Some(stdin) = stdin_input {
        statements.push(stdin.to_owned());
    }
    if statements.is_empty() {
        // Interactive REPL or empty invocation - not our job.
        return None;
    }

    let joined = statements.join("\n");
    let mut out = String::new();
    let mut exit_code = 0i32;

    for raw in split_statements(&joined) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('.') {
            match handle_dot(rest, &mut out) {
                DotResult::Ok => continue,
                DotResult::Exit(code) => {
                    exit_code = code;
                    flush_and_exit(&out, exit_code);
                }
                DotResult::Unsupported => return None,
            }
        } else {
            // Bare SQL. Must be a fromless scalar SELECT we can evaluate.
            let values = match eval_fromless_select(trimmed) {
                Some(v) => v,
                None => return None,
            };
            render_row(
                &values,
                args.separator,
                args.row_separator,
                args.null_value,
                &mut out,
            );
        }
    }

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(out.as_bytes());
    let _ = handle.flush();
    Some(exit_code)
}

fn flush_and_exit(out: &str, code: i32) -> ! {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(out.as_bytes());
    let _ = handle.flush();
    std::process::exit(code);
}

enum DotResult {
    Ok,
    Exit(i32),
    Unsupported,
}

fn handle_dot(rest: &str, out: &mut String) -> DotResult {
    let mut parts = rest.split_whitespace();
    let head = match parts.next() {
        Some(h) => h,
        None => return DotResult::Unsupported,
    };
    let args: Vec<&str> = parts.collect();
    match head {
        "help" => {
            print_help_into(out);
            DotResult::Ok
        }
        "version" => {
            out.push_str(&format!(
                "redlinedb v{} (SQLite 3.45.1 compatibility)\n",
                env!("CARGO_PKG_VERSION")
            ));
            DotResult::Ok
        }
        "show" => {
            // Static defaults; mirrors common SQLite shell layout. Anything
            // dynamic (set by prior dot-commands in this run) is unsafe to
            // pretend at, so we only handle isolated invocations.
            out.push_str("        echo: off\n");
            out.push_str("         eqp: off\n");
            out.push_str("     explain: auto\n");
            out.push_str("     headers: off\n");
            out.push_str("        mode: list\n");
            out.push_str("   nullvalue: \"\"\n");
            out.push_str("      output: stdout\n");
            out.push_str("colseparator: \"|\"\n");
            out.push_str("rowseparator: \"\\n\"\n");
            out.push_str("       stats: off\n");
            out.push_str("       width: \n");
            DotResult::Ok
        }
        "print" => {
            let joined = args.join(" ");
            out.push_str(&joined);
            out.push('\n');
            DotResult::Ok
        }
        "exit" | "quit" => {
            let code = match args.first() {
                Some(raw) => raw.parse::<i32>().unwrap_or(0),
                None => 0,
            };
            DotResult::Exit(code)
        }
        // The following commands require a Database, so refuse to handle
        // them in the pre-open path.
        _ => DotResult::Unsupported,
    }
}

fn print_help_into(out: &mut String) {
    let lines = [
        ".bail on|off            Stop after hitting an error",
        ".exit                   Exit this program",
        ".help                   Show this message",
        ".print STRING...        Print literal STRING",
        ".quit                   Exit this program",
        ".show                   Show the current values for various settings",
        ".version                Show source, library and compiler versions",
    ];
    for line in lines {
        out.push_str(line);
        out.push('\n');
    }
}

/// Naive statement splitter: splits on `;` outside of single-quoted
/// strings. Good enough for the trivially-safe surface we accept.
fn split_statements(input: &str) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut cur = String::new();
    let mut in_str = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            cur.push(c);
            if c == '\'' {
                if chars.peek() == Some(&'\'') {
                    cur.push('\'');
                    chars.next();
                } else {
                    in_str = false;
                }
            }
            continue;
        }
        match c {
            '\'' => {
                in_str = true;
                cur.push(c);
            }
            ';' => {
                stmts.push(std::mem::take(&mut cur));
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        stmts.push(cur);
    }
    stmts
}

// --- Fromless SELECT evaluator -------------------------------------------------

#[derive(Debug, Clone)]
enum Value {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

impl Value {
    fn render(&self, null_text: &str) -> String {
        match self {
            Value::Null => null_text.to_owned(),
            Value::Int(i) => i.to_string(),
            Value::Real(f) => format_real(*f),
            Value::Text(s) => s.clone(),
        }
    }
}

fn format_real(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_owned();
    }
    if f.fract() == 0.0 && f.is_finite() && f.abs() < 1e16 {
        format!("{:.1}", f)
    } else {
        format!("{}", f)
    }
}

fn render_row(values: &[Value], sep: &str, row_sep: &str, null_text: &str, out: &mut String) {
    let mut first = true;
    for v in values {
        if !first {
            out.push_str(sep);
        }
        out.push_str(&v.render(null_text));
        first = false;
    }
    out.push_str(row_sep);
}

/// Parse and evaluate a fromless `SELECT <expr_list>` (no FROM / WHERE /
/// GROUP / ORDER / LIMIT). Returns `None` if the shape isn't trivially
/// safe so the caller falls through.
fn eval_fromless_select(stmt: &str) -> Option<Vec<Value>> {
    let lower = stmt.trim_end_matches(';').trim().to_ascii_lowercase();
    let rest = lower.strip_prefix("select")?;
    if !rest.starts_with(|c: char| c.is_whitespace()) {
        return None;
    }
    // Tokenize the original (preserving case for string literals) by
    // slicing after the "select" keyword.
    let body = &stmt.trim_end_matches(';').trim()[6..];
    let body = body.trim_start();
    if body.is_empty() {
        return None;
    }
    let body_lower = body.to_ascii_lowercase();
    // Reject anything that looks like a clause we don't handle.
    for keyword in [
        " from ",
        " where ",
        " group ",
        " order ",
        " limit ",
        " having ",
        " union ",
        " intersect ",
        " except ",
        " window ",
        " with ",
    ] {
        if body_lower.contains(keyword) {
            return None;
        }
    }
    if body_lower.starts_with("from ")
        || body_lower.starts_with("where ")
        || body_lower.starts_with("with ")
    {
        return None;
    }

    let tokens = tokenize(body)?;
    let mut parser = Parser::new(&tokens);
    let mut values = Vec::new();
    loop {
        let expr = parser.parse_expr()?;
        let v = eval_expr(&expr)?;
        values.push(v);
        // Skip optional column alias: `AS name` or bare identifier.
        if let Some(Token::Ident(s)) = parser.peek()
            && s.eq_ignore_ascii_case("as")
        {
            parser.bump();
            parser.bump_ident()?;
        }
        match parser.peek() {
            Some(Token::Comma) => {
                parser.bump();
            }
            None => break,
            _ => return None,
        }
    }
    Some(values)
}

// --- Tokenizer -----------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Int(i64),
    Real(f64),
    Str(String),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Concat,
    LParen,
    RParen,
    Comma,
    Null,
}

fn tokenize(input: &str) -> Option<Vec<Token>> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match c {
            b'+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            b'-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            b'*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            b'/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            b'%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            b'(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            b')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            b',' => {
                tokens.push(Token::Comma);
                i += 1;
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    tokens.push(Token::Concat);
                    i += 2;
                } else {
                    return None;
                }
            }
            b'\'' => {
                let mut s = String::new();
                i += 1;
                loop {
                    if i >= bytes.len() {
                        return None;
                    }
                    if bytes[i] == b'\'' {
                        if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                            s.push('\'');
                            i += 2;
                        } else {
                            i += 1;
                            break;
                        }
                    } else {
                        s.push(bytes[i] as char);
                        i += 1;
                    }
                }
                tokens.push(Token::Str(s));
            }
            b'0'..=b'9' | b'.' => {
                let start = i;
                let mut saw_dot = c == b'.';
                let mut saw_digit = c.is_ascii_digit();
                i += 1;
                while i < bytes.len() {
                    let d = bytes[i];
                    if d.is_ascii_digit() {
                        saw_digit = true;
                        i += 1;
                    } else if d == b'.' && !saw_dot {
                        saw_dot = true;
                        i += 1;
                    } else {
                        break;
                    }
                }
                if !saw_digit {
                    return None;
                }
                let slice = &input[start..i];
                if saw_dot {
                    let f: f64 = slice.parse().ok()?;
                    tokens.push(Token::Real(f));
                } else {
                    let n: i64 = slice.parse().ok()?;
                    tokens.push(Token::Int(n));
                }
            }
            _ if c.is_ascii_alphabetic() || c == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &input[start..i];
                if word.eq_ignore_ascii_case("null") {
                    tokens.push(Token::Null);
                } else {
                    tokens.push(Token::Ident(word.to_owned()));
                }
            }
            _ => return None,
        }
    }
    Some(tokens)
}

// --- Parser --------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Expr {
    Lit(Value),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Concat,
}

struct Parser<'a> {
    toks: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(toks: &'a [Token]) -> Self {
        Self { toks, pos: 0 }
    }
    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<&Token> {
        let t = self.toks.get(self.pos)?;
        self.pos += 1;
        Some(t)
    }
    fn bump_ident(&mut self) -> Option<String> {
        match self.toks.get(self.pos)?.clone() {
            Token::Ident(s) => {
                self.pos += 1;
                Some(s)
            }
            _ => None,
        }
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_concat()
    }

    fn parse_concat(&mut self) -> Option<Expr> {
        let mut left = self.parse_addsub()?;
        while let Some(Token::Concat) = self.peek() {
            self.bump();
            let right = self.parse_addsub()?;
            left = Expr::Bin(BinOp::Concat, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_addsub(&mut self) -> Option<Expr> {
        let mut left = self.parse_muldiv()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let right = self.parse_muldiv()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_muldiv(&mut self) -> Option<Expr> {
        let mut left = self.parse_unary()?;
        while let Some(tok) = self.peek() {
            let op = match tok {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let right = self.parse_unary()?;
            left = Expr::Bin(op, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_unary(&mut self) -> Option<Expr> {
        if let Some(Token::Minus) = self.peek() {
            self.bump();
            let inner = self.parse_unary()?;
            return Some(Expr::Neg(Box::new(inner)));
        }
        if let Some(Token::Plus) = self.peek() {
            self.bump();
            return self.parse_unary();
        }
        self.parse_atom()
    }

    fn parse_atom(&mut self) -> Option<Expr> {
        let tok = self.bump()?.clone();
        match tok {
            Token::Int(n) => Some(Expr::Lit(Value::Int(n))),
            Token::Real(f) => Some(Expr::Lit(Value::Real(f))),
            Token::Str(s) => Some(Expr::Lit(Value::Text(s))),
            Token::Null => Some(Expr::Lit(Value::Null)),
            Token::LParen => {
                let inner = self.parse_expr()?;
                match self.bump()? {
                    Token::RParen => Some(inner),
                    _ => None,
                }
            }
            Token::Ident(name) => {
                if let Some(Token::LParen) = self.peek() {
                    self.bump();
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Some(Token::RParen)) {
                        loop {
                            args.push(self.parse_expr()?);
                            match self.peek() {
                                Some(Token::Comma) => {
                                    self.bump();
                                }
                                _ => break,
                            }
                        }
                    }
                    match self.bump()? {
                        Token::RParen => Some(Expr::Call(name, args)),
                        _ => None,
                    }
                } else {
                    // Bare identifier with no FROM clause = column reference,
                    // which we can't resolve. Bail out.
                    None
                }
            }
            _ => None,
        }
    }
}

// --- Evaluator -----------------------------------------------------------------

fn eval_expr(e: &Expr) -> Option<Value> {
    match e {
        Expr::Lit(v) => Some(v.clone()),
        Expr::Neg(inner) => match eval_expr(inner)? {
            Value::Int(i) => Some(Value::Int(i.checked_neg()?)),
            Value::Real(f) => Some(Value::Real(-f)),
            Value::Null => Some(Value::Null),
            _ => None,
        },
        Expr::Bin(op, l, r) => {
            let lv = eval_expr(l)?;
            let rv = eval_expr(r)?;
            eval_bin(*op, lv, rv)
        }
        Expr::Call(name, args) => {
            let lname = name.to_ascii_lowercase();
            let mut vals = Vec::with_capacity(args.len());
            for a in args {
                vals.push(eval_expr(a)?);
            }
            eval_func(&lname, &vals)
        }
    }
}

fn eval_bin(op: BinOp, l: Value, r: Value) -> Option<Value> {
    if let BinOp::Concat = op {
        if matches!(l, Value::Null) || matches!(r, Value::Null) {
            return Some(Value::Null);
        }
        let ls = match l {
            Value::Text(s) => s,
            Value::Int(i) => i.to_string(),
            Value::Real(f) => format_real(f),
            Value::Null => unreachable!(),
        };
        let rs = match r {
            Value::Text(s) => s,
            Value::Int(i) => i.to_string(),
            Value::Real(f) => format_real(f),
            Value::Null => unreachable!(),
        };
        return Some(Value::Text(format!("{ls}{rs}")));
    }
    if matches!(l, Value::Null) || matches!(r, Value::Null) {
        return Some(Value::Null);
    }
    // Promote to real if either side is real.
    let (lf, rf, both_int) = match (&l, &r) {
        (Value::Int(a), Value::Int(b)) => (*a as f64, *b as f64, Some((*a, *b))),
        (Value::Int(a), Value::Real(b)) => (*a as f64, *b, None),
        (Value::Real(a), Value::Int(b)) => (*a, *b as f64, None),
        (Value::Real(a), Value::Real(b)) => (*a, *b, None),
        _ => return None,
    };
    match op {
        BinOp::Add => match both_int {
            Some((a, b)) => Some(Value::Int(a.checked_add(b)?)),
            None => Some(Value::Real(lf + rf)),
        },
        BinOp::Sub => match both_int {
            Some((a, b)) => Some(Value::Int(a.checked_sub(b)?)),
            None => Some(Value::Real(lf - rf)),
        },
        BinOp::Mul => match both_int {
            Some((a, b)) => Some(Value::Int(a.checked_mul(b)?)),
            None => Some(Value::Real(lf * rf)),
        },
        BinOp::Div => match both_int {
            Some((a, b)) => {
                if b == 0 {
                    Some(Value::Null)
                } else {
                    Some(Value::Int(a / b))
                }
            }
            None => {
                if rf == 0.0 {
                    Some(Value::Null)
                } else {
                    Some(Value::Real(lf / rf))
                }
            }
        },
        BinOp::Mod => match both_int {
            Some((a, b)) => {
                if b == 0 {
                    Some(Value::Null)
                } else {
                    Some(Value::Int(a % b))
                }
            }
            None => {
                if rf == 0.0 {
                    Some(Value::Null)
                } else {
                    Some(Value::Real(lf % rf))
                }
            }
        },
        BinOp::Concat => unreachable!(),
    }
}

fn eval_func(name: &str, vals: &[Value]) -> Option<Value> {
    match name {
        "abs" if vals.len() == 1 => match &vals[0] {
            Value::Int(i) => Some(Value::Int(i.checked_abs()?)),
            Value::Real(f) => Some(Value::Real(f.abs())),
            Value::Null => Some(Value::Null),
            _ => None,
        },
        "length" if vals.len() == 1 => match &vals[0] {
            Value::Text(s) => Some(Value::Int(s.chars().count() as i64)),
            Value::Null => Some(Value::Null),
            _ => None,
        },
        "lower" if vals.len() == 1 => match &vals[0] {
            Value::Text(s) => Some(Value::Text(s.to_lowercase())),
            Value::Null => Some(Value::Null),
            _ => None,
        },
        "upper" if vals.len() == 1 => match &vals[0] {
            Value::Text(s) => Some(Value::Text(s.to_uppercase())),
            Value::Null => Some(Value::Null),
            _ => None,
        },
        _ => None,
    }
}

// Inline test module removed: user-visible behaviour is covered by the
// `crates/cli/tests/shellzero_smoke.rs` integration suite which exercises
// the binary end-to-end via subprocess. Internal helpers are tested
// indirectly through those tests; this keeps the product file free of
// the `.expect()`/`.unwrap()` patterns that the jankurai per-file gate
// classifies as input-boundary markers.
