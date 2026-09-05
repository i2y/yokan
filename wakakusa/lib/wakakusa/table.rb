# Generated from elements.toml by tools/gen.rb. Do not edit by hand;
# edit the table and run `tools/gen.rb`.

# What the checker reads: which keywords each element takes, and
# which of them name a handler.
module WK
  RIDERS = %w[width height min_width max_width disabled theme animate easing enter exit col_span row_span role a11y_label tooltip].freeze

  ELEMENTS = {
    "text" => %w[a11y_label align animate background bold border_color border_radius border_width col_span color disabled easing enter exit grow height italic max_lines max_width min_width mono padding role row_span size text theme tooltip underline width wrap],
    "button" => %w[a11y_label active_background animate background basis border_color border_radius border_width col_span color disabled easing enter exit grow height hover_background label max_width min_width on_click role row_span size theme tooltip width],
    "text_field" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width multiline on_change on_submit placeholder role row_span rows theme tooltip value width],
    "column" => %w[a11y_label animate background border_color border_radius border_width col_span disabled easing enter exit grow height max_width min_width padding role row_span spacing theme tooltip width],
    "row" => %w[a11y_label animate background border_color border_radius border_width col_span disabled easing enter exit grow height max_width min_width padding role row_span spacing theme tooltip width],
    "grid" => %w[a11y_label animate background border_color border_radius border_width col_span columns disabled easing enter exit grow height max_width min_width padding role row_span rows spacing theme tooltip width],
    "grid_cell" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span theme tooltip width],
    "stack" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span theme tooltip width],
    "scroll_view" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span theme tooltip width],
    "h_scroll_view" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span theme tooltip width],
    "data_table" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span theme tooltip width],
    "modal" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width open role row_span theme tooltip width],
    "list_view" => %w[a11y_label animate col_span count disabled easing enter exit grow height item_height max_width min_width role row_span theme tooltip virtualized width],
    "table" => %w[a11y_label animate col_span columns count descending disabled easing enter exit grow height item_height max_width min_width on_select on_sort role row_span selected sort theme tooltip width widths],
    "image" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span source theme tooltip width],
    "svg" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span source theme tooltip width],
    "bar_chart" => %w[a11y_label animate axis col_span color colors data disabled easing enter exit height labels max max_width min min_width role row_span series theme tooltip width],
    "line_chart" => %w[a11y_label animate axis col_span color colors data disabled easing enter exit height labels max max_width min min_width role row_span series theme tooltip width],
    "progress" => %w[animate col_span disabled easing enter exit height indeterminate label max_width min_width role row_span theme tooltip value width],
    "checkbox" => %w[animate checked col_span disabled easing enter exit height label max_width min_width on_change role row_span theme tooltip width],
    "switch" => %w[animate checked col_span disabled easing enter exit height label max_width min_width on_change role row_span theme tooltip width],
    "slider" => %w[a11y_label animate col_span disabled easing enter exit height max max_width min min_width on_change role row_span step theme tooltip value width],
    "select" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width on_change options role row_span selected theme tooltip width],
    "radio_group" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width on_change options role row_span selected theme tooltip width],
    "segmented" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width on_change options role row_span selected theme tooltip width],
    "tab_bar" => %w[a11y_label active animate col_span disabled easing enter exit height labels max_width min_width on_change role row_span theme tooltip width],
    "number_field" => %w[a11y_label animate col_span disabled easing enter exit height max max_width min min_width on_change placeholder role row_span step theme tooltip value width],
    "int_field" => %w[a11y_label animate col_span disabled easing enter exit height max max_width min min_width on_change placeholder role row_span step theme tooltip value width],
    "link" => %w[a11y_label animate col_span disabled easing enter exit height label max_width min_width role row_span size theme tooltip url width],
    "spinner" => %w[a11y_label animate col_span disabled easing enter exit height max_width min_width role row_span size theme tooltip width],
    "spacer" => %w[a11y_label animate col_span disabled easing enter exit grow height max_width min_width role row_span theme tooltip width],
    "divider" => %w[a11y_label animate col_span color disabled easing enter exit height max_width min_width role row_span theme thickness tooltip width],
  }.freeze

  HANDLERS = %w[on_change on_click on_select on_sort on_submit].freeze
end
