//! The engine-side text editor behind `Element::TextField`.
//!
//! Ported from the S1-verified `s4-gpui/src/bin/ime_input.rs` — the
//! fixed fork of Zed's input example. The two upstream defects stay
//! fixed here: the IME's selection is converted inside `new_text` (not
//! against the whole content), and every range arriving from the IME is
//! clamped to char boundaries before slicing. The mac input-method
//! callbacks are plain `extern "C"`, so any panic below them aborts the
//! process — nothing in this file may index with an unsanitized range.
//!
//! One entity per live TextField, keyed by element-tree path and kept
//! across rebuilds (positional state transfer): caret, selection,
//! composition, and focus survive; the bound value is pushed in only
//! when the app-side value actually changes between rebuilds.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
    WrappedLine,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ShapedLine,
    SharedString, Style, TextRun, UTF16Selection, UnderlineStyle, Window, actions, div, fill,
    point, prelude::*, px, relative, rgb, rgba, size,
};
use unicode_segmentation::*;

actions!(
    pixie_text_input,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        ShowCharacterPalette,
        Paste,
        Cut,
        Copy,
        Submit,
        TabNext,
        TabPrev,
        Up,
        Down,
    ]
);

/// Register the editing key map once per app. Bindings are scoped to
/// the `PixieTextInput` key context, so they fire only while a field
/// has focus.
pub fn bind_keys(cx: &mut App) {
    const CTX: Option<&str> = Some("PixieTextInput");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, CTX),
        KeyBinding::new("delete", Delete, CTX),
        KeyBinding::new("left", Left, CTX),
        KeyBinding::new("right", Right, CTX),
        KeyBinding::new("shift-left", SelectLeft, CTX),
        KeyBinding::new("shift-right", SelectRight, CTX),
        KeyBinding::new("cmd-a", SelectAll, CTX),
        KeyBinding::new("cmd-v", Paste, CTX),
        KeyBinding::new("cmd-c", Copy, CTX),
        KeyBinding::new("cmd-x", Cut, CTX),
        KeyBinding::new("home", Home, CTX),
        KeyBinding::new("end", End, CTX),
        KeyBinding::new("enter", Submit, CTX),
        KeyBinding::new("tab", TabNext, CTX),
        KeyBinding::new("shift-tab", TabPrev, CTX),
        KeyBinding::new("up", Up, CTX),
        KeyBinding::new("down", Down, CTX),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, CTX),
    ]);
}

/// App-level callback installed by the render pass: routes a committed
/// edit (or a submit) back into the World through the root entity.
pub type TextCallback = Rc<dyn Fn(&str, &mut App)>;

pub struct PixieInput {
    pub focus_handle: FocusHandle,
    content: SharedString,
    /// The value the element tree carried at the last sync — the push
    /// happens only when the tree-side value changes, so user edits are
    /// never clobbered by an unrelated rebuild.
    last_bound: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    /// The wrapped layout of a multi-line field, one entry per hard
    /// line, with the byte offset that line starts at. Both the caret
    /// and the mouse read positions out of this.
    last_wrapped: Vec<(WrappedLine, usize)>,
    last_bounds: Option<Bounds<Pixels>>,
    /// A field that holds paragraphs: it wraps, `enter` writes a
    /// newline, and the caret moves by visual line.
    pub multiline: bool,
    /// How many lines of it are visible (0 = the default 4).
    pub rows: f32,
    /// Vertical scroll keeping the caret in view — the multi-line
    /// twin of `scroll_x`.
    scroll_y: Pixels,
    /// The line height the last layout used.
    line_height: Pixels,
    /// Horizontal scroll keeping the caret in view when the text is
    /// wider than the field (a cute_ui gap closed in the port).
    scroll_x: Pixels,
    is_selecting: bool,
    pub on_commit: Option<TextCallback>,
    pub on_submit: Option<TextCallback>,
    pub next_focus: Option<FocusHandle>,
    pub prev_focus: Option<FocusHandle>,
}

impl PixieInput {
    pub fn new(cx: &mut Context<Self>, value: &str, placeholder: &str) -> Self {
        PixieInput {
            focus_handle: cx.focus_handle(),
            content: SharedString::from(value.to_string()),
            last_bound: SharedString::from(value.to_string()),
            placeholder: SharedString::from(placeholder.to_string()),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_wrapped: Vec::new(),
            last_bounds: None,
            multiline: false,
            rows: 0.,
            scroll_x: px(0.),
            scroll_y: px(0.),
            line_height: px(0.),
            is_selecting: false,
            on_commit: None,
            on_submit: None,
            next_focus: None,
            prev_focus: None,
        }
    }

