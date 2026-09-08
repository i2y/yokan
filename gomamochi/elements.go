package gomamochi

// The elements, written by hand for the first demos in the shape the
// generator will write from crates/pixie-capi/elements.toml: one type
// per element, one method per keyword, the keywords every element takes
// promoted from `box`, and `emit` writing only what the app set.

import "github.com/i2y/yokan/gomamochi/internal/door"

const (
	kindText      = 1
	kindButton    = 2
	kindTextField = 3
	kindColumn    = 4
	kindRow       = 5
	kindListView  = 13

	kWidth        = 1
	kHeight       = 2
	kMinWidth     = 3
	kMaxWidth     = 4
	kDisabled     = 5
	kTheme        = 6
	kAnimate      = 7
	kEasing       = 8
	kEnter        = 9
	kExit         = 10
	kColSpan      = 11
	kRowSpan      = 12
	kRole         = 13
	kA11yLabel    = 14
	kTooltip      = 15
	kText         = 16
	kSize         = 17
	kColor        = 18
	kAlign        = 19
	kGrow         = 20
	kBold         = 21
	kItalic       = 22
	kMono         = 23
	kUnderline    = 24
	kWrap         = 25
	kMaxLines     = 26
	kBackground   = 27
	kPadding      = 28
	kBorderRadius = 29
	kBorderWidth  = 30
	kBorderColor  = 31
	kLabel        = 32
	kOnClick      = 33
	kHoverBg      = 34
	kActiveBg     = 35
	kBasis        = 36
	kValue        = 37
	kPlaceholder  = 38
	kOnChange     = 39
	kOnSubmit     = 40
	kMultiline    = 41
	kRows         = 42
	kSpacing      = 43
	kCount        = 46
	kRow          = 47
	kItemHeight   = 48
	kVirtualized  = 49
)

// --- the riders --------------------------------------------------------------

// The keyword arguments every element takes, under one name and one
// meaning. A rider that was written is written to the engine whatever
// its value — a width of 0 still makes a box, and one nobody wrote does
// not — so each remembers whether it was.
type riders struct {
	set                               uint32
	width, height, minWidth, maxWidth float64
	disabled, enter, exit             bool
	theme, easing, role, label, tip   string
	animate                           float64
	colSpan, rowSpan                  int64
}

const (
	rWidth uint32 = 1 << iota
	rHeight
	rMinWidth
	rMaxWidth
	rDisabled
	rTheme
	rAnimate
	rEasing
	rEnter
	rExit
	rColSpan
	rRowSpan
	rRole
	rA11yLabel
	rTooltip
)

func (r *riders) write(el int64) {
	if r.set&rWidth != 0 {
		door.Num(el, kWidth, r.width)
	}
	if r.set&rHeight != 0 {
		door.Num(el, kHeight, r.height)
	}
	if r.set&rMinWidth != 0 {
		door.Num(el, kMinWidth, r.minWidth)
	}
	if r.set&rMaxWidth != 0 {
		door.Num(el, kMaxWidth, r.maxWidth)
	}
	if r.set&rDisabled != 0 {
		door.Bool(el, kDisabled, r.disabled)
	}
	if r.set&rTheme != 0 {
		door.Str(el, kTheme, r.theme)
	}
	if r.set&rAnimate != 0 {
		door.Num(el, kAnimate, r.animate)
	}
	if r.set&rEasing != 0 {
		door.Str(el, kEasing, r.easing)
	}
	if r.set&rEnter != 0 {
		door.Bool(el, kEnter, r.enter)
	}
	if r.set&rExit != 0 {
		door.Bool(el, kExit, r.exit)
	}
	if r.set&rColSpan != 0 {
		door.Int(el, kColSpan, r.colSpan)
	}
	if r.set&rRowSpan != 0 {
		door.Int(el, kRowSpan, r.rowSpan)
	}
	if r.set&rRole != 0 {
		door.Str(el, kRole, r.role)
	}
	if r.set&rA11yLabel != 0 {
		door.Str(el, kA11yLabel, r.label)
	}
	if r.set&rTooltip != 0 {
		door.Str(el, kTooltip, r.tip)
	}
}

// box carries the riders and the methods that set them, promoted into
// every element; `self` is the element, so a chain keeps its type. An
// element whose own keyword has the same name (a button's width) has a
// method of its own that wins over the rider's.
type box[T any] struct {
	self T
	riders
}

