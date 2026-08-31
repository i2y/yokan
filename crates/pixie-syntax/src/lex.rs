//! Hand-written lexer for pixie.
//!
//! Produces a `Vec<Token>` covering the whole source plus a final `Eof`.
//! Errors short-circuit rather than recover - the parser's job is recovery.

use crate::span::{FileId, Span};
use crate::token::{StrSeg, Token, TokenKind};

#[derive(Debug)]
pub struct LexError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {:?}", self.message, self.span)
    }
}

impl std::error::Error for LexError {}

pub fn lex(file: FileId, src: &str) -> Result<Vec<Token>, LexError> {
    let mut lx = Lexer {
        file,
        src,
        pos: 0,
        out: Vec::new(),
    };
    lx.run()?;
    Ok(lx.out)
}

struct Lexer<'a> {
    file: FileId,
    src: &'a str,
    pos: usize,
    out: Vec<Token>,
}

impl<'a> Lexer<'a> {
    fn run(&mut self) -> Result<(), LexError> {
        while self.pos < self.src.len() {
            self.skip_trivia();
            if self.pos >= self.src.len() {
                break;
            }
            let start = self.pos;
            let ch = self.peek().unwrap();

            if ch == '\n' {
                self.pos += 1;
                self.push(TokenKind::Newline, start);
                continue;
            }

            if ch.is_ascii_digit() {
                self.lex_number(start)?;
                continue;
            }

            if ch == '"' {
                self.lex_string(start)?;
                continue;
            }

            if ch == '@' {
                self.pos += 1;
                let name = self.lex_ident_body();
                if name.is_empty() {
                    return Err(self.err(start, "expected identifier after `@`"));
                }
                self.push(TokenKind::AtIdent(name), start);
                continue;
            }

            if ch == ':' {
                // `:` could be `::`, `:` (type annotation / hash key), or `:foo` symbol.
                if self.peek_at(1) == Some(':') {
                    self.pos += 2;
                    self.push(TokenKind::DoubleColon, start);
                    continue;
                }
                let next = self.peek_at(1);
                if next.map_or(false, |c| c == '_' || c.is_ascii_alphabetic()) {
                    self.pos += 1;
                    let name = self.lex_ident_body();
                    self.push(TokenKind::Sym(name), start);
                } else {
                    self.pos += 1;
                    self.push(TokenKind::Colon, start);
                }
                continue;
            }

            if ch == '_' || ch.is_alphabetic() {
                let name = self.lex_ident_body();
                let kind = keyword_or_ident(&name);
                self.push(kind, start);
                continue;
            }

            // Punctuation / operators.
            self.lex_punct(start)?;
        }
        let eof = Span::new(self.file, self.src.len() as u32, self.src.len() as u32);
        self.out.push(Token::new(TokenKind::Eof, eof));
        Ok(())
    }