    /// Reconcile against the element tree after a rebuild. The bound
    /// value is applied only when it differs from the previously bound
    /// one (and never mid-composition, where the IME owns the text).
    pub fn sync(
        &mut self,
        value: &str,
        placeholder: &str,
        multiline: bool,
        rows: f32,
        cx: &mut Context<Self>,
    ) {
        if self.multiline != multiline || self.rows != rows {
            self.multiline = multiline;
            self.rows = rows;
            cx.notify();
        }
        if self.placeholder.as_ref() != placeholder {
            self.placeholder = SharedString::from(placeholder.to_string());
            cx.notify();
        }
        if self.last_bound.as_ref() != value {
            self.last_bound = SharedString::from(value.to_string());
            if self.marked_range.is_none() && self.content.as_ref() != value {
                self.content = self.last_bound.clone();
                let end = clamp_to_boundary(&self.content, self.selected_range.end);
                self.selected_range = end..end;
                self.selection_reversed = false;
                cx.notify();
            }
        }
    }

    fn fire_commit(&mut self, cx: &mut Context<Self>) {
        if let Some(cb) = self.on_commit.clone() {
            let text = self.content.to_string();
            cb(&text, cx);
        }
    }

    fn submit(&mut self, _: &Submit, window: &mut Window, cx: &mut Context<Self>) {
        // A field that holds paragraphs takes the key as a newline;
        // there is nowhere else for `enter` to go in one.
        if self.multiline {
            self.replace_text_in_range(None, "\n", window, cx);
            return;
        }
        if let Some(cb) = self.on_submit.clone() {
            let text = self.content.to_string();
            cb(&text, cx);
        }
    }

