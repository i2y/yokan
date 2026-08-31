//! Diagnostic shape used across the syntax crate.
//!
//! Designed to be cheap to construct in hot lex/parse paths and to convert
//! into `codespan-reporting`'s `Diagnostic` for terminal rendering at the
//! driver layer.

use crate::span::Span;

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub message: String,
    pub primary: Span,
    pub notes: Vec<(Span, String)>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            primary: span,
            notes: Vec::new(),
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            primary: span,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, span: Span, note: impl Into<String>) -> Self {
        self.notes.push((span, note.into()));
        self
    }
}