    fn skip_trivia(&mut self) {
        while self.pos < self.src.len() {
            let ch = self.peek().unwrap();
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.pos += 1;
                continue;
            }
            if ch == '#' && self.peek_at(1) != Some('{') {
                // Line comment: `#` to end-of-line. (Inside a string `#{...}`
                // is interpolation, but that path is handled in lex_string.)
                // Multi-byte chars in comments are common (Japanese in our
                // source samples), so step by `len_utf8`, not 1.
                while self.pos < self.src.len() {
                    let c = self.peek().unwrap();
                    if c == '\n' {
                        break;
                    }
                    self.pos += c.len_utf8();
                }
                continue;
            }
            break;
        }
    }

    fn lex_ident_body(&mut self) -> String {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == '_' || ch.is_alphanumeric() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        self.src[start..self.pos].to_string()
    }

    fn lex_number(&mut self, start: usize) -> Result<(), LexError> {
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let mut is_float = false;
        if self.peek() == Some('.') && self.peek_at(1).map_or(false, |c| c.is_ascii_digit()) {
            is_float = true;
            self.pos += 1;
            while let Some(ch) = self.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        let raw: String = self.src[start..self.pos]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        let kind = if is_float {
            TokenKind::Float(
                raw.parse()
                    .map_err(|e: std::num::ParseFloatError| self.err(start, e.to_string()))?,
            )
        } else {
            TokenKind::Int(
                raw.parse()
                    .map_err(|e: std::num::ParseIntError| self.err(start, e.to_string()))?,
            )
        };
        self.push(kind, start);
        Ok(())
    }

    fn lex_string(&mut self, start: usize) -> Result<(), LexError> {
        self.pos += 1; // consume opening `"`
        let mut segs = Vec::new();
        let mut buf = String::new();
        loop {
            let Some(ch) = self.peek() else {
                return Err(self.err(start, "unterminated string literal"));
            };
            match ch {
                '"' => {
                    self.pos += 1;
                    if !buf.is_empty() {
                        segs.push(StrSeg::Text(buf));
                    }
                    self.push(TokenKind::Str(segs), start);
                    return Ok(());
                }
                '\\' => {
                    self.pos += 1;
                    let esc = self
                        .peek()
                        .ok_or_else(|| self.err(start, "trailing `\\` in string"))?;
                    self.pos += esc.len_utf8();
                    buf.push(match esc {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '0' => '\0',
                        '"' => '"',
                        '\\' => '\\',
                        '#' => '#',
                        other => {
                            return Err(
                                self.err(self.pos - 1, format!("unknown escape `\\{other}`"))
                            );
                        }
                    });
                }
                '#' if self.peek_at(1) == Some('{') => {
                    if !buf.is_empty() {
                        segs.push(StrSeg::Text(std::mem::take(&mut buf)));
                    }
                    self.pos += 2;
                    let inner_start = self.pos;
                    let mut depth = 1;
                    while depth > 0 {
                        match self.peek() {
                            None => return Err(self.err(start, "unterminated `#{...}` in string")),
                            Some('{') => {
                                self.pos += 1;
                                depth += 1;
                            }
                            Some('}') => {
                                depth -= 1;
                                if depth == 0 {
                                    let inner_end = self.pos;
                                    self.pos += 1; // consume `}`
                                    let inner = &self.src[inner_start..inner_end];
                                    if let Some(sep) = find_format_spec_separator(inner) {
                                        let expr_end = inner_start + sep;
                                        let spec_start = expr_end + 1; // skip `:`
                                        let format_spec =
                                            self.src[spec_start..inner_end].trim().to_string();
                                        segs.push(StrSeg::InterpFmt {
                                            span: Span::new(
                                                self.file,
                                                inner_start as u32,
                                                expr_end as u32,
                                            ),
                                            format_spec,
                                        });
                                    } else {
                                        segs.push(StrSeg::Interp(Span::new(
                                            self.file,
                                            inner_start as u32,
                                            inner_end as u32,
                                        )));
                                    }
                                } else {
                                    self.pos += 1;
                                }
                            }
                            Some(c) => self.pos += c.len_utf8(),
                        }
                    }
                }
                _ => {
                    self.pos += ch.len_utf8();
                    buf.push(ch);
                }
            }
        }
    }

    fn lex_punct(&mut self, start: usize) -> Result<(), LexError> {
        // `..=` first so it wins against the `..` two-char match.
        if self.peek() == Some('.') && self.peek_at(1) == Some('.') && self.peek_at(2) == Some('=')
        {
            self.pos += 3;
            self.push(TokenKind::DotDotEq, start);
            return Ok(());
        }
        let two: Option<[char; 2]> = (|| Some([self.peek()?, self.peek_at(1)?]))();
        if let Some([a, b]) = two {
            let two_kind = match (a, b) {
                ('-', '>') => Some(TokenKind::Arrow),
                ('=', '>') => Some(TokenKind::FatArrow),
                ('<', '=') => Some(TokenKind::LtEq),
                ('>', '=') => Some(TokenKind::GtEq),
                ('=', '=') => Some(TokenKind::EqEq),
                ('!', '=') => Some(TokenKind::NotEq),
                ('+', '=') => Some(TokenKind::PlusEq),
                ('-', '=') => Some(TokenKind::MinusEq),
                ('*', '=') => Some(TokenKind::StarEq),
                ('/', '=') => Some(TokenKind::SlashEq),
                ('&', '&') => Some(TokenKind::AndAnd),
                ('|', '|') => Some(TokenKind::OrOr),
                ('.', '.') => Some(TokenKind::DotDot),
                _ => None,
            };
            if let Some(k) = two_kind {
                self.pos += 2;
                self.push(k, start);
                return Ok(());
            }
        }
        let ch = self.peek().unwrap();
        let kind = match ch {
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            '[' => TokenKind::LBracket,
            ']' => TokenKind::RBracket,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '?' => TokenKind::Question,
            '!' => TokenKind::Bang,
            ';' => TokenKind::Semicolon,
            '|' => TokenKind::Pipe,
            '&' => TokenKind::Amp,
            '^' => TokenKind::Caret,
            '<' => TokenKind::Lt,
            '>' => TokenKind::Gt,
            '=' => TokenKind::Eq,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '~' => TokenKind::Tilde,
            other => return Err(self.err(start, format!("unexpected character `{other}`"))),
        };
        self.pos += ch.len_utf8();
        self.push(kind, start);
        Ok(())
    }

    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.src[self.pos..].chars().nth(n)
    }

    fn push(&mut self, kind: TokenKind, start: usize) {
        let span = Span::new(self.file, start as u32, self.pos as u32);
        self.out.push(Token::new(kind, span));
    }

    fn err(&self, start: usize, msg: impl Into<String>) -> LexError {
        LexError {
            span: Span::new(self.file, start as u32, self.pos.max(start + 1) as u32),
            message: msg.into(),
        }
    }
}

