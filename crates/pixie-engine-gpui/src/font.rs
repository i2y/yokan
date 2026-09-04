//! The canvas font: 95 printable ASCII glyphs, 3x5 on a 4x6 cell.
//!
//! Written as art so the source shows the letters — each entry is one
//! glyph, five rows of three columns, top to bottom. The fourth
//! column and the sixth row are the spacing between glyphs, which is
//! why text laid out at four pixels a character never touches.
//!
//! Yokan's own, not carried from anywhere: a drawing surface whose
//! text depends on another project's asset would not be
//! self-contained, and at this size the shapes are decided by the
//! grid anyway.

use std::sync::LazyLock;

/// Cell size in virtual pixels. Text advances by `CELL_W` per
/// character and a line is `CELL_H` tall.
pub const CELL_W: i64 = 4;
pub const CELL_H: i64 = 6;

const ART: [&str; 95] = [
    "... ... ... ... ...", // space
    ".#. .#. .#. ... .#.", // !
    "#.# #.# ... ... ...", // quote
    "#.# ### #.# ### #.#", // #
    ".## ##. .#. ##. ##.", // $
    "#.# ..# .#. #.. #.#", // %
    ".#. #.# .#. #.# .##", // &
    ".#. .#. ... ... ...", // '
    "..# .#. .#. .#. ..#", // (
    "#.. ..# ..# ..# #..", // )
    "#.# .#. #.# ... ...", // *
    "... .#. ### .#. ...", // +
    "... ... ... .#. #..", // ,
    "... ... ### ... ...", // -
    "... ... ... ... .#.", // .
    "..# ..# .#. #.. #..", // /
    "### #.# #.# #.# ###", // 0
    ".#. ##. .#. .#. ###", // 1
    "### ..# ### #.. ###", // 2
    "### ..# ### ..# ###", // 3
    "#.# #.# ### ..# ..#", // 4
    "### #.. ### ..# ###", // 5
    "### #.. ### #.# ###", // 6
    "### ..# ..# ..# ..#", // 7
    "### #.# ### #.# ###", // 8
    "### #.# ### ..# ###", // 9
    "... .#. ... .#. ...", // :
    "... .#. ... .#. #..", // ;
    "..# .#. #.. .#. ..#", // <
    "... ### ... ### ...", // =
    "#.. .#. ..# .#. #..", // >
    "### ..# .## ... .#.", // ?
    "### #.# ### #.. ###", // @
    ".#. #.# ### #.# #.#", // A
    "##. #.# ##. #.# ##.", // B
    ".## #.. #.. #.. .##", // C
    "##. #.# #.# #.# ##.", // D
    "### #.. ##. #.. ###", // E
    "### #.. ##. #.. #..", // F
    ".## #.. #.# #.# .##", // G
    "#.# #.# ### #.# #.#", // H
    "### .#. .#. .#. ###", // I
    "..# ..# ..# #.# ###", // J
    "#.# #.# ##. #.# #.#", // K
    "#.. #.. #.. #.. ###", // L
    "#.# ### ### #.# #.#", // M
    "#.# ##. #.# .## #.#", // N
    "### #.# #.# #.# ###", // O
    "### #.# ### #.. #..", // P
    "### #.# #.# ### ..#", // Q
    "### #.# ##. #.# #.#", // R
    ".## #.. ### ..# ##.", // S
    "### .#. .#. .#. .#.", // T
    "#.# #.# #.# #.# ###", // U
    "#.# #.# #.# #.# .#.", // V
    "#.# #.# ### ### #.#", // W
    "#.# #.# .#. #.# #.#", // X
    "#.# #.# .#. .#. .#.", // Y
    "### ..# .#. #.. ###", // Z
    "##. #.. #.. #.. ##.", // [
    "#.. #.. .#. ..# ..#", // backslash
    ".## ..# ..# ..# .##", // ]
    ".#. #.# ... ... ...", // ^
    "... ... ... ... ###", // _
    "#.. .#. ... ... ...", // `
    "... ##. ..# ### .##", // a
    "#.. #.. ##. #.# ##.", // b
    "... .## #.. #.. .##", // c
    "..# ..# .## #.# .##", // d
    "... .#. #.# ##. .##", // e
    "..# .#. ### .#. .#.", // f
    "... .## #.# .## ##.", // g
    "#.. #.. ##. #.# #.#", // h
    ".#. ... .#. .#. .#.", // i
    "..# ... ..# #.# .#.", // j
    "#.. #.# ##. ##. #.#", // k
    "##. .#. .#. .#. .##", // l
    "... #.# ### ### #.#", // m
    "... ##. #.# #.# #.#", // n
    "... .#. #.# #.# .#.", // o
    "... ##. #.# ##. #..", // p
    "... .## #.# .## ..#", // q
    "... .## #.. #.. #..", // r
    "... .## ##. ..# ##.", // s
    ".#. ### .#. .#. ..#", // t
    "... #.# #.# #.# .##", // u
    "... #.# #.# #.# .#.", // v
    "... #.# ### ### .#.", // w
    "... #.# .#. .#. #.#", // x
    "... #.# .## ..# ##.", // y
    "... ### .#. #.. ###", // z
    "..# .#. ##. .#. ..#", // {
    ".#. .#. .#. .#. .#.", // |
    "#.. .#. .## .#. #..", // }
    "... .## #.# ##. ...", // ~
];

/// One glyph as five row bitmasks, bit 2 = leftmost column.
static GLYPHS: LazyLock<[[u8; 5]; 95]> = LazyLock::new(|| {
    let mut out = [[0u8; 5]; 95];
    for (g, art) in ART.iter().enumerate() {
        for (r, row) in art.split(' ').enumerate() {
            let mut bits = 0u8;
            for (c, ch) in row.chars().enumerate() {
                if ch == '#' {
                    bits |= 1 << (2 - c);
                }
            }
            out[g][r] = bits;
        }
    }
    out
});

/// Is this column of this row of this character's glyph painted?
/// Anything outside the printable range draws nothing — a canvas
/// never fails a frame over a character it does not have.
pub fn dot(ch: char, col: i64, row: i64) -> bool {
    if !(0..3).contains(&col) || !(0..5).contains(&row) {
        return false;
    }
    let c = ch as u32;
    if !(32..127).contains(&c) {
        return false;
    }
    let g = GLYPHS[(c - 32) as usize][row as usize];
    g & (1 << (2 - col)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_parses_to_five_rows() {
        for art in ART {
            assert_eq!(art.split(' ').count(), 5, "{art}");
            for row in art.split(' ') {
                assert_eq!(row.chars().count(), 3, "{art}");
                assert!(row.chars().all(|c| c == '#' || c == '.'), "{art}");
            }
        }
    }

    #[test]
    fn a_known_glyph_reads_back() {
        // `L`: a stem down the left and a foot along the bottom.
        assert!(dot('L', 0, 0));
        assert!(!dot('L', 1, 0));
        assert!(dot('L', 2, 4));
        // Space is empty, and so is anything outside the table.
        assert!(!dot(' ', 0, 0));
        assert!(!dot('あ', 0, 0));
    }
}
