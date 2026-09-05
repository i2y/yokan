# The spinel door: the same C ABI the interpreted run opens with Fiddle,
# declared with spinel's FFI so the compiled binary calls it directly.
# Below the marked line the Ruby is word for word what
# door/cruby/wakakusa.rb says, so an app reads one source in both
# runs; the sweep compares the two halves and stops if they drift.
module PixieC
  # The two functions that take a callback get no extern from spinel (it
  # calls the header prototype), so their prototypes travel here.
  # spinel spells `:long` as C `long`; the prototypes say the same so the
  # trampolines' pointer types match (the ABI is i64 either way here).
  ffi_source <<~C
    typedef long (*pixie_build_fn)(void);
    typedef void (*pixie_event_fn)(long);
    void pixie_set_event_handler(pixie_event_fn f);
    int pixie_run(const char *title, pixie_build_fn build);
  C
  ffi_callback :build_fn, [], :long
  ffi_callback :event_fn, [:long], :void
  ffi_func :pixie_text, [:str, :double], :long
  ffi_func :pixie_button, [:str, :long], :long
  ffi_func :pixie_column, [:int_array, :size_t, :double, :double], :long
  ffi_func :pixie_row, [:int_array, :size_t, :double, :double], :long
  ffi_func :pixie_set_event_handler, [:event_fn], :void
  ffi_func :pixie_run, [:str, :build_fn], :int
end

# A list of handles crosses as the array's own storage here; the CRuby
# door packs a buffer. This is the one line the doors spell differently.
def wakakusa_ids(kids)
  kids
end

# --- the part both doors share, word for word ---------------------------

$wakakusa_handlers = []
$wakakusa_view = nil

# The engine hands a handler id back; the block that was registered runs.
def wakakusa_on_event(id)
  $wakakusa_handlers[id].call
end

# The engine asks for the tree; the registry starts over for this build.
def wakakusa_build
  $wakakusa_handlers = []
  $wakakusa_view.call
end

def text(s, size: 0.0)
  PixieC.pixie_text(s, size)
end

def button(label, &on_click)
  $wakakusa_handlers.push(on_click)
  PixieC.pixie_button(label, $wakakusa_handlers.length - 1)
end

def column(*kids, spacing: -1.0, padding: 0.0)
  PixieC.pixie_column(wakakusa_ids(kids), kids.length, spacing, padding)
end

def row(*kids, spacing: -1.0, padding: 0.0)
  PixieC.pixie_row(wakakusa_ids(kids), kids.length, spacing, padding)
end

# --- the run: spinel's trampolines take a top-level method(:name) ---------

def run(title, &view)
  $wakakusa_view = view
  PixieC.pixie_set_event_handler(method(:wakakusa_on_event))
  PixieC.pixie_run(title, method(:wakakusa_build))
end