fn keyword_or_ident(s: &str) -> TokenKind {
    match s {
        // Retired words are NOT here (§8.71): a keyword earns its
        // reservation by naming something the language HAS, and a word
        // that only produces "this was removed" costs every author an
        // identifier to buy a porting message for a language pixie is
        // not a port of any more. `slot`, `arc`, `widget`, `unowned`,
        // `owned`, `extern` and `batch` are ordinary identifiers now.
        //
        // `consuming` and `escaping` stayed: `consuming` feeds the
        // linearity analysis the driver runs on every build, so it
        // names something the language has whatever its current
        // reach — dropping it is a design question about move
        // semantics, not a keyword cleanup.
        "class" => TokenKind::Class,
        "struct" => TokenKind::Struct,
        "enum" => TokenKind::Enum,
        "flags" => TokenKind::Flags,
        // `of` is a contextual keyword: special only after `flags
        // <name>`. Lexed as a regular identifier in every other
        // position, so existing user code with `of` named locals
        // / fields keeps working.
        // Class-member field declaration. Kept short to match
        // Cute's other keyword family (`fn`, `pub`, `var`, …).
        "prop" => TokenKind::Property,
        "signal" => TokenKind::Signal,
        "fn" => TokenKind::Fn,
        "init" => TokenKind::Init,
        "deinit" => TokenKind::Deinit,
        "test" => TokenKind::Test,
        "emit" => TokenKind::Emit,
        "case" => TokenKind::Case,
        "when" => TokenKind::When,
        "error" => TokenKind::Error,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "use" => TokenKind::Use,
        "view" => TokenKind::View,
        "style" => TokenKind::Style,
        "try" => TokenKind::Try,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "for" => TokenKind::For,
        "while" => TokenKind::While,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "return" => TokenKind::Return,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        // `state` is a contextual keyword: lexed as an identifier and
        // recognized only at the head of a `view` body (see
        // `parse_state_fields`). Keeping it lexed-as-Ident lets prior
        // bindings (`signal stateChanged(state: Int)` in
        // `qwidgets_extra.qpi`) keep using `state` as a parameter name.
        "self" => TokenKind::Self_,
        "weak" => TokenKind::Weak,
        "consuming" => TokenKind::Consuming,
        "escaping" => TokenKind::Escaping,
        "pub" => TokenKind::Pub,
        "trait" => TokenKind::Trait,
        "impl" => TokenKind::Impl,
        "store" => TokenKind::Store,
        "suite" => TokenKind::Suite,
        "static" => TokenKind::Static,
        "true" => TokenKind::Bool(true),
        "false" => TokenKind::Bool(false),
        "nil" => TokenKind::Nil,
        _ => TokenKind::Ident(s.to_string()),
    }
}

