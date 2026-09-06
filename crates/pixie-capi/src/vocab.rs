//! Generated from `elements.toml` beside this crate by `wakakusa/tools/gen.rb`.
//! Do not edit by hand; edit the table and run the generator.
//!
//! The numbers here are the ones the caller's side counts with, and
//! the defaults are what a property reads as when the caller left it
//! out — which is why only what an app actually wrote has to cross.

#![allow(dead_code)]

/// What a handler is called with. The engine names the kind when
/// it hands an event over, because a caller keeps one registry per
/// kind: a list of blocks all called with the same sort of value is
/// one a compiler can type, and a mixed one is not.
pub const PAY_NONE: i64 = 0;
pub const PAY_TEXT: i64 = 1;
pub const PAY_BOOL: i64 = 2;
pub const PAY_INT: i64 = 3;
pub const PAY_FLOAT: i64 = 4;

pub const KIND_TEXT: i32 = 1;
pub const KIND_BUTTON: i32 = 2;
pub const KIND_TEXT_FIELD: i32 = 3;
pub const KIND_COLUMN: i32 = 4;
pub const KIND_ROW: i32 = 5;
pub const KIND_GRID: i32 = 6;
pub const KIND_GRID_CELL: i32 = 7;
pub const KIND_STACK: i32 = 8;
pub const KIND_SCROLL_VIEW: i32 = 9;
pub const KIND_H_SCROLL_VIEW: i32 = 10;
pub const KIND_DATA_TABLE: i32 = 11;
pub const KIND_MODAL: i32 = 12;
pub const KIND_LIST_VIEW: i32 = 13;
pub const KIND_TABLE: i32 = 14;
pub const KIND_IMAGE: i32 = 15;
pub const KIND_SVG: i32 = 16;
pub const KIND_BAR_CHART: i32 = 17;
pub const KIND_LINE_CHART: i32 = 18;
pub const KIND_PROGRESS: i32 = 19;
pub const KIND_CHECKBOX: i32 = 20;
pub const KIND_SWITCH: i32 = 21;
pub const KIND_SLIDER: i32 = 22;
pub const KIND_SELECT: i32 = 23;
pub const KIND_RADIO_GROUP: i32 = 24;
pub const KIND_SEGMENTED: i32 = 25;
pub const KIND_TAB_BAR: i32 = 26;
pub const KIND_NUMBER_FIELD: i32 = 27;
pub const KIND_INT_FIELD: i32 = 28;
pub const KIND_LINK: i32 = 29;
pub const KIND_SPINNER: i32 = 30;
pub const KIND_SPACER: i32 = 31;
pub const KIND_DIVIDER: i32 = 32;
pub const KIND_CANVAS: i32 = 33;