func (b *box[T]) Width(v float64) T    { b.width, b.set = v, b.set|rWidth; return b.self }
func (b *box[T]) Height(v float64) T   { b.height, b.set = v, b.set|rHeight; return b.self }
func (b *box[T]) MinWidth(v float64) T { b.minWidth, b.set = v, b.set|rMinWidth; return b.self }
func (b *box[T]) MaxWidth(v float64) T { b.maxWidth, b.set = v, b.set|rMaxWidth; return b.self }
func (b *box[T]) Disabled(v bool) T    { b.disabled, b.set = v, b.set|rDisabled; return b.self }
func (b *box[T]) Theme(v string) T     { b.theme, b.set = v, b.set|rTheme; return b.self }
func (b *box[T]) Animate(v float64) T  { b.animate, b.set = v, b.set|rAnimate; return b.self }
func (b *box[T]) Easing(v string) T    { b.easing, b.set = v, b.set|rEasing; return b.self }
func (b *box[T]) Enter(v bool) T       { b.enter, b.set = v, b.set|rEnter; return b.self }
func (b *box[T]) Exit(v bool) T        { b.exit, b.set = v, b.set|rExit; return b.self }
func (b *box[T]) ColSpan(v int) T      { b.colSpan, b.set = int64(v), b.set|rColSpan; return b.self }
func (b *box[T]) RowSpan(v int) T      { b.rowSpan, b.set = int64(v), b.set|rRowSpan; return b.self }
func (b *box[T]) Role(v string) T      { b.role, b.set = v, b.set|rRole; return b.self }
func (b *box[T]) A11yLabel(v string) T { b.label, b.set = v, b.set|rA11yLabel; return b.self }
func (b *box[T]) Tooltip(v string) T   { b.tip, b.set = v, b.set|rTooltip; return b.self }

// --- text ---------------------------------------------------------------------

// TextEl is a run of text. `Wrap` is "", "nowrap" or "ellipsis"; a
// background with padding and a radius makes a pill.
type TextEl struct {
	box[*TextEl]
	text                          string
	size, grow, width, padding    float64
	color, align, wrap            string
	bold, italic, mono, underline bool
	maxLines                      int64
	background, borderColor       string
	borderRadius, borderWidth     float64
}

func Text(text string) *TextEl {
	e := &TextEl{text: text}
	e.self = e
	return e
}

func (e *TextEl) Size(v float64) *TextEl         { e.size = v; return e }
func (e *TextEl) Color(v string) *TextEl         { e.color = v; return e }
func (e *TextEl) Align(v string) *TextEl         { e.align = v; return e }
func (e *TextEl) Grow(v float64) *TextEl         { e.grow = v; return e }
func (e *TextEl) Bold(v bool) *TextEl            { e.bold = v; return e }
func (e *TextEl) Italic(v bool) *TextEl          { e.italic = v; return e }
func (e *TextEl) Mono(v bool) *TextEl            { e.mono = v; return e }
func (e *TextEl) Underline(v bool) *TextEl       { e.underline = v; return e }
func (e *TextEl) Wrap(v string) *TextEl          { e.wrap = v; return e }
func (e *TextEl) MaxLines(v int) *TextEl         { e.maxLines = int64(v); return e }
func (e *TextEl) Width(v float64) *TextEl        { e.width = v; return e }
func (e *TextEl) Background(v string) *TextEl    { e.background = v; return e }
func (e *TextEl) Padding(v float64) *TextEl      { e.padding = v; return e }
func (e *TextEl) BorderRadius(v float64) *TextEl { e.borderRadius = v; return e }
func (e *TextEl) BorderWidth(v float64) *TextEl  { e.borderWidth = v; return e }
func (e *TextEl) BorderColor(v string) *TextEl   { e.borderColor = v; return e }

func (e *TextEl) emit() int64 {
	el := door.El(kindText)
	door.Str(el, kText, e.text)
	if e.size != 0 {
		door.Num(el, kSize, e.size)
	}
	if e.color != "" {
		door.Str(el, kColor, e.color)
	}
	if e.align != "" {
		door.Str(el, kAlign, e.align)
	}
	if e.grow != 0 {
		door.Num(el, kGrow, e.grow)
	}
	if e.bold {
		door.Bool(el, kBold, true)
	}
	if e.italic {
		door.Bool(el, kItalic, true)
	}
	if e.mono {
		door.Bool(el, kMono, true)
	}
	if e.underline {
		door.Bool(el, kUnderline, true)
	}
	if e.wrap != "" {
		door.Str(el, kWrap, e.wrap)
	}
	if e.maxLines != 0 {
		door.Int(el, kMaxLines, e.maxLines)
	}
	if e.width != 0 {
		door.Num(el, kWidth, e.width)
	}
	if e.background != "" {
		door.Str(el, kBackground, e.background)
	}
	if e.padding != 0 {
		door.Num(el, kPadding, e.padding)
	}
	if e.borderRadius != 0 {
		door.Num(el, kBorderRadius, e.borderRadius)
	}
	if e.borderWidth != 0 {
		door.Num(el, kBorderWidth, e.borderWidth)
	}
	if e.borderColor != "" {
		door.Str(el, kBorderColor, e.borderColor)
	}
	e.riders.write(el)
	return door.End(el)
}

