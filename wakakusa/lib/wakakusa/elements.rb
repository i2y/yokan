# Generated from elements.toml by tools/gen.rb. Do not edit by hand;
# edit the table and run `tools/gen.rb`.

# A handler keyword that was not written holds one of these. They
# are the shape a handler of that kind has, so the keyword always
# holds a block and never a nothing: a compiled run cannot call one
# whose type it had to guess. Identity is what tells them apart
# from a handler an app actually wrote.
WK_IGNORE_NONE = proc {}
WK_IGNORE_TEXT = proc { |v| }
WK_IGNORE_BOOL = proc { |v| }
WK_IGNORE_INT = proc { |v| }
WK_IGNORE_FLOAT = proc { |v| }

# One method per element. Its own keywords are spelled out; the
# keywords every element takes ride in `riders` and are applied
# below, so an element's own `width` wins over the box's.

# A run of text. `wrap` is "", "nowrap" or "ellipsis"; a background with
# padding and a radius makes a pill.
def text(text, size: 0.0, color: "", align: "", grow: 0.0, bold: false,
         italic: false, mono: false, underline: false, wrap: "", max_lines: 0,
         width: 0.0, background: "", padding: 0.0, border_radius: 0.0,
         border_width: 0.0, border_color: "", **riders)
  el = PixieC.pixie_el(WK::KIND_TEXT)
  PixieC.pixie_str(el, WK::K_TEXT, text)
  PixieC.pixie_num(el, WK::K_SIZE, size) if size != 0.0
  PixieC.pixie_str(el, WK::K_COLOR, color) if color != ""
  PixieC.pixie_str(el, WK::K_ALIGN, align) if align != ""
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  PixieC.pixie_bool(el, WK::K_BOLD, bold ? 1 : 0) if bold != false
  PixieC.pixie_bool(el, WK::K_ITALIC, italic ? 1 : 0) if italic != false
  PixieC.pixie_bool(el, WK::K_MONO, mono ? 1 : 0) if mono != false
  PixieC.pixie_bool(el, WK::K_UNDERLINE, underline ? 1 : 0) if underline != false
  PixieC.pixie_str(el, WK::K_WRAP, wrap) if wrap != ""
  PixieC.pixie_int(el, WK::K_MAX_LINES, max_lines) if max_lines != 0
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_str(el, WK::K_BACKGROUND, background) if background != ""
  PixieC.pixie_num(el, WK::K_PADDING, padding) if padding != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_RADIUS, border_radius) if border_radius != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_WIDTH, border_width) if border_width != 0.0
  PixieC.pixie_str(el, WK::K_BORDER_COLOR, border_color) if border_color != ""
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A button. The block runs when it is pressed.
def button(label, on_click: WK_IGNORE_NONE, width: 0.0, height: 0.0,
           size: 0.0, background: "", grow: 0.0, color: "",
           hover_background: "", active_background: "", border_radius: 0.0,
           border_width: 0.0, border_color: "", basis: 0.0, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_BUTTON)
  PixieC.pixie_str(el, WK::K_LABEL, label)
  if on_click.equal?(WK_IGNORE_NONE)
    wakakusa_on_none(el, WK::K_ON_CLICK, &blk) unless blk.nil?
  else
    wakakusa_proc_none(el, WK::K_ON_CLICK, on_click)
  end
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  PixieC.pixie_num(el, WK::K_SIZE, size) if size != 0.0
  PixieC.pixie_str(el, WK::K_BACKGROUND, background) if background != ""
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  PixieC.pixie_str(el, WK::K_COLOR, color) if color != ""
  PixieC.pixie_str(el, WK::K_HOVER_BACKGROUND, hover_background) if hover_background != ""
  PixieC.pixie_str(el, WK::K_ACTIVE_BACKGROUND, active_background) if active_background != ""
  PixieC.pixie_num(el, WK::K_BORDER_RADIUS, border_radius) if border_radius != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_WIDTH, border_width) if border_width != 0.0
  PixieC.pixie_str(el, WK::K_BORDER_COLOR, border_color) if border_color != ""
  PixieC.pixie_num(el, WK::K_BASIS, basis) if basis != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A line a person types into. `on_change` fires per keystroke, `on_submit`
