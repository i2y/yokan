# The CRuby door: the same C face the compiled run links, opened with
# Fiddle. Only this file and its twin under door/spinel differ; above
# them an app reads one source in both runs, and the sweep stops if the
# shared half ever drifts apart.
require "fiddle"
require "fiddle/import"

module PixieC
  extend Fiddle::Importer
  # The engine builds into the shared target dir, like every crate here.
  dlload ENV.fetch("PIXIE_CAPI", File.join(ENV.fetch("CARGO_TARGET_DIR", File.expand_path("~/.cache/pixie/target")), "release", "libpixie_capi.dylib"))
  extern "long pixie_el(int)"
  extern "void pixie_str(long, int, const char*)"
  extern "void pixie_num(long, int, double)"
  extern "void pixie_int(long, int, long)"
  extern "void pixie_bool(long, int, int)"
  extern "void pixie_push_str(long, int, const char*)"
  extern "void pixie_push_num(long, int, double)"
  extern "void pixie_list_break(long, int)"
  extern "void pixie_on(long, int, long)"
  extern "void pixie_rows(long, int, long)"
  extern "void pixie_children(long, void*, size_t)"
  extern "long pixie_end(long)"
  extern "long pixie_event_int(void)"
  extern "double pixie_event_num(void)"
  extern "long pixie_event_text_length(void)"
  extern "long pixie_event_text_char(long)"
  extern "void pixie_set_event_handler(void*)"
  extern "void pixie_set_row_builder(void*)"
  extern "void pixie_set_timer_handler(void*)"
  extern "void pixie_watch(const char*, void*)"
  extern "void pixie_set_task_handler(void*)"
  extern "long pixie_task(void)"
  extern "void pixie_task_done(long)"
  extern "void pixie_set_pump_handler(void*)"
  extern "void pixie_every(double, long)"
  extern "int pixie_run(const char*, double, double, double, void*)"
end

# The one line the doors spell differently: a list of handles crosses as
# a packed buffer here and as the array's own storage there.
def wakakusa_ids(kids)
  kids.pack("q*")
end


# The callbacks have to outlive the call that registers them.
WAKAKUSA_BUILD_CB = Fiddle::Closure::BlockCaller.new(
  Fiddle::TYPE_LONG, []
) { wakakusa_build }
WAKAKUSA_EVENT_CB = Fiddle::Closure::BlockCaller.new(
  Fiddle::TYPE_VOID, [Fiddle::TYPE_LONG, Fiddle::TYPE_LONG]
) { |id, kind| wakakusa_on_event(id, kind) }
WAKAKUSA_ROW_CB = Fiddle::Closure::BlockCaller.new(
  Fiddle::TYPE_LONG, [Fiddle::TYPE_LONG, Fiddle::TYPE_LONG]
) { |h, i| wakakusa_row_build(h, i) }
WAKAKUSA_TIMER_CB = Fiddle::Closure::BlockCaller.new(
  Fiddle::TYPE_VOID, [Fiddle::TYPE_LONG]
) { |id| wakakusa_tick(id) }
WAKAKUSA_TASK_CB = Fiddle::Closure::BlockCaller.new(
  Fiddle::TYPE_VOID, [Fiddle::TYPE_LONG]
) { |id| wakakusa_task_done(id) }
WAKAKUSA_PUMP_CB = Fiddle::Closure::BlockCaller.new(Fiddle::TYPE_VOID, []) { wakakusa_pump }
WAKAKUSA_RELOAD_CB = Fiddle::Closure::BlockCaller.new(
  Fiddle::TYPE_INT, []
) { wakakusa_reload }

# The app's file changed. Reading it again redefines the app's class,
# and the object the window is holding is an instance of that same
# class — so it answers with the new `view` and keeps every value it
# had. The `run` at the bottom of the file returns at once.
#
# A file that does not parse leaves the window on what it had; the
# error goes to the terminal and the next save is tried afresh.
def wakakusa_reload
  load($0)
  1
rescue ScriptError, StandardError => e
  warn "wakakusa: #{e.class}: #{e.message}"
  0
end

def wakakusa_start(title, width, height, padding)
  PixieC.pixie_set_event_handler(WAKAKUSA_EVENT_CB)
  PixieC.pixie_set_row_builder(WAKAKUSA_ROW_CB)
  PixieC.pixie_set_timer_handler(WAKAKUSA_TIMER_CB)
  PixieC.pixie_set_task_handler(WAKAKUSA_TASK_CB)
  PixieC.pixie_set_pump_handler(WAKAKUSA_PUMP_CB)
  # Only the interpreted run watches: a compiled app is what it is.
  PixieC.pixie_watch($0, WAKAKUSA_RELOAD_CB) if File.file?($0)
  PixieC.pixie_run(title, width, height, padding, WAKAKUSA_BUILD_CB)
end