    /// The caret one visual line up or down, keeping its column. A
    /// single-line field has nowhere to go, so it stays put.
    fn move_by_line(&mut self, down: bool, cx: &mut Context<Self>) {
        if !self.multiline || self.last_wrapped.is_empty() {
            return;
        }
        let line_height = self.line_height;
        let ix = self.cursor_offset();
        let Some((li, (line, start))) = self
            .last_wrapped
            .iter()
            .enumerate()
            .find(|(_, (l, st))| ix >= *st && ix <= *st + l.len())
        else {
            return;
        };
        let Some(here) = line.position_for_index(ix - start, line_height) else {
            return;
        };
        let want_y = if down {
            here.y + line_height
        } else {
            here.y - line_height
        };
        // Inside this hard line when the row exists there, otherwise
        // the neighbouring hard line's first or last row.
        let rows = line.size(line_height).height;
        if want_y >= px(0.) && want_y < rows {
            let target = point(here.x, want_y);
            let found = line.closest_index_for_position(target, line_height);
            let off = found.unwrap_or_else(|e| e);
            self.move_to(start + off, cx);
            return;
        }
        let next = if down { li + 1 } else { li.checked_sub(1).unwrap_or(li) };
        if down && next >= self.last_wrapped.len() {
            let end = self.content.len();
            self.move_to(end, cx);
            return;
        }
        if !down && li == 0 {
            self.move_to(0, cx);
            return;
        }
        let (nline, nstart) = &self.last_wrapped[next];
        let y = if down {
            px(0.)
        } else {
            nline.size(line_height).height - line_height
        };
        let found = nline.closest_index_for_position(point(here.x, y), line_height);
        let off = found.unwrap_or_else(|e| e);
        self.move_to(nstart + off, cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_line(false, cx);
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.move_by_line(true, cx);
    }

    fn tab_next(&mut self, _: &TabNext, window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(h) = &self.next_focus {
            window.focus(h, _cx);
        }
    }

    fn tab_prev(&mut self, _: &TabPrev, window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(h) = &self.prev_focus {
            window.focus(h, _cx);
        }
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let to = if self.multiline {
            let ix = self.cursor_offset();
            self.content[..ix].rfind('\n').map(|n| n + 1).unwrap_or(0)
        } else {
            0
        };
        self.move_to(to, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let to = if self.multiline {
            let ix = self.cursor_offset();
            self.content[ix..]
                .find('\n')
                .map(|n| ix + n)
                .unwrap_or(self.content.len())
        } else {
            self.content.len()
        };
        self.move_to(to, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        self.is_selecting = true;

        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn show_character_palette(
        &mut self,
        _: &ShowCharacterPalette,
        window: &mut Window,
        _: &mut Context<Self>,
    ) {
        window.show_character_palette();
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.replace_text_in_range(None, &text.replace("\n", " "), window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let range = sanitize_range(&self.content, &self.selected_range);
            cx.write_to_clipboard(ClipboardItem::new_string(self.content[range].to_string()));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            let range = sanitize_range(&self.content, &self.selected_range);
            cx.write_to_clipboard(ClipboardItem::new_string(self.content[range].to_string()));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }

        let Some(bounds) = self.last_bounds.as_ref() else {
            return 0;
        };
        if self.multiline {
            let line_height = self.line_height;
            let mut y = bounds.top() - self.scroll_y;
            for (line, start) in &self.last_wrapped {
                let h = line.size(line_height).height;
                if position.y < y + h || Some(*start) == self.last_wrapped.last().map(|(_, s)| *s) {
                    let local = point(position.x - bounds.left(), (position.y - y).max(px(0.)));
                    let found = line.closest_index_for_position(local, line_height);
                    return start + found.unwrap_or_else(|e| e);
                }
                y += h;
            }
            return self.content.len();
        }
        let Some(line) = self.last_layout.as_ref() else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left() + self.scroll_x)
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        utf16_offset_in(&self.content, offset)
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;

        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }

        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

// Everything arriving from the IME is converted and clamped so that no
// range arithmetic can leave the string or split a code point (a panic
// under the mac input callbacks aborts the process).

fn utf16_offset_in(s: &str, offset: usize) -> usize {
    let mut utf8_offset = 0;
    let mut utf16_count = 0;
    for ch in s.chars() {
        if utf16_count >= offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

fn clamp_to_boundary(s: &str, mut offset: usize) -> usize {
    if offset > s.len() {
        offset = s.len();
    }
    while offset > 0 && !s.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

fn sanitize_range(s: &str, range: &Range<usize>) -> Range<usize> {
    let start = clamp_to_boundary(s, range.start);
    let end = clamp_to_boundary(s, range.end).max(start);
    start..end
}

impl EntityInputHandler for PixieInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = sanitize_range(&self.content, &self.range_from_utf16(&range_utf16));
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&sanitize_range(&self.content, &self.selected_range)),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = sanitize_range(&self.content, &range);

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        cx.notify();
        self.fire_commit(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let range = sanitize_range(&self.content, &range);

        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        if !new_text.is_empty() {
            self.marked_range = Some(range.start..range.start + new_text.len());
        } else {
            self.marked_range = None;
        }
        // The IME's selection is relative to `new_text`, not to the whole
        // content, and both of its ends are anchored at where the composed
        // text was inserted.
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| {
                let start = range.start + utf16_offset_in(new_text, range_utf16.start);
                let end = range.start + utf16_offset_in(new_text, range_utf16.end);
                start..end
            })
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());

        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let last_layout = self.last_layout.as_ref()?;
        let range = sanitize_range(&self.content, &self.range_from_utf16(&range_utf16));
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start) - self.scroll_x,
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end) - self.scroll_x,
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;

        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct PixieInputElement {
    input: Entity<PixieInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    /// The wrapped layout of a multi-line field, with each hard line's
    /// start offset — empty for a single-line one.
    wrapped: Vec<(WrappedLine, usize)>,
    scroll: Pixels,
    scroll_y: Pixels,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    /// Selection quads for a multi-line field: one per visual row the
    /// selection covers.
    selections: Vec<PaintQuad>,
}

impl IntoElement for PixieInputElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl gpui::Element for PixieInputElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.read(cx);
        let rows = if input.multiline {
            if input.rows > 0. { input.rows } else { 4. }
        } else {
            1.
        };
        style.size.height = (window.line_height() * rows).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = sanitize_range(&content, &input.selected_range);
        let cursor = clamp_to_boundary(&content, input.cursor_offset());
        let marked_range = input
            .marked_range
            .as_ref()
            .map(|r| sanitize_range(&content, r));
        let style = window.text_style();

        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), crate::rgba(crate::theme().text_dim_rgba).into())
        } else {
            (content, style.color)
        };

        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };

        let font_size = style.font_size.to_pixels(window.rem_size());
        let display_len = display_text.len();
        let line_height = window.line_height();
        if input.multiline {
            // A field that holds paragraphs: shaped WRAPPED, one entry
            // per hard line, and every position the caret, the mouse
            // and the selection need is read out of that layout.
            let wrap_width = bounds.right() - bounds.left();
            let shaped = window
                .text_system()
                .shape_text(display_text, font_size, &runs, Some(wrap_width), None)
                .map(|v| v.into_iter().collect::<Vec<_>>())
                .unwrap_or_default();
            let mut wrapped: Vec<(WrappedLine, usize)> = Vec::with_capacity(shaped.len());
            let mut at = 0usize;
            for l in shaped {
                let len = l.len();
                wrapped.push((l, at));
                at += len + 1; // the newline between two hard lines
            }
            // Where the caret sits, in the field's own coordinates.
            let caret = wrapped
                .iter()
                .find(|(l, st)| cursor >= *st && cursor <= *st + l.len())
                .and_then(|(l, st)| {
                    let p = l.position_for_index(cursor - st, line_height)?;
                    let above: Pixels = wrapped
                        .iter()
                        .take_while(|(_, s)| *s < *st)
                        .map(|(l, _)| l.size(line_height).height)
                        .fold(px(0.), |a, b| a + b);
                    Some(point(p.x, p.y + above))
                })
                .unwrap_or(point(px(0.), px(0.)));
            // Vertical scroll: keep the caret's row inside the field.
            let total: Pixels = wrapped
                .iter()
                .map(|(l, _)| l.size(line_height).height)
                .fold(px(0.), |a, b| a + b);
            let view_h = bounds.bottom() - bounds.top();
            let mut scroll_y = self.input.read(cx).scroll_y;
            if total <= view_h {
                scroll_y = px(0.);
            } else {
                if caret.y < scroll_y {
                    scroll_y = caret.y;
                }
                if caret.y + line_height > scroll_y + view_h {
                    scroll_y = caret.y + line_height - view_h;
                }
                let max = total - view_h;
                if scroll_y > max {
                    scroll_y = max;
                }
                if scroll_y < px(0.) {
                    scroll_y = px(0.);
                }
            }
            let cursor_quad = selected_range.is_empty().then(|| {
                fill(
                    Bounds::new(
                        point(bounds.left() + caret.x, bounds.top() + caret.y - scroll_y),
                        size(px(2.), line_height),
                    ),
                    rgb(crate::theme().accent),
                )
            });
            // One quad per visual row the selection covers.
            let mut selections = Vec::new();
            if !selected_range.is_empty() {
                let mut above = px(0.);
                for (l, st) in &wrapped {
                    let (ls, le) = (*st, *st + l.len());
                    let from = selected_range.start.max(ls);
                    let to = selected_range.end.min(le);
                    if from <= to && selected_range.start <= le && selected_range.end >= ls {
                        if let (Some(a), Some(b)) = (
                            l.position_for_index(from - ls, line_height),
                            l.position_for_index(to - ls, line_height),
                        ) {
                            let right = bounds.right();
                            if a.y == b.y {
                                selections.push(fill(
                                    Bounds::from_corners(
                                        point(bounds.left() + a.x, bounds.top() + above + a.y - scroll_y),
                                        point(bounds.left() + b.x, bounds.top() + above + a.y + line_height - scroll_y),
                                    ),
                                    rgba(crate::theme().selection_rgba),
                                ));
                            } else {
                                selections.push(fill(
                                    Bounds::from_corners(
                                        point(bounds.left() + a.x, bounds.top() + above + a.y - scroll_y),
                                        point(right, bounds.top() + above + a.y + line_height - scroll_y),
                                    ),
                                    rgba(crate::theme().selection_rgba),
                                ));
                                let mut y = a.y + line_height;
                                while y < b.y {
                                    selections.push(fill(
                                        Bounds::from_corners(
                                            point(bounds.left(), bounds.top() + above + y - scroll_y),
                                            point(right, bounds.top() + above + y + line_height - scroll_y),
                                        ),
                                        rgba(crate::theme().selection_rgba),
                                    ));
                                    y += line_height;
                                }
                                selections.push(fill(
                                    Bounds::from_corners(
                                        point(bounds.left(), bounds.top() + above + b.y - scroll_y),
                                        point(bounds.left() + b.x, bounds.top() + above + b.y + line_height - scroll_y),
                                    ),
                                    rgba(crate::theme().selection_rgba),
                                ));
                            }
                        }
                    }
                    above += l.size(line_height).height;
                }
            }
            return PrepaintState {
                line: None,
                wrapped,
                scroll: px(0.),
                scroll_y,
                cursor: cursor_quad,
                selection: None,
                selections,
            };
        }
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);

        // Horizontal scroll: keep the caret inside the field. When the
        // line fits, snap back to zero.
        let cursor_pos = line.x_for_index(cursor);
        let inner_width = bounds.right() - bounds.left();
        let line_width = line.x_for_index(display_len);
        let margin = px(2.);
        let mut scroll = self.input.read(cx).scroll_x;
        if line_width <= inner_width {
            scroll = px(0.);
        } else {
            if cursor_pos < scroll {
                scroll = cursor_pos;
            }
            if cursor_pos > scroll + inner_width - margin {
                scroll = cursor_pos - inner_width + margin;
            }
            let max_scroll = line_width - inner_width + margin;
            if scroll > max_scroll {
                scroll = max_scroll;
            }
            if scroll < px(0.) {
                scroll = px(0.);
            }
        }

        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos - scroll, bounds.top()),
                        size(px(2.), bounds.bottom() - bounds.top()),
                    ),
                    rgb(crate::theme().accent),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start) - scroll,
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end) - scroll,
                            bounds.bottom(),
                        ),
                    ),
                    rgba(crate::theme().selection_rgba),
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            wrapped: Vec::new(),
            scroll,
            scroll_y: px(0.),
            cursor,
            selection,
            selections: Vec::new(),
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let line_height = window.line_height();
        if !prepaint.wrapped.is_empty() || prepaint.line.is_none() {
            // The multi-line field: every row of every hard line, then
            // the selection and the caret over them.
            for quad in std::mem::take(&mut prepaint.selections) {
                window.paint_quad(quad);
            }
            let wrapped = std::mem::take(&mut prepaint.wrapped);
            let mut y = bounds.origin.y - prepaint.scroll_y;
            for (l, _) in &wrapped {
                l.paint(
                    point(bounds.origin.x, y),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                )
                .ok();
                y += l.size(line_height).height;
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = prepaint.cursor.take()
            {
                window.paint_quad(cursor);
            }
            let scroll_y = prepaint.scroll_y;
            self.input.update(cx, |input, _cx| {
                input.last_wrapped = wrapped;
                input.last_bounds = Some(bounds);
                input.scroll_y = scroll_y;
                input.line_height = line_height;
            });
            return;
        }
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        let origin = point(bounds.origin.x - prepaint.scroll, bounds.origin.y);
        line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
            .unwrap();

        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }

        let scroll = prepaint.scroll;
        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
            input.scroll_x = scroll;
            input.line_height = line_height;
        });
    }
}