# when they press enter; `multiline` makes it a paragraph field.
def text_field(value, placeholder: "", on_change: WK_IGNORE_TEXT,
               on_submit: WK_IGNORE_TEXT, multiline: false, rows: 0.0,
               **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_TEXT_FIELD)
  PixieC.pixie_str(el, WK::K_VALUE, value)
  PixieC.pixie_str(el, WK::K_PLACEHOLDER, placeholder) if placeholder != ""
  if on_change.equal?(WK_IGNORE_TEXT)
    wakakusa_on_text(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_text(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_proc_text(el, WK::K_ON_SUBMIT, on_submit) unless on_submit.equal?(WK_IGNORE_TEXT)
  PixieC.pixie_bool(el, WK::K_MULTILINE, multiline ? 1 : 0) if multiline != false
  PixieC.pixie_num(el, WK::K_ROWS, rows) if rows != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Its children down the page.
def column(*kids, spacing: -1.0, padding: 0.0, background: "", grow: 0.0,
           border_radius: 0.0, border_width: 0.0, border_color: "", **riders,
           &blk)
  el = PixieC.pixie_el(WK::KIND_COLUMN)
  PixieC.pixie_num(el, WK::K_SPACING, spacing) if spacing != -1.0
  PixieC.pixie_num(el, WK::K_PADDING, padding) if padding != 0.0
  PixieC.pixie_str(el, WK::K_BACKGROUND, background) if background != ""
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_RADIUS, border_radius) if border_radius != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_WIDTH, border_width) if border_width != 0.0
  PixieC.pixie_str(el, WK::K_BORDER_COLOR, border_color) if border_color != ""
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Its children across the page.
def row(*kids, spacing: -1.0, padding: 0.0, background: "", grow: 0.0,
        border_radius: 0.0, border_width: 0.0, border_color: "", **riders,
        &blk)
  el = PixieC.pixie_el(WK::KIND_ROW)
  PixieC.pixie_num(el, WK::K_SPACING, spacing) if spacing != -1.0
  PixieC.pixie_num(el, WK::K_PADDING, padding) if padding != 0.0
  PixieC.pixie_str(el, WK::K_BACKGROUND, background) if background != ""
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_RADIUS, border_radius) if border_radius != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_WIDTH, border_width) if border_width != 0.0
  PixieC.pixie_str(el, WK::K_BORDER_COLOR, border_color) if border_color != ""
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Its children on tracks. `columns` counts the tracks; `col_span` on a
# child covers more than one.
def grid(*kids, columns: 2, rows: 0, spacing: -1.0, padding: 0.0,
         background: "", grow: 0.0, border_radius: 0.0, border_width: 0.0,
         border_color: "", **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_GRID)
  PixieC.pixie_int(el, WK::K_COLUMNS, columns) if columns != 2
  PixieC.pixie_int(el, WK::K_ROWS, rows) if rows != 0
  PixieC.pixie_num(el, WK::K_SPACING, spacing) if spacing != -1.0
  PixieC.pixie_num(el, WK::K_PADDING, padding) if padding != 0.0
  PixieC.pixie_str(el, WK::K_BACKGROUND, background) if background != ""
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_RADIUS, border_radius) if border_radius != 0.0
  PixieC.pixie_num(el, WK::K_BORDER_WIDTH, border_width) if border_width != 0.0
  PixieC.pixie_str(el, WK::K_BORDER_COLOR, border_color) if border_color != ""
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# The span written out: this and `col_span:` on the child itself are the
# same tree.
def grid_cell(*kids, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_GRID_CELL)
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Its children on top of one another.
def stack(*kids, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_STACK)
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A pane that scrolls when its children do not fit.
def scroll_view(*kids, height: 0.0, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_SCROLL_VIEW)
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A pane that scrolls sideways.
def h_scroll_view(*kids, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_H_SCROLL_VIEW)
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# The first `row` child is the header; the later ones are data rows,
# shaded in alternation, in a frame that comes with the element.
def data_table(*kids, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_DATA_TABLE)
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A panel over the rest of the window while `open`.
def modal(*kids, open: true, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_MODAL)
  PixieC.pixie_bool(el, WK::K_OPEN, open ? 1 : 0) if open != true
  wakakusa_children(el, wakakusa_collect(kids, &blk))
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Rows built on demand: the builder is called for the rows in view, not
# for all of them.
def list_view(count, item_height: 24.0, height: 0.0, virtualized: true,
              grow: 0.0, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_LIST_VIEW)
  PixieC.pixie_int(el, WK::K_COUNT, count)
  wakakusa_rows(el, WK::K_ROW, &blk)
  PixieC.pixie_num(el, WK::K_ITEM_HEIGHT, item_height) if item_height != 24.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  PixieC.pixie_bool(el, WK::K_VIRTUALIZED, virtualized ? 1 : 0) if virtualized != true
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A table whose rows are built on demand, laid on tracks whose shares are
# `widths`. `on_select` receives the row clicked, `on_sort` the header.
def table(columns, count, widths: [], item_height: 24.0, height: 0.0,
          grow: 0.0, selected: -1, on_select: WK_IGNORE_INT, sort: -1,
          descending: false, on_sort: WK_IGNORE_INT, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_TABLE)
  columns.each { |v| PixieC.pixie_push_str(el, WK::K_COLUMNS, v) }
  PixieC.pixie_int(el, WK::K_COUNT, count)
  wakakusa_rows(el, WK::K_ROW, &blk)
  widths.each { |v| PixieC.pixie_push_num(el, WK::K_WIDTHS, v) }
  PixieC.pixie_num(el, WK::K_ITEM_HEIGHT, item_height) if item_height != 24.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  PixieC.pixie_int(el, WK::K_SELECTED, selected) if selected != -1
  wakakusa_proc_int(el, WK::K_ON_SELECT, on_select) unless on_select.equal?(WK_IGNORE_INT)
  PixieC.pixie_int(el, WK::K_SORT, sort) if sort != -1
  PixieC.pixie_bool(el, WK::K_DESCENDING, descending ? 1 : 0) if descending != false
  wakakusa_proc_int(el, WK::K_ON_SORT, on_sort) unless on_sort.equal?(WK_IGNORE_INT)
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A picture from a file.
def image(source, width: 0.0, height: 0.0, **riders)
  el = PixieC.pixie_el(WK::KIND_IMAGE)
  PixieC.pixie_str(el, WK::K_SOURCE, source)
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A drawing from an SVG file, painted at any size.
def svg(source, width: 0.0, height: 0.0, **riders)
  el = PixieC.pixie_el(WK::KIND_SVG)
  PixieC.pixie_str(el, WK::K_SOURCE, source)
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Bars. `min`/`max` both 0 take the range from the data; `axis` draws
# ticks and gridlines; `series` draws several groups.
def bar_chart(data = [], labels: [], width: 0.0, height: 0.0, min: 0.0,
              max: 0.0, axis: false, color: "", series: [], colors: [],
              **riders)
  el = PixieC.pixie_el(WK::KIND_BAR_CHART)
  data.each { |v| PixieC.pixie_push_num(el, WK::K_DATA, v) }
  labels.each { |v| PixieC.pixie_push_str(el, WK::K_LABELS, v) }
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  PixieC.pixie_num(el, WK::K_MIN, min) if min != 0.0
  PixieC.pixie_num(el, WK::K_MAX, max) if max != 0.0
  PixieC.pixie_bool(el, WK::K_AXIS, axis ? 1 : 0) if axis != false
  PixieC.pixie_str(el, WK::K_COLOR, color) if color != ""
  series.each do |inner|
    PixieC.pixie_list_break(el, WK::K_SERIES)
    inner.each { |v| PixieC.pixie_push_num(el, WK::K_SERIES, v) }
  end
  colors.each { |v| PixieC.pixie_push_str(el, WK::K_COLORS, v) }
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A line. Same arguments as the bars, and `series` draws several lines.
def line_chart(data = [], labels: [], width: 0.0, height: 0.0, min: 0.0,
               max: 0.0, axis: false, color: "", series: [], colors: [],
               **riders)
  el = PixieC.pixie_el(WK::KIND_LINE_CHART)
  data.each { |v| PixieC.pixie_push_num(el, WK::K_DATA, v) }
  labels.each { |v| PixieC.pixie_push_str(el, WK::K_LABELS, v) }
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  PixieC.pixie_num(el, WK::K_MIN, min) if min != 0.0
  PixieC.pixie_num(el, WK::K_MAX, max) if max != 0.0
  PixieC.pixie_bool(el, WK::K_AXIS, axis ? 1 : 0) if axis != false
  PixieC.pixie_str(el, WK::K_COLOR, color) if color != ""
  series.each do |inner|
    PixieC.pixie_list_break(el, WK::K_SERIES)
    inner.each { |v| PixieC.pixie_push_num(el, WK::K_SERIES, v) }
  end
  colors.each { |v| PixieC.pixie_push_str(el, WK::K_COLORS, v) }
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A track filled to `value` (0 to 1); `indeterminate` sweeps instead, for
# work with no known length.
def progress(value, width: 0.0, height: 0.0, label: "", indeterminate: false,
             **riders)
  el = PixieC.pixie_el(WK::KIND_PROGRESS)
  PixieC.pixie_num(el, WK::K_VALUE, value)
  PixieC.pixie_num(el, WK::K_WIDTH, width) if width != 0.0
  PixieC.pixie_num(el, WK::K_HEIGHT, height) if height != 0.0
  PixieC.pixie_str(el, WK::K_LABEL, label) if label != ""
  PixieC.pixie_bool(el, WK::K_INDETERMINATE, indeterminate ? 1 : 0) if indeterminate != false
  wakakusa_riders(el, riders, true)
  wakakusa_done(el)