/// Locate the format-spec separator inside a `#{...}` body.
///
/// Returns the byte offset of the `:` separator (relative to `inner`)
/// when one is present, or `None` if the body has no format spec.
///
/// The separator is the LAST `:` that is:
/// - At brace/bracket/paren depth 0 (i.e. not nested inside `{}`,
///   `[]`, `()`).
/// - Not paired with an earlier `?` (so `a ? b : c` ternaries are
///   skipped).
/// - Not the leading `:` of a Sym literal at the very start of the
///   interp (the spec must follow some expression text).
/// - Not inside a string literal.
///
/// This is a heuristic — pathological cases like `#{x?:foo:.2f}` (a
/// ternary whose true-branch is a Sym, with a format spec) will
/// be parsed wrong. In practice the disambiguation matches what users
/// expect for the common `:fmt` shapes (`.2f`, `08d`, `>20`).
fn find_format_spec_separator(inner: &str) -> Option<usize> {
    let bytes = inner.as_bytes();
    let mut brace_depth: i32 = 0;
    let mut bracket_depth: i32 = 0;
    let mut paren_depth: i32 = 0;
    let mut ternary_open: i32 = 0;
    let mut in_str = false;
    let mut last_colon: Option<usize> = None;
    let mut prev_non_ws: Option<u8> = None;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => brace_depth += 1,
            b'}' => brace_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b'(' => paren_depth += 1,
            b')' => paren_depth -= 1,
            b'?' if brace_depth == 0 && bracket_depth == 0 && paren_depth == 0 => {
                ternary_open += 1;
            }
            b':' if brace_depth == 0 && bracket_depth == 0 && paren_depth == 0 => {
                if ternary_open > 0 {
                    ternary_open -= 1;
                } else if prev_non_ws.is_some() {
                    last_colon = Some(i);
                }
            }
            _ => {}
        }
        if !c.is_ascii_whitespace() {
            prev_non_ws = Some(c);
        }
        i += 1;
    }
    last_colon
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::FileId;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(FileId(0), src)
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn lex_class_skeleton() {
        let src = "class TodoItem < Base {}";
        let toks = kinds(src);
        assert!(matches!(toks[0], TokenKind::Class));
        assert!(matches!(toks[1], TokenKind::Ident(ref s) if s == "TodoItem"));
        assert!(matches!(toks[2], TokenKind::Lt));
        assert!(matches!(toks[3], TokenKind::Ident(ref s) if s == "Base"));
        assert!(matches!(toks[4], TokenKind::LBrace));
        assert!(matches!(toks[5], TokenKind::RBrace));
    }

    #[test]
    fn lex_property_with_notify() {
        let src = "prop done : Bool, notify: :stateChanged";
        let toks = kinds(src);
        assert!(matches!(toks[0], TokenKind::Property));
        assert!(matches!(toks[1], TokenKind::Ident(ref s) if s == "done"));
        assert!(matches!(toks[2], TokenKind::Colon));
        assert!(matches!(toks[3], TokenKind::Ident(ref s) if s == "Bool"));
        assert!(matches!(toks[4], TokenKind::Comma));
        assert!(matches!(toks[5], TokenKind::Ident(ref s) if s == "notify"));
        assert!(matches!(toks[6], TokenKind::Colon));
        assert!(matches!(toks[7], TokenKind::Sym(ref s) if s == "stateChanged"));
    }

    #[test]
    fn lex_at_ident_assignment() {
        let src = "@done = !@done";
        let toks = kinds(src);
        assert!(matches!(toks[0], TokenKind::AtIdent(ref s) if s == "done"));
        assert!(matches!(toks[1], TokenKind::Eq));
        assert!(matches!(toks[2], TokenKind::Bang));
        assert!(matches!(toks[3], TokenKind::AtIdent(ref s) if s == "done"));
    }

    #[test]
    fn lex_block_arg_pipes() {
        let src = "{ |item| item }";
        let toks = kinds(src);
        assert!(matches!(toks[0], TokenKind::LBrace));
        assert!(matches!(toks[1], TokenKind::Pipe));
        assert!(matches!(toks[2], TokenKind::Ident(ref s) if s == "item"));
        assert!(matches!(toks[3], TokenKind::Pipe));
    }

    #[test]
    fn lex_string_with_interpolation() {
        let src = "\"hello #{name}!\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str, got {:?}", toks[0])
        };
        assert_eq!(segs.len(), 3);
        assert!(matches!(&segs[0], StrSeg::Text(t) if t == "hello "));
        assert!(matches!(&segs[1], StrSeg::Interp(_)));
        assert!(matches!(&segs[2], StrSeg::Text(t) if t == "!"));
    }

    #[test]
    fn lex_string_with_format_spec_simple() {
        // `#{x:.2f}` → InterpFmt with format_spec=".2f"
        let src = "\"price: #{x:.2f}\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str")
        };
        assert_eq!(segs.len(), 2);
        assert!(matches!(&segs[0], StrSeg::Text(t) if t == "price: "));
        let spec = match &segs[1] {
            StrSeg::InterpFmt { format_spec, .. } => format_spec.clone(),
            other => panic!("expected InterpFmt, got {:?}", other),
        };
        assert_eq!(spec, ".2f");
    }

    #[test]
    fn lex_string_with_format_spec_zero_pad() {
        let src = "\"#{n:08d}\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str")
        };
        assert_eq!(segs.len(), 1);
        let spec = match &segs[0] {
            StrSeg::InterpFmt { format_spec, .. } => format_spec.clone(),
            other => panic!("expected InterpFmt, got {:?}", other),
        };
        assert_eq!(spec, "08d");
    }

    #[test]
    fn lex_string_with_format_spec_align() {
        let src = "\"#{name:>20}\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str")
        };
        let spec = match &segs[0] {
            StrSeg::InterpFmt { format_spec, .. } => format_spec.clone(),
            other => panic!("expected InterpFmt, got {:?}", other),
        };
        assert_eq!(spec, ">20");
    }

    #[test]
    fn lex_string_no_format_spec_for_sym_at_start() {
        // `#{:foo}` is a Sym literal, not a format spec on an empty
        // expression. The parser-side test verifies this resolves to
        // a Sym; here we just confirm it does not emit InterpFmt.
        let src = "\"#{:foo}\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str")
        };
        assert!(
            matches!(&segs[0], StrSeg::Interp(_)),
            "leading `:` is a Sym literal, not a format spec; got {:?}",
            &segs[0]
        );
    }

    #[test]
    fn lex_string_no_format_spec_for_map_literal() {
        // `#{ {a: 1} }` — the `:` is inside a nested map literal,
        // brace_depth > 0, so it must NOT be treated as format spec.
        let src = "\"#{ {a: 1} }\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str")
        };
        assert!(
            matches!(&segs[0], StrSeg::Interp(_)),
            "`:` inside map literal is NOT a format spec; got {:?}",
            &segs[0]
        );
    }

    #[test]
    fn lex_string_format_spec_with_ternary_uses_outer_colon() {
        // `#{a ? b : c:.2f}` — the first `:` matches the ternary,
        // the second `:` is the format spec.
        let src = "\"#{a ? b : c:.2f}\"";
        let toks = kinds(src);
        let TokenKind::Str(ref segs) = toks[0] else {
            panic!("expected Str")
        };
        let spec = match &segs[0] {
            StrSeg::InterpFmt { format_spec, .. } => format_spec.clone(),
            other => panic!("expected InterpFmt, got {:?}", other),
        };
        assert_eq!(spec, ".2f");
    }

    #[test]
    fn lex_error_union_and_propagation() {
        let src = "fn open(path: String) !File { File.open(path)? }";
        let toks = kinds(src);
        assert!(matches!(toks[0], TokenKind::Fn));
        // ... we just spot-check a few interesting tokens
        assert!(toks.iter().any(|t| matches!(t, TokenKind::Bang)));
        assert!(toks.iter().any(|t| matches!(t, TokenKind::Question)));
    }

    #[test]
    fn lex_line_comment_skipped() {
        let src = "# leading comment\nclass X {}";
        let toks = kinds(src);
        assert!(matches!(toks[0], TokenKind::Newline));
        assert!(matches!(toks[1], TokenKind::Class));
    }

    #[test]
    fn lex_keywords_lookup() {
        let src = "use error case when emit await async self weak";
        let toks = kinds(src);
        for t in &toks[..toks.len() - 1] {
            assert!(
                matches!(
                    t,
                    TokenKind::Use
                        | TokenKind::Error
                        | TokenKind::Case
                        | TokenKind::When
                        | TokenKind::Emit
                        | TokenKind::Await
                        | TokenKind::Async
                        | TokenKind::Self_
                        | TokenKind::Weak
                ),
                "unexpected token {:?}",
                t
            );
        }
    }

    /// The words that stopped being words (§8.71). Each one named a
    /// construct pixie does not have, and reserving it cost every
    /// author an ordinary identifier.
    #[test]
    fn retired_words_are_ordinary_identifiers() {
        let src = "slot arc widget unowned owned extern batch";
        for t in &kinds(src)[..7] {
            assert!(
                matches!(t, TokenKind::Ident(_)),
                "expected an identifier, got {t:?}"
            );
        }
    }
}
