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
//!
//! `Element::NumberField` and `Element::IntField` are this same
//! editor in NUMERIC mode (`Numeric`, installed by the render pass)
//! rather than a second implementation: the shown text is the bound
//! number's own spelling, keystrokes report nothing, and `enter`, an
//! up/down arrow or leaving the field COMMITS — parse with Python's
//! `float()` / `int()` rules, clamp, snap, fire only on a real
//! change. Text that is not a number puts the bound value back.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, KeyBinding, LayoutId, MouseButton,
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
        StepUp,
        StepDown,
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
        // The number fields' spinner keys. Bound for every field
        // because the key context is shared; a text field ignores
        // them.
        KeyBinding::new("up", StepUp, CTX),
        KeyBinding::new("down", StepDown, CTX),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, CTX),
    ]);
}

/// App-level callback installed by the render pass: routes a committed
/// edit (or a submit) back into the World through the root entity.
pub type TextCallback = Rc<dyn Fn(&str, &mut App)>;

/// The numeric fields' callback: the payload is the parsed, clamped
/// and snapped NUMBER, not the text that produced it. The int field
/// rounds it back to an `i64` in the closure the render pass builds,
/// so one editor serves both.
pub type NumberCallback = Rc<dyn Fn(f64, &mut App)>;

/// What turns this editor into a `NumberField` / `IntField`. `None`
/// is a plain TextField, which is why every rule below reads as an
/// early return: the text half is untouched.
#[derive(Clone, Copy, PartialEq)]
pub struct Numeric {
    pub min: f64,
    pub max: f64,
    pub step: f64,
    /// Python's `int()` rules for the parse and integer arithmetic
    /// for the clamp and the snap.
    pub int: bool,
}

impl Numeric {
    /// The one text a bound value is ever shown as — `str(value)` for
    /// the float field, the plain integer for the int one.
    pub fn show(&self, v: f64) -> String {
        if self.int {
            format!("{}", v as i64)
        } else {
            pixie_kernel::py_float_repr(v)
        }
    }

    /// Python's `float()` / `int()` on the typed text: `None` is
    /// "this is not a number", which commits nothing.
    fn parse(&self, text: &str) -> Option<f64> {
        if self.int {
            pixie_kernel::parse_int_text(text).map(|v| v as f64)
        } else {
            pixie_kernel::parse_float_text(text)
        }
    }

    /// Clamp into the range and snap onto the step grid — the same
    /// two kernel functions the headless `input:` step runs, so a
    /// typed commit and a scripted one land on identical values.
    fn snap(&self, v: f64) -> f64 {
        if self.int {
            pixie_kernel::int_snap(
                self.min as i64,
                self.max as i64,
                self.step as i64,
                v as i64,
            ) as f64
        } else {
            pixie_kernel::number_snap(self.min, self.max, self.step, v)
        }
    }

    /// One arrow press. An unset step means the smallest useful move:
    /// 1 either way.
    fn arrow(&self, from: f64, up: bool) -> f64 {
        let d = if self.step > 0.0 { self.step } else { 1.0 };
        self.snap(if up { from + d } else { from - d })
    }