end

# A box a person ticks. The block receives the new state.
def checkbox(label, checked: false, on_change: WK_IGNORE_BOOL, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_CHECKBOX)
  PixieC.pixie_str(el, WK::K_LABEL, label)
  PixieC.pixie_bool(el, WK::K_CHECKED, checked ? 1 : 0) if checked != false
  if on_change.equal?(WK_IGNORE_BOOL)
    wakakusa_on_bool(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_bool(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, true)
  wakakusa_done(el)
end

# A switch a person flips. The block receives the new state.
def switch(label, checked: false, on_change: WK_IGNORE_BOOL, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_SWITCH)
  PixieC.pixie_str(el, WK::K_LABEL, label)
  PixieC.pixie_bool(el, WK::K_CHECKED, checked ? 1 : 0) if checked != false
  if on_change.equal?(WK_IGNORE_BOOL)
    wakakusa_on_bool(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_bool(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, true)
  wakakusa_done(el)
end

# A track a person drags. The block receives the new number.
def slider(value: 0.0, min: 0.0, max: 1.0, step: 0.0,
           on_change: WK_IGNORE_FLOAT, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_SLIDER)
  PixieC.pixie_num(el, WK::K_VALUE, value) if value != 0.0
  PixieC.pixie_num(el, WK::K_MIN, min) if min != 0.0
  PixieC.pixie_num(el, WK::K_MAX, max) if max != 1.0
  PixieC.pixie_num(el, WK::K_STEP, step) if step != 0.0
  if on_change.equal?(WK_IGNORE_FLOAT)
    wakakusa_on_float(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_float(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A drop-down. The block receives the chosen index.
def select(options: [], selected: 0, on_change: WK_IGNORE_INT, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_SELECT)
  options.each { |v| PixieC.pixie_push_str(el, WK::K_OPTIONS, v) }
  PixieC.pixie_int(el, WK::K_SELECTED, selected) if selected != 0
  if on_change.equal?(WK_IGNORE_INT)
    wakakusa_on_int(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_int(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A column of radio buttons. The block receives the chosen index.
def radio_group(options: [], selected: 0, on_change: WK_IGNORE_INT, **riders,
                &blk)
  el = PixieC.pixie_el(WK::KIND_RADIO_GROUP)
  options.each { |v| PixieC.pixie_push_str(el, WK::K_OPTIONS, v) }
  PixieC.pixie_int(el, WK::K_SELECTED, selected) if selected != 0
  if on_change.equal?(WK_IGNORE_INT)
    wakakusa_on_int(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_int(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A row of joined toggle buttons. The block receives the chosen index.
def segmented(options: [], selected: 0, on_change: WK_IGNORE_INT, **riders,
              &blk)
  el = PixieC.pixie_el(WK::KIND_SEGMENTED)
  options.each { |v| PixieC.pixie_push_str(el, WK::K_OPTIONS, v) }
  PixieC.pixie_int(el, WK::K_SELECTED, selected) if selected != 0
  if on_change.equal?(WK_IGNORE_INT)
    wakakusa_on_int(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_int(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A row of tabs. The block receives the chosen index.
def tab_bar(labels: [], active: 0, on_change: WK_IGNORE_INT, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_TAB_BAR)
  labels.each { |v| PixieC.pixie_push_str(el, WK::K_LABELS, v) }
  PixieC.pixie_int(el, WK::K_ACTIVE, active) if active != 0
  if on_change.equal?(WK_IGNORE_INT)
    wakakusa_on_int(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_int(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A field for a number: enter or leaving it commits, text that is not a
# number is dropped. `min`/`max` both 0 is unbounded, `step` 0 is free.
def number_field(value, min: 0.0, max: 0.0, step: 0.0, placeholder: "",
                 on_change: WK_IGNORE_FLOAT, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_NUMBER_FIELD)
  PixieC.pixie_num(el, WK::K_VALUE, value)
  PixieC.pixie_num(el, WK::K_MIN, min) if min != 0.0
  PixieC.pixie_num(el, WK::K_MAX, max) if max != 0.0
  PixieC.pixie_num(el, WK::K_STEP, step) if step != 0.0
  PixieC.pixie_str(el, WK::K_PLACEHOLDER, placeholder) if placeholder != ""
  if on_change.equal?(WK_IGNORE_FLOAT)
    wakakusa_on_float(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_float(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# The same field for a whole number.
def int_field(value, min: 0, max: 0, step: 1, placeholder: "",
              on_change: WK_IGNORE_INT, **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_INT_FIELD)
  PixieC.pixie_int(el, WK::K_VALUE, value)
  PixieC.pixie_int(el, WK::K_MIN, min) if min != 0
  PixieC.pixie_int(el, WK::K_MAX, max) if max != 0
  PixieC.pixie_int(el, WK::K_STEP, step) if step != 1
  PixieC.pixie_str(el, WK::K_PLACEHOLDER, placeholder) if placeholder != ""
  if on_change.equal?(WK_IGNORE_INT)
    wakakusa_on_int(el, WK::K_ON_CHANGE, &blk) unless blk.nil?
  else
    wakakusa_proc_int(el, WK::K_ON_CHANGE, on_change)
  end
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Text that opens a page when clicked. There is no handler: opening a page
# is not the app's state.
def link(label, url, size: 0.0, **riders)
  el = PixieC.pixie_el(WK::KIND_LINK)
  PixieC.pixie_str(el, WK::K_LABEL, label)
  PixieC.pixie_str(el, WK::K_URL, url)
  PixieC.pixie_num(el, WK::K_SIZE, size) if size != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A turning ring, for work with no known length.
def spinner(size: 0.0, **riders)
  el = PixieC.pixie_el(WK::KIND_SPINNER)
  PixieC.pixie_num(el, WK::K_SIZE, size) if size != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# Takes the space its parent has left over; 0 is one share.
def spacer(grow: 0.0, **riders)
  el = PixieC.pixie_el(WK::KIND_SPACER)
  PixieC.pixie_num(el, WK::K_GROW, grow) if grow != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A rule across its parent: level in a column, upright in a row.
def divider(color: "", thickness: 0.0, **riders)
  el = PixieC.pixie_el(WK::KIND_DIVIDER)
  PixieC.pixie_str(el, WK::K_COLOR, color) if color != ""
  PixieC.pixie_num(el, WK::K_THICKNESS, thickness) if thickness != 0.0
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# A grid of virtual pixels, painted by the commands written in its block.
# A color here is a NUMBER: the index of a color in `palette`, which is
# how drawing code written for a pixel machine ports line for line.
# `scale` is how many logical pixels one virtual pixel takes.
def canvas(width, height, scale: 1, background: 0, palette: [], **riders, &blk)
  el = PixieC.pixie_el(WK::KIND_CANVAS)
  PixieC.pixie_int(el, WK::K_WIDTH, width)
  PixieC.pixie_int(el, WK::K_HEIGHT, height)
  PixieC.pixie_int(el, WK::K_SCALE, scale) if scale != 1
  PixieC.pixie_int(el, WK::K_BACKGROUND, background) if background != 0
  palette.each { |v| PixieC.pixie_push_str(el, WK::K_PALETTE, v) }
  wakakusa_paint(el, &blk)
  wakakusa_riders(el, riders, false)
  wakakusa_done(el)
end

# The keywords every element takes. An element that owns one of
# these names under its own meaning never gets here: Ruby binds it
# to that element's own keyword first.
def wakakusa_rider(el, name, v)
  case name
  when :width then PixieC.pixie_num(el, WK::K_WIDTH, v)
  when :height then PixieC.pixie_num(el, WK::K_HEIGHT, v)
  when :min_width then PixieC.pixie_num(el, WK::K_MIN_WIDTH, v)
  when :max_width then PixieC.pixie_num(el, WK::K_MAX_WIDTH, v)
  when :disabled then PixieC.pixie_bool(el, WK::K_DISABLED, v ? 1 : 0)
  when :theme then PixieC.pixie_str(el, WK::K_THEME, v)
  when :animate then PixieC.pixie_num(el, WK::K_ANIMATE, v)
  when :easing then PixieC.pixie_str(el, WK::K_EASING, v)
  when :enter then PixieC.pixie_bool(el, WK::K_ENTER, v ? 1 : 0)
  when :exit then PixieC.pixie_bool(el, WK::K_EXIT, v ? 1 : 0)
  when :col_span then PixieC.pixie_int(el, WK::K_COL_SPAN, v)
  when :row_span then PixieC.pixie_int(el, WK::K_ROW_SPAN, v)
  when :role then PixieC.pixie_str(el, WK::K_ROLE, v)
  when :a11y_label then PixieC.pixie_str(el, WK::K_A11Y_LABEL, v)
  when :tooltip then PixieC.pixie_str(el, WK::K_TOOLTIP, v)
  else
    raise ArgumentError, "no property `#{name}` — every element takes " \
                         "width, height, min_width, max_width, disabled, theme, animate, easing, enter, exit, col_span, row_span, role, a11y_label, tooltip"
  end
end