pub const K_WIDTH: i32 = 1;
pub const K_HEIGHT: i32 = 2;
pub const K_MIN_WIDTH: i32 = 3;
pub const K_MAX_WIDTH: i32 = 4;
pub const K_DISABLED: i32 = 5;
pub const K_THEME: i32 = 6;
pub const K_ANIMATE: i32 = 7;
pub const K_EASING: i32 = 8;
pub const K_ENTER: i32 = 9;
pub const K_EXIT: i32 = 10;
pub const K_COL_SPAN: i32 = 11;
pub const K_ROW_SPAN: i32 = 12;
pub const K_ROLE: i32 = 13;
pub const K_A11Y_LABEL: i32 = 14;
pub const K_TOOLTIP: i32 = 15;
pub const K_TEXT: i32 = 16;
pub const K_SIZE: i32 = 17;
pub const K_COLOR: i32 = 18;
pub const K_ALIGN: i32 = 19;
pub const K_GROW: i32 = 20;
pub const K_BOLD: i32 = 21;
pub const K_ITALIC: i32 = 22;
pub const K_MONO: i32 = 23;
pub const K_UNDERLINE: i32 = 24;
pub const K_WRAP: i32 = 25;
pub const K_MAX_LINES: i32 = 26;
pub const K_BACKGROUND: i32 = 27;
pub const K_PADDING: i32 = 28;
pub const K_BORDER_RADIUS: i32 = 29;
pub const K_BORDER_WIDTH: i32 = 30;
pub const K_BORDER_COLOR: i32 = 31;
pub const K_LABEL: i32 = 32;
pub const K_ON_CLICK: i32 = 33;
pub const K_HOVER_BACKGROUND: i32 = 34;
pub const K_ACTIVE_BACKGROUND: i32 = 35;
pub const K_BASIS: i32 = 36;
pub const K_VALUE: i32 = 37;
pub const K_PLACEHOLDER: i32 = 38;
pub const K_ON_CHANGE: i32 = 39;
pub const K_ON_SUBMIT: i32 = 40;
pub const K_MULTILINE: i32 = 41;
pub const K_ROWS: i32 = 42;
pub const K_SPACING: i32 = 43;
pub const K_COLUMNS: i32 = 44;
pub const K_OPEN: i32 = 45;
pub const K_COUNT: i32 = 46;
pub const K_ROW: i32 = 47;
pub const K_ITEM_HEIGHT: i32 = 48;
pub const K_VIRTUALIZED: i32 = 49;
pub const K_WIDTHS: i32 = 50;
pub const K_SELECTED: i32 = 51;
pub const K_ON_SELECT: i32 = 52;
pub const K_SORT: i32 = 53;
pub const K_DESCENDING: i32 = 54;
pub const K_ON_SORT: i32 = 55;
pub const K_SOURCE: i32 = 56;
pub const K_DATA: i32 = 57;
pub const K_LABELS: i32 = 58;
pub const K_MIN: i32 = 59;
pub const K_MAX: i32 = 60;
pub const K_AXIS: i32 = 61;
pub const K_SERIES: i32 = 62;
pub const K_COLORS: i32 = 63;
pub const K_INDETERMINATE: i32 = 64;
pub const K_CHECKED: i32 = 65;
pub const K_STEP: i32 = 66;
pub const K_OPTIONS: i32 = 67;
pub const K_ACTIVE: i32 = 68;
pub const K_URL: i32 = 69;
pub const K_THICKNESS: i32 = 70;
pub const K_SCALE: i32 = 71;
pub const K_PALETTE: i32 = 72;