    /// Finishing an edit, as a VALUE: the text the field should show
    /// afterwards, and the number to report (`None` = report nothing).
    /// Pure, so the rule this widget exists for — junk reverts,
    /// numbers clamp and snap, an unchanged number is not an event —
    /// is checked without a window.
    fn commit(&self, typed: &str, bound: f64) -> (String, Option<f64>) {
        match self.parse(typed) {
            // Not a number: nothing is committed and the field goes
            // back to showing what the app holds.
            None => (self.show(bound), None),
            Some(v) => {
                let v = self.snap(v);
                (self.show(v), (v != bound).then_some(v))
            }
        }
    }
}

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
    last_bounds: Option<Bounds<Pixels>>,
    /// Horizontal scroll keeping the caret in view when the text is
    /// wider than the field (a cute_ui gap closed in the port).
    scroll_x: Pixels,
    is_selecting: bool,
    pub on_commit: Option<TextCallback>,
    pub on_submit: Option<TextCallback>,
    pub next_focus: Option<FocusHandle>,
    pub prev_focus: Option<FocusHandle>,
    /// Numeric mode, installed by the render pass each rebuild.
    pub numeric: Option<Numeric>,
    /// The number the element tree carries, in numeric mode — what
    /// the shown text renders and what a commit compares against, so
    /// `on_number` fires only on a real change.
    pub bound_num: f64,
    pub on_number: Option<NumberCallback>,
    /// Leaving the field commits, exactly as `enter` does. The
    /// subscription is registered on the first render (the one place
    /// an entity has a `&mut Window` in reach) and lives as long as
    /// the editor.
    blur: Option<gpui::Subscription>,
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
            last_bounds: None,
            scroll_x: px(0.),
            is_selecting: false,
            on_commit: None,
            on_submit: None,
            next_focus: None,
            prev_focus: None,
            numeric: None,
            bound_num: 0.0,
            on_number: None,
            blur: None,
        }
    }

    /// Reconcile against the element tree after a rebuild. The bound
    /// value is applied only when it differs from the previously bound
    /// one (and never mid-composition, where the IME owns the text).
    pub fn sync(&mut self, value: &str, placeholder: &str, cx: &mut Context<Self>) {
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
        // A number field does not report every keystroke: typing "1"
        // on the way to "12" is not a value of 1. It reports when the
        // edit is FINISHED — `enter`, an arrow, or leaving the field.
        if self.numeric.is_some() {
            return;
        }
        if let Some(cb) = self.on_commit.clone() {
            let text = self.content.to_string();
            cb(&text, cx);
        }
    }

    fn submit(&mut self, _: &Submit, _window: &mut Window, cx: &mut Context<Self>) {
        if self.numeric.is_some() {
            self.commit_number(cx);
            return;
        }
        if let Some(cb) = self.on_submit.clone() {
            let text = self.content.to_string();
            cb(&text, cx);
        }
    }

    /// Finish a numeric edit. Text that parses is clamped, snapped
    /// and shown back in its canonical spelling (`" 02.50 "` becomes
    /// `2.5`); text that does not parse commits nothing and the field
    /// returns to the value it is bound to. The handler runs only
    /// when the snapped number actually differs from that value —
    /// the Slider's rule, so re-committing the same number is not an
    /// event.
    fn commit_number(&mut self, cx: &mut Context<Self>) {
        let Some(num) = self.numeric else {
            return;
        };
        let (shown, report) = num.commit(&self.content, self.bound_num);
        self.set_text(shown, cx);
        if let Some(v) = report {
            if let Some(cb) = self.on_number.clone() {
                cb(v, cx);
            }
        }
    }

    /// Up / Down: one step from the value the field currently shows,
    /// committed at once — a keyboard spinner. Unparsable text steps
    /// from the bound value instead, which is where a failed commit
    /// would have put the field anyway.
    fn step_by(&mut self, up: bool, cx: &mut Context<Self>) {
        let Some(num) = self.numeric else {
            return;
        };
        let from = num.parse(&self.content).unwrap_or(self.bound_num);
        let v = num.arrow(from, up);
        self.set_text(num.show(v), cx);
        if v != self.bound_num {
            if let Some(cb) = self.on_number.clone() {
                cb(v, cx);
            }
        }
    }

    fn step_up(&mut self, _: &StepUp, _: &mut Window, cx: &mut Context<Self>) {
        self.step_by(true, cx);
    }

    fn step_down(&mut self, _: &StepDown, _: &mut Window, cx: &mut Context<Self>) {
        self.step_by(false, cx);
    }

    /// Replace the whole content and park the caret at its end.
    fn set_text(&mut self, text: String, cx: &mut Context<Self>) {
        if self.content.as_str() == text {
            return;
        }
        self.content = SharedString::from(text);
        let end = self.content.len();
        self.selected_range = end..end;
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
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
        self.move_to(0, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(self.content.len(), cx);
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

        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
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
    scroll: Pixels,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
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
        style.size.height = window.line_height().into();
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
            scroll,
            cursor,
            selection,
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
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let line = prepaint.line.take().unwrap();
        let origin = point(bounds.origin.x - prepaint.scroll, bounds.origin.y);
        line.paint(origin, window.line_height(), gpui::TextAlign::Left, None, window, cx)
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
        });
    }
}