// --- button -------------------------------------------------------------------

// ButtonEl is a button; OnClick runs when it is pressed.
type ButtonEl struct {
	box[*ButtonEl]
	label                                string
	onClick                              func()
	width, height, size, grow, basis     float64
	background, color, hoverBg, activeBg string
	borderRadius, borderWidth            float64
	borderColor                          string
}

func Button(label string) *ButtonEl {
	e := &ButtonEl{label: label}
	e.self = e
	return e
}

func (e *ButtonEl) OnClick(f func()) *ButtonEl          { e.onClick = f; return e }
func (e *ButtonEl) Width(v float64) *ButtonEl           { e.width = v; return e }
func (e *ButtonEl) Height(v float64) *ButtonEl          { e.height = v; return e }
func (e *ButtonEl) Size(v float64) *ButtonEl            { e.size = v; return e }
func (e *ButtonEl) Background(v string) *ButtonEl       { e.background = v; return e }
func (e *ButtonEl) Grow(v float64) *ButtonEl            { e.grow = v; return e }
func (e *ButtonEl) Color(v string) *ButtonEl            { e.color = v; return e }
func (e *ButtonEl) HoverBackground(v string) *ButtonEl  { e.hoverBg = v; return e }
func (e *ButtonEl) ActiveBackground(v string) *ButtonEl { e.activeBg = v; return e }
func (e *ButtonEl) BorderRadius(v float64) *ButtonEl    { e.borderRadius = v; return e }
func (e *ButtonEl) BorderWidth(v float64) *ButtonEl     { e.borderWidth = v; return e }
func (e *ButtonEl) BorderColor(v string) *ButtonEl      { e.borderColor = v; return e }
func (e *ButtonEl) Basis(v float64) *ButtonEl           { e.basis = v; return e }

func (e *ButtonEl) emit() int64 {
	el := door.El(kindButton)
	door.Str(el, kLabel, e.label)
	if e.onClick != nil {
		door.Handler(el, kOnClick, e.onClick)
	}
	if e.width != 0 {
		door.Num(el, kWidth, e.width)
	}
	if e.height != 0 {
		door.Num(el, kHeight, e.height)
	}
	if e.size != 0 {
		door.Num(el, kSize, e.size)
	}
	if e.background != "" {
		door.Str(el, kBackground, e.background)
	}
	if e.grow != 0 {
		door.Num(el, kGrow, e.grow)
	}
	if e.color != "" {
		door.Str(el, kColor, e.color)
	}
	if e.hoverBg != "" {
		door.Str(el, kHoverBg, e.hoverBg)
	}
	if e.activeBg != "" {
		door.Str(el, kActiveBg, e.activeBg)
	}
	if e.borderRadius != 0 {
		door.Num(el, kBorderRadius, e.borderRadius)
	}
	if e.borderWidth != 0 {
		door.Num(el, kBorderWidth, e.borderWidth)
	}
	if e.borderColor != "" {
		door.Str(el, kBorderColor, e.borderColor)
	}
	if e.basis != 0 {
		door.Num(el, kBasis, e.basis)
	}
	e.riders.write(el)
	return door.End(el)
}

// --- text field ---------------------------------------------------------------

// TextFieldEl is a line a person types into. OnChange fires per
// keystroke, OnSubmit when they press enter; Multiline makes it a
// paragraph field.
type TextFieldEl struct {
	box[*TextFieldEl]
	value, placeholder string
	onChange, onSubmit func(string)
	multiline          bool
	rows               float64
}

func TextField(value string) *TextFieldEl {
	e := &TextFieldEl{value: value}
	e.self = e
	return e
}

func (e *TextFieldEl) Placeholder(v string) *TextFieldEl    { e.placeholder = v; return e }
func (e *TextFieldEl) OnChange(f func(string)) *TextFieldEl { e.onChange = f; return e }
func (e *TextFieldEl) OnSubmit(f func(string)) *TextFieldEl { e.onSubmit = f; return e }
func (e *TextFieldEl) Multiline(v bool) *TextFieldEl        { e.multiline = v; return e }
func (e *TextFieldEl) Rows(v float64) *TextFieldEl          { e.rows = v; return e }