/// `(kind, key, value)` — what a property reads as when nobody wrote
/// it. Kind 0 is a rider, which means the same on every element.
pub const DEF_NUM: &[(i32, i32, f64)] = &[
    (KIND_TEXT, K_SIZE, 0.0),
    (KIND_TEXT, K_GROW, 0.0),
    (KIND_TEXT, K_WIDTH, 0.0),
    (KIND_TEXT, K_PADDING, 0.0),
    (KIND_TEXT, K_BORDER_RADIUS, 0.0),
    (KIND_TEXT, K_BORDER_WIDTH, 0.0),
    (KIND_BUTTON, K_WIDTH, 0.0),
    (KIND_BUTTON, K_HEIGHT, 0.0),
    (KIND_BUTTON, K_SIZE, 0.0),
    (KIND_BUTTON, K_GROW, 0.0),
    (KIND_BUTTON, K_BORDER_RADIUS, 0.0),
    (KIND_BUTTON, K_BORDER_WIDTH, 0.0),
    (KIND_BUTTON, K_BASIS, 0.0),
    (KIND_TEXT_FIELD, K_ROWS, 0.0),
    (KIND_COLUMN, K_SPACING, -1.0),
    (KIND_COLUMN, K_PADDING, 0.0),
    (KIND_COLUMN, K_GROW, 0.0),
    (KIND_COLUMN, K_BORDER_RADIUS, 0.0),
    (KIND_COLUMN, K_BORDER_WIDTH, 0.0),
    (KIND_ROW, K_SPACING, -1.0),
    (KIND_ROW, K_PADDING, 0.0),
    (KIND_ROW, K_GROW, 0.0),
    (KIND_ROW, K_BORDER_RADIUS, 0.0),
    (KIND_ROW, K_BORDER_WIDTH, 0.0),
    (KIND_GRID, K_SPACING, -1.0),
    (KIND_GRID, K_PADDING, 0.0),
    (KIND_GRID, K_GROW, 0.0),
    (KIND_GRID, K_BORDER_RADIUS, 0.0),
    (KIND_GRID, K_BORDER_WIDTH, 0.0),
    (KIND_SCROLL_VIEW, K_HEIGHT, 0.0),
    (KIND_LIST_VIEW, K_ITEM_HEIGHT, 24.0),
    (KIND_LIST_VIEW, K_HEIGHT, 0.0),
    (KIND_LIST_VIEW, K_GROW, 0.0),
    (KIND_TABLE, K_ITEM_HEIGHT, 24.0),
    (KIND_TABLE, K_HEIGHT, 0.0),
    (KIND_TABLE, K_GROW, 0.0),
    (KIND_IMAGE, K_WIDTH, 0.0),
    (KIND_IMAGE, K_HEIGHT, 0.0),
    (KIND_SVG, K_WIDTH, 0.0),
    (KIND_SVG, K_HEIGHT, 0.0),
    (KIND_BAR_CHART, K_WIDTH, 0.0),
    (KIND_BAR_CHART, K_HEIGHT, 0.0),
    (KIND_BAR_CHART, K_MIN, 0.0),
    (KIND_BAR_CHART, K_MAX, 0.0),
    (KIND_LINE_CHART, K_WIDTH, 0.0),
    (KIND_LINE_CHART, K_HEIGHT, 0.0),
    (KIND_LINE_CHART, K_MIN, 0.0),
    (KIND_LINE_CHART, K_MAX, 0.0),
    (KIND_PROGRESS, K_WIDTH, 0.0),
    (KIND_PROGRESS, K_HEIGHT, 0.0),
    (KIND_SLIDER, K_VALUE, 0.0),
    (KIND_SLIDER, K_MIN, 0.0),
    (KIND_SLIDER, K_MAX, 1.0),
    (KIND_SLIDER, K_STEP, 0.0),
    (KIND_NUMBER_FIELD, K_MIN, 0.0),
    (KIND_NUMBER_FIELD, K_MAX, 0.0),
    (KIND_NUMBER_FIELD, K_STEP, 0.0),
    (KIND_LINK, K_SIZE, 0.0),
    (KIND_SPINNER, K_SIZE, 0.0),
    (KIND_SPACER, K_GROW, 0.0),
    (KIND_DIVIDER, K_THICKNESS, 0.0),
    (0, K_ANIMATE, 0.0),
];
pub const DEF_INT: &[(i32, i32, i64)] = &[
    (KIND_TEXT, K_MAX_LINES, 0),
    (KIND_GRID, K_COLUMNS, 2),
    (KIND_GRID, K_ROWS, 0),
    (KIND_TABLE, K_SELECTED, -1),
    (KIND_TABLE, K_SORT, -1),
    (KIND_SELECT, K_SELECTED, 0),
    (KIND_RADIO_GROUP, K_SELECTED, 0),
    (KIND_SEGMENTED, K_SELECTED, 0),
    (KIND_TAB_BAR, K_ACTIVE, 0),
    (KIND_INT_FIELD, K_MIN, 0),
    (KIND_INT_FIELD, K_MAX, 0),
    (KIND_INT_FIELD, K_STEP, 1),
    (KIND_CANVAS, K_SCALE, 1),
    (KIND_CANVAS, K_BACKGROUND, 0),
    (0, K_COL_SPAN, 1),
    (0, K_ROW_SPAN, 1),
];
pub const DEF_BOOL: &[(i32, i32, bool)] = &[
    (KIND_TEXT, K_BOLD, false),
    (KIND_TEXT, K_ITALIC, false),
    (KIND_TEXT, K_MONO, false),
    (KIND_TEXT, K_UNDERLINE, false),
    (KIND_TEXT_FIELD, K_MULTILINE, false),
    (KIND_MODAL, K_OPEN, true),
    (KIND_LIST_VIEW, K_VIRTUALIZED, true),
    (KIND_TABLE, K_DESCENDING, false),
    (KIND_BAR_CHART, K_AXIS, false),
    (KIND_LINE_CHART, K_AXIS, false),
    (KIND_PROGRESS, K_INDETERMINATE, false),
    (KIND_CHECKBOX, K_CHECKED, false),
    (KIND_SWITCH, K_CHECKED, false),
    (0, K_DISABLED, false),
    (0, K_ENTER, false),
    (0, K_EXIT, false),
];
pub const DEF_STR: &[(i32, i32, &str)] = &[
    (KIND_TEXT, K_COLOR, ""),
    (KIND_TEXT, K_ALIGN, ""),
    (KIND_TEXT, K_WRAP, ""),
    (KIND_TEXT, K_BACKGROUND, ""),
    (KIND_TEXT, K_BORDER_COLOR, ""),
    (KIND_BUTTON, K_BACKGROUND, ""),
    (KIND_BUTTON, K_COLOR, ""),
    (KIND_BUTTON, K_HOVER_BACKGROUND, ""),
    (KIND_BUTTON, K_ACTIVE_BACKGROUND, ""),
    (KIND_BUTTON, K_BORDER_COLOR, ""),
    (KIND_TEXT_FIELD, K_PLACEHOLDER, ""),
    (KIND_COLUMN, K_BACKGROUND, ""),
    (KIND_COLUMN, K_BORDER_COLOR, ""),
    (KIND_ROW, K_BACKGROUND, ""),
    (KIND_ROW, K_BORDER_COLOR, ""),
    (KIND_GRID, K_BACKGROUND, ""),
    (KIND_GRID, K_BORDER_COLOR, ""),
    (KIND_BAR_CHART, K_COLOR, ""),
    (KIND_LINE_CHART, K_COLOR, ""),
    (KIND_PROGRESS, K_LABEL, ""),
    (KIND_NUMBER_FIELD, K_PLACEHOLDER, ""),
    (KIND_INT_FIELD, K_PLACEHOLDER, ""),
    (KIND_DIVIDER, K_COLOR, ""),
    (0, K_THEME, ""),
    (0, K_EASING, ""),
    (0, K_ROLE, ""),
    (0, K_A11Y_LABEL, ""),
    (0, K_TOOLTIP, ""),
];

