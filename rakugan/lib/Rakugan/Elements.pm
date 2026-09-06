package Rakugan::Elements;
# Generated from crates/pixie-capi/elements.toml by tools/gen.pl. Do not
# edit by hand; edit the table and run `tools/gen.pl`.
#
# The elements an app writes its screen with: one sub per row of the
# table. Positional arguments come first, then keywords and children in
# any order (`column(text(...), row(...), spacing => 8)`); the keywords
# every element takes ride along with the element's own. What each one
# does with what it was given is in Rakugan::Runtime, read off the
# table, so the subs here are the names and their sentences.
use v5.40;
use Rakugan::Runtime;
use Rakugan::Vocab;

our @ELEMENTS = qw(text button text_field column row grid grid_cell stack scroll_view h_scroll_view data_table modal list_view table image svg bar_chart line_chart progress checkbox switch slider select radio_group segmented tab_bar number_field int_field link spinner spacer divider canvas);

# A run of text. `wrap` is "", "nowrap" or "ellipsis"; a background with
# padding and a radius makes a pill.
sub text { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{text}, @_) }

# A button. The block runs when it is pressed.
sub button { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{button}, @_) }

# A line a person types into. `on_change` fires per keystroke, `on_submit`
# when they press enter; `multiline` makes it a paragraph field.
sub text_field { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{text_field}, @_) }

# Its children down the page.
sub column { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{column}, @_) }

# Its children across the page.
sub row { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{row}, @_) }

# Its children on tracks. `columns` counts the tracks; `col_span` on a
# child covers more than one.
sub grid { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{grid}, @_) }

# The span written out: this and `col_span:` on the child itself are the
# same tree.
sub grid_cell { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{grid_cell}, @_) }

# Its children on top of one another.
sub stack { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{stack}, @_) }

# A pane that scrolls when its children do not fit.
sub scroll_view { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{scroll_view}, @_) }

# A pane that scrolls sideways.
sub h_scroll_view { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{h_scroll_view}, @_) }

# The first `row` child is the header; the later ones are data rows,
# shaded in alternation, in a frame that comes with the element.
sub data_table { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{data_table}, @_) }

# A panel over the rest of the window while `open`.
sub modal { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{modal}, @_) }

# Rows built on demand: the builder is called for the rows in view, not
# for all of them.
sub list_view { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{list_view}, @_) }

# A table whose rows are built on demand, laid on tracks whose shares are
# `widths`. `on_select` receives the row clicked, `on_sort` the header.
sub table { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{table}, @_) }

# A picture from a file.
sub image { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{image}, @_) }

# A drawing from an SVG file, painted at any size.
sub svg { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{svg}, @_) }

# Bars. `min`/`max` both 0 take the range from the data; `axis` draws
# ticks and gridlines; `series` draws several groups.
sub bar_chart { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{bar_chart}, @_) }

# A line. Same arguments as the bars, and `series` draws several lines.
sub line_chart { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{line_chart}, @_) }

# A track filled to `value` (0 to 1); `indeterminate` sweeps instead, for
# work with no known length.
sub progress { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{progress}, @_) }

# A box a person ticks. The block receives the new state.
sub checkbox { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{checkbox}, @_) }

# A switch a person flips. The block receives the new state.
sub switch { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{switch}, @_) }

# A track a person drags. The block receives the new number.
sub slider { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{slider}, @_) }

# A drop-down. The block receives the chosen index.
sub select { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{select}, @_) }

# A column of radio buttons. The block receives the chosen index.
sub radio_group { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{radio_group}, @_) }

# A row of joined toggle buttons. The block receives the chosen index.
sub segmented { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{segmented}, @_) }

# A row of tabs. The block receives the chosen index.
sub tab_bar { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{tab_bar}, @_) }

# A field for a number: enter or leaving it commits, text that is not a
# number is dropped. `min`/`max` both 0 is unbounded, `step` 0 is free.
sub number_field { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{number_field}, @_) }

# The same field for a whole number.
sub int_field { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{int_field}, @_) }

# Text that opens a page when clicked. There is no handler: opening a page
# is not the app's state.
sub link { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{link}, @_) }

# A turning ring, for work with no known length.
sub spinner { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{spinner}, @_) }

# Takes the space its parent has left over; 0 is one share.
sub spacer { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{spacer}, @_) }

# A rule across its parent: level in a column, upright in a row.
sub divider { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{divider}, @_) }

# A grid of virtual pixels, painted by the commands written in its block.
# A color here is a NUMBER: the index of a color in `palette`, which is
# how drawing code written for a pixel machine ports line for line.
# `scale` is how many logical pixels one virtual pixel takes.
sub canvas { Rakugan::Runtime::element($Rakugan::Vocab::ELEMENT{canvas}, @_) }

1;
