# The calculator's look, kept where both of its layouts can read it.
KEY = { grow: 1.0, size: 20.0, background: "panel",
        hover_background: "#45475a", active_background: "#585b70" }.freeze
FUN = KEY.merge({ background: "#313244", color: "#a6adc8" }).freeze
OP = KEY.merge({ background: "#fab387", color: "#1e1e2e",
                 hover_background: "#f8c49b", active_background: "#f5e0dc" }).freeze
WIDE = KEY.merge({ grow: 2.0, basis: 8.0 }).freeze
READOUT = { size: 40.0, color: "text", align: "right", grow: 1.4 }.freeze
KEYS = { spacing: 8.0, grow: 1.0 }.freeze