/// Every element and every keyword the table declares, by name, so
/// a test can read the table and check that each one reached an arm.
pub const KINDS: &[(&str, i32)] = &[
    ("text", KIND_TEXT),
    ("button", KIND_BUTTON),
    ("text_field", KIND_TEXT_FIELD),
    ("column", KIND_COLUMN),
    ("row", KIND_ROW),
    ("grid", KIND_GRID),
    ("grid_cell", KIND_GRID_CELL),
    ("stack", KIND_STACK),
    ("scroll_view", KIND_SCROLL_VIEW),
    ("h_scroll_view", KIND_H_SCROLL_VIEW),
    ("data_table", KIND_DATA_TABLE),
    ("modal", KIND_MODAL),
    ("list_view", KIND_LIST_VIEW),
    ("table", KIND_TABLE),
    ("image", KIND_IMAGE),
    ("svg", KIND_SVG),
    ("bar_chart", KIND_BAR_CHART),
    ("line_chart", KIND_LINE_CHART),
    ("progress", KIND_PROGRESS),
    ("checkbox", KIND_CHECKBOX),
    ("switch", KIND_SWITCH),
    ("slider", KIND_SLIDER),
    ("select", KIND_SELECT),
    ("radio_group", KIND_RADIO_GROUP),
    ("segmented", KIND_SEGMENTED),
    ("tab_bar", KIND_TAB_BAR),
    ("number_field", KIND_NUMBER_FIELD),
    ("int_field", KIND_INT_FIELD),
    ("link", KIND_LINK),
    ("spinner", KIND_SPINNER),
    ("spacer", KIND_SPACER),
    ("divider", KIND_DIVIDER),
    ("canvas", KIND_CANVAS),
];
pub const KEYS: &[(&str, i32)] = &[
    ("width", K_WIDTH),
    ("height", K_HEIGHT),
    ("min_width", K_MIN_WIDTH),
    ("max_width", K_MAX_WIDTH),
    ("disabled", K_DISABLED),
    ("theme", K_THEME),
    ("animate", K_ANIMATE),
    ("easing", K_EASING),
    ("enter", K_ENTER),
    ("exit", K_EXIT),
    ("col_span", K_COL_SPAN),
    ("row_span", K_ROW_SPAN),
    ("role", K_ROLE),
    ("a11y_label", K_A11Y_LABEL),
    ("tooltip", K_TOOLTIP),
    ("text", K_TEXT),
    ("size", K_SIZE),
    ("color", K_COLOR),
    ("align", K_ALIGN),
    ("grow", K_GROW),
    ("bold", K_BOLD),
    ("italic", K_ITALIC),
    ("mono", K_MONO),
    ("underline", K_UNDERLINE),
    ("wrap", K_WRAP),
    ("max_lines", K_MAX_LINES),
    ("background", K_BACKGROUND),
    ("padding", K_PADDING),
    ("border_radius", K_BORDER_RADIUS),
    ("border_width", K_BORDER_WIDTH),
    ("border_color", K_BORDER_COLOR),
    ("label", K_LABEL),
    ("on_click", K_ON_CLICK),
    ("hover_background", K_HOVER_BACKGROUND),
    ("active_background", K_ACTIVE_BACKGROUND),
    ("basis", K_BASIS),
    ("value", K_VALUE),
    ("placeholder", K_PLACEHOLDER),
    ("on_change", K_ON_CHANGE),
    ("on_submit", K_ON_SUBMIT),
    ("multiline", K_MULTILINE),
    ("rows", K_ROWS),
    ("spacing", K_SPACING),
    ("columns", K_COLUMNS),
    ("open", K_OPEN),
    ("count", K_COUNT),
    ("row", K_ROW),
    ("item_height", K_ITEM_HEIGHT),
    ("virtualized", K_VIRTUALIZED),
    ("widths", K_WIDTHS),
    ("selected", K_SELECTED),
    ("on_select", K_ON_SELECT),
    ("sort", K_SORT),
    ("descending", K_DESCENDING),
    ("on_sort", K_ON_SORT),
    ("source", K_SOURCE),
    ("data", K_DATA),
    ("labels", K_LABELS),
    ("min", K_MIN),
    ("max", K_MAX),
    ("axis", K_AXIS),
    ("series", K_SERIES),
    ("colors", K_COLORS),
    ("indeterminate", K_INDETERMINATE),
    ("checked", K_CHECKED),
    ("step", K_STEP),
    ("options", K_OPTIONS),
    ("active", K_ACTIVE),
    ("url", K_URL),
    ("thickness", K_THICKNESS),
    ("scale", K_SCALE),
    ("palette", K_PALETTE),
];

/// The elements that size their own width and height,
/// so the box rider leaves that side alone.
pub const NATIVE_SIZE: &[i32] = &[KIND_BUTTON, KIND_IMAGE, KIND_SVG, KIND_BAR_CHART, KIND_LINE_CHART, KIND_PROGRESS, KIND_CANVAS];

/// The elements that size their own width,
/// so the box rider leaves that side alone.
pub const NATIVE_WIDTH: &[i32] = &[KIND_TEXT];

/// The elements that size their own height,
/// so the box rider leaves that side alone.
pub const NATIVE_HEIGHT: &[i32] = &[KIND_SCROLL_VIEW, KIND_LIST_VIEW, KIND_TABLE];
