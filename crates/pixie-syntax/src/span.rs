//! Source spans and a tiny `SourceMap` for diagnostic rendering.

use std::ops::Range;

/// Identifier for a single source file in a `SourceMap`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct FileId(pub u32);

/// A half-open byte range `[start, end)` paired with its `FileId`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(file: FileId, start: u32, end: u32) -> Self {
        debug_assert!(start <= end);
        Self { file, start, end }
    }

    pub fn dummy() -> Self {
        Self {
            file: FileId(0),
            start: 0,
            end: 0,
        }
    }

    /// Merge two spans into one that covers both. Files must match.
    pub fn join(self, other: Span) -> Span {
        debug_assert_eq!(self.file, other.file, "cannot join spans across files");
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

#[derive(Default, Debug)]
pub struct SourceMap {
    files: Vec<SourceFile>,
}

#[derive(Debug)]
pub struct SourceFile {
    pub name: String,
    pub source: String,
}

impl SourceMap {
    pub fn add(&mut self, name: String, source: String) -> FileId {
        let id = FileId(self.files.len() as u32);
        self.files.push(SourceFile { name, source });
        id
    }

    pub fn source(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].source
    }

    pub fn name(&self, id: FileId) -> &str {
        &self.files[id.0 as usize].name
    }

    /// Number of files registered. Returned ids are in `0..count`,
    /// so callers iterating the map (e.g. building a parallel
    /// codespan-reporting `SimpleFiles`) walk this range.
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    pub fn line_col(&self, span: Span) -> (usize, usize) {
        let src = self.source(span.file);
        let mut line = 1usize;
        let mut col = 1usize;
        for (i, b) in src.bytes().enumerate() {
            if i == span.start as usize {
                return (line, col);
            }
            if b == b'\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }
}