impl Render for PixieInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        div()
            .flex()
            .key_context("PixieTextInput")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::show_character_palette))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::tab_next))
            .on_action(cx.listener(Self::tab_prev))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .px_2()
            .py_1()
            .bg(rgb(crate::theme().field_bg))
            .border_1()
            .border_color(if focused { rgb(crate::theme().accent) } else { rgb(crate::theme().surface) })
            .rounded_md()
            .child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .child(PixieInputElement { input: cx.entity() }),
            )
    }
}

impl Focusable for PixieInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_stays_on_char_boundaries() {
        let s = "aあb"; // 'あ' is 3 bytes at offset 1..4
        assert_eq!(clamp_to_boundary(s, 0), 0);
        assert_eq!(clamp_to_boundary(s, 1), 1);
        assert_eq!(clamp_to_boundary(s, 2), 1);
        assert_eq!(clamp_to_boundary(s, 3), 1);
        assert_eq!(clamp_to_boundary(s, 4), 4);
        assert_eq!(clamp_to_boundary(s, 99), 5);
    }

    #[test]
    fn sanitize_orders_and_clamps() {
        let s = "あい"; // 3-byte chars at 0..3, 3..6
        assert_eq!(sanitize_range(s, &(2..5)), 0..3);
        assert_eq!(sanitize_range(s, &(4..2)), 3..3);
        assert_eq!(sanitize_range(s, &(0..100)), 0..6);
    }

    #[test]
    fn utf16_offsets_convert_inside_new_text() {
        // "が" is one UTF-16 unit, 3 UTF-8 bytes; "🎉" is two UTF-16
        // units, 4 UTF-8 bytes.
        assert_eq!(utf16_offset_in("がぎ", 1), 3);
        assert_eq!(utf16_offset_in("🎉x", 2), 4);
        assert_eq!(utf16_offset_in("abc", 99), 3);
    }
}
