# The CRuby door: the same C ABI the compiled run links, opened with
# Fiddle. Below the marked line the Ruby is word for word what
# door/spinel/wakakusa.rb says, so an app reads one source in both
# runs; the sweep compares the two halves and stops if they drift.
require "fiddle"
require "fiddle/import"

module PixieC
  extend Fiddle::Importer
  # The engine builds into the shared target dir, like every crate here.
  dlload ENV.fetch("PIXIE_CAPI", File.join(ENV.fetch("CARGO_TARGET_DIR", File.expand_path("~/.cache/pixie/target")), "release", "libpixie_capi.dylib"))
  extern "long pixie_text(const char*, double)"
  extern "long pixie_button(const char*, long)"
  extern "long pixie_column(void*, size_t, double, double)"
  extern "long pixie_row(void*, size_t, double, double)"
  extern "void pixie_set_event_handler(void*)"
  extern "int pixie_run(const char*, void*)"
end

# A list of handles crosses as a packed buffer here; spinel hands the
# array's own storage over. This is the one line the doors spell
# differently.
def wakakusa_ids(kids)
  kids.pack("q*")
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

# --- the run: the callbacks have to outlive the call ---------------------

WAKAKUSA_BUILD_CB = Fiddle::Closure::BlockCaller.new(Fiddle::TYPE_LONG, []) { wakakusa_build }
WAKAKUSA_EVENT_CB = Fiddle::Closure::BlockCaller.new(Fiddle::TYPE_VOID, [Fiddle::TYPE_LONG]) { |id| wakakusa_on_event(id) }

def run(title, &view)
  $wakakusa_view = view
  PixieC.pixie_set_event_handler(WAKAKUSA_EVENT_CB)
  PixieC.pixie_run(title, WAKAKUSA_BUILD_CB)
end