impl Render for PixieInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Blur commits a numeric edit. `Context::on_blur` needs a
        // window, and `new` has none, so the subscription is armed on
        // the first paint and then held for the entity's life.
        if self.blur.is_none() {
            let handle = self.focus_handle.clone();
            let sub = cx.on_blur(&handle, window, |this, _window, cx| {
                if this.numeric.is_some() {
                    this.commit_number(cx);
                }
            });
            self.blur = Some(sub);
        }
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
            .on_action(cx.listener(Self::step_up))
            .on_action(cx.listener(Self::step_down))
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

    /// The whole rule a number field exists for, without a window:
    /// what `enter` (or a blur) leaves on screen, and what it
    /// reports. `None` is "no event" — either the text was not a
    /// number, or it snapped back onto the value already bound.
    #[test]
    fn committing_a_number_field() {
        let f = Numeric {
            min: 0.0,
            max: 10.0,
            step: 0.5,
            int: false,
        };
        // A number in range: shown in its canonical spelling, and
        // reported once.
        assert_eq!(f.commit("2.5", 0.0), ("2.5".into(), Some(2.5)));
        // Whitespace and a redundant zero are Python's to ignore.
        assert_eq!(f.commit(" 02.50 ", 0.0), ("2.5".into(), Some(2.5)));
        // Off the grid snaps; out of range clamps.
        assert_eq!(f.commit("2.7", 0.0), ("2.5".into(), Some(2.5)));
        assert_eq!(f.commit("500", 0.0), ("10.0".into(), Some(10.0)));
        // Junk commits NOTHING and the field returns to the bound
        // value — the reading the widget is for.
        assert_eq!(f.commit("abc", 2.5), ("2.5".into(), None));
        assert_eq!(f.commit("", 2.5), ("2.5".into(), None));
        assert_eq!(f.commit("2.5kg", 2.5), ("2.5".into(), None));
        // The same number again is not an event.
        assert_eq!(f.commit("2.5", 2.5), ("2.5".into(), None));
        // A whole number keeps Python's trailing `.0`.
        assert_eq!(f.commit("3", 0.0), ("3.0".into(), Some(3.0)));

        let i = Numeric {
            min: 1.0,
            max: 99.0,
            step: 1.0,
            int: true,
        };
        assert_eq!(i.commit("3", 1.0), ("3".into(), Some(3.0)));
        assert_eq!(i.commit("500", 3.0), ("99".into(), Some(99.0)));
        assert_eq!(i.commit("abc", 3.0), ("3".into(), None));
        // Python's `int()` refuses a decimal point, so this reverts.
        assert_eq!(i.commit("3.0", 3.0), ("3".into(), None));
    }

    /// Up / Down move one step and land on the grid, never outside
    /// the range.
    #[test]
    fn the_arrow_keys_step_and_stay_in_range() {
        let f = Numeric {
            min: 0.0,
            max: 10.0,
            step: 0.5,
            int: false,
        };
        assert_eq!(f.arrow(2.5, true), 3.0);
        assert_eq!(f.arrow(2.5, false), 2.0);
        assert_eq!(f.arrow(10.0, true), 10.0);
        assert_eq!(f.arrow(0.0, false), 0.0);
        // No step: one, either way.
        let free = Numeric {
            min: 0.0,
            max: 0.0,
            step: 0.0,
            int: false,
        };
        assert_eq!(free.arrow(2.5, true), 3.5);
        assert_eq!(free.arrow(-1.0, false), -2.0);
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