func (e *TextFieldEl) emit() int64 {
	el := door.El(kindTextField)
	door.Str(el, kValue, e.value)
	if e.placeholder != "" {
		door.Str(el, kPlaceholder, e.placeholder)
	}
	if f := e.onChange; f != nil {
		door.Handler(el, kOnChange, func() { f(door.EventText()) })
	}
	if f := e.onSubmit; f != nil {
		door.Handler(el, kOnSubmit, func() { f(door.EventText()) })
	}
	if e.multiline {
		door.Bool(el, kMultiline, true)
	}
	if e.rows != 0 {
		door.Num(el, kRows, e.rows)
	}
	e.riders.write(el)
	return door.End(el)
}

// --- column and row -----------------------------------------------------------

// BoxEl lays its children out in a column or a row.
type BoxEl struct {
	box[*BoxEl]
	kind                      int32
	kids                      []Element
	spacing, padding, grow    float64
	background, borderColor   string
	borderRadius, borderWidth float64
}

// Column stacks its children top to bottom.
func Column(kids ...Element) *BoxEl { return newBox(kindColumn, kids) }

// Row lays its children out left to right.
func Row(kids ...Element) *BoxEl { return newBox(kindRow, kids) }

func newBox(kind int32, kids []Element) *BoxEl {
	e := &BoxEl{kind: kind, kids: kids, spacing: -1}
	e.self = e
	return e
}

func (e *BoxEl) Spacing(v float64) *BoxEl      { e.spacing = v; return e }
func (e *BoxEl) Padding(v float64) *BoxEl      { e.padding = v; return e }
func (e *BoxEl) Background(v string) *BoxEl    { e.background = v; return e }
func (e *BoxEl) Grow(v float64) *BoxEl         { e.grow = v; return e }
func (e *BoxEl) BorderRadius(v float64) *BoxEl { e.borderRadius = v; return e }
func (e *BoxEl) BorderWidth(v float64) *BoxEl  { e.borderWidth = v; return e }
func (e *BoxEl) BorderColor(v string) *BoxEl   { e.borderColor = v; return e }

func (e *BoxEl) emit() int64 {
	// Children first, as an argument list is evaluated in every
	// sibling language; the dumps then read the same.
	ids := make([]int64, 0, len(e.kids))
	for _, k := range e.kids {
		if k != nil {
			ids = append(ids, k.emit())
		}
	}
	el := door.El(e.kind)
	if e.spacing != -1 {
		door.Num(el, kSpacing, e.spacing)
	}
	if e.padding != 0 {
		door.Num(el, kPadding, e.padding)
	}
	if e.background != "" {
		door.Str(el, kBackground, e.background)
	}
	if e.grow != 0 {
		door.Num(el, kGrow, e.grow)
	}
	if e.borderRadius != 0 {
		door.Num(el, kBorderRadius, e.borderRadius)
	}
	if e.borderWidth != 0 {
		door.Num(el, kBorderWidth, e.borderWidth)
	}
	if e.borderColor != "" {
		door.Str(el, kBorderColor, e.borderColor)
	}
	door.Children(el, ids)
	e.riders.write(el)
	return door.End(el)
}

// --- list view ----------------------------------------------------------------

// ListViewEl builds its rows on demand: `row` is called for the rows in
// view, not for all of them, with the row's number.
type ListViewEl struct {
	box[*ListViewEl]
	count                    int64
	row                      func(int) Element
	itemHeight, height, grow float64
	virtualized              bool
}

func ListView(count int, row func(i int) Element) *ListViewEl {
	e := &ListViewEl{count: int64(count), row: row, itemHeight: 24, virtualized: true}
	e.self = e
	return e
}

func (e *ListViewEl) ItemHeight(v float64) *ListViewEl { e.itemHeight = v; return e }
func (e *ListViewEl) Height(v float64) *ListViewEl     { e.height = v; return e }
func (e *ListViewEl) Virtualized(v bool) *ListViewEl   { e.virtualized = v; return e }
func (e *ListViewEl) Grow(v float64) *ListViewEl       { e.grow = v; return e }

func (e *ListViewEl) emit() int64 {
	el := door.El(kindListView)
	door.Int(el, kCount, e.count)
	if row := e.row; row != nil {
		door.RowBuilder(el, kRow, func(i int64) int64 {
			r := row(int(i))
			if r == nil {
				return 0
			}
			return r.emit()
		})
	}
	if e.itemHeight != 24 {
		door.Num(el, kItemHeight, e.itemHeight)
	}
	if e.height != 0 {
		door.Num(el, kHeight, e.height)
	}
	if !e.virtualized {
		door.Bool(el, kVirtualized, false)
	}
	if e.grow != 0 {
		door.Num(el, kGrow, e.grow)
	}
	e.riders.write(el)
	return door.End(el)
}
