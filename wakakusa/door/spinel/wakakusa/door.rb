# The spinel door: the same C face the interpreted run opens with
# Fiddle, declared so the compiled binary calls it directly. Only this
# file and its twin under door/cruby differ; above them an app reads one
# source in both runs, and the sweep stops if the shared half ever
# drifts apart.
module PixieC
  # A function that takes a callback gets no extern from spinel (it
  # calls the header prototype), so those prototypes travel here.
  # spinel spells `:long` as C `long`; the prototypes say the same, so
  # the trampolines' pointer types match.
  #
  # An event hands over two numbers and nothing else: which handler,
  # and what kind of thing it carries. spinel's trampoline reads every
  # callback argument as an integer — a `double` argument arrives
  # truncated and a `const char *` arrives as its address — so the
  # payload is fetched with the three calls below, which convert
  # correctly, and its kind picks the registry to look in.
  ffi_source <<~C
    typedef long (*pixie_build_fn)(void);
    typedef void (*pixie_event_fn)(long, long);
    typedef long (*pixie_row_fn)(long, long);
    typedef void (*pixie_timer_fn)(long);
    typedef void (*pixie_task_fn)(long);
    typedef void (*pixie_pump_fn)(void);
    typedef void (*pixie_binding_fn)(long);
    void pixie_set_event_handler(pixie_event_fn f);
    void pixie_set_row_builder(pixie_row_fn f);
    void pixie_set_timer_handler(pixie_timer_fn f);
    void pixie_set_task_handler(pixie_task_fn f);
    void pixie_set_pump_handler(pixie_pump_fn f);
    void pixie_set_binding_handler(pixie_binding_fn f);
    int pixie_run(const char *title, double w, double h, double pad, pixie_build_fn build);
  C
  ffi_callback :build_fn, [], :long
  ffi_callback :event_fn, [:long, :long], :void
  ffi_callback :row_fn, [:long, :long], :long
  ffi_callback :timer_fn, [:long], :void
  ffi_callback :task_fn, [:long], :void
  ffi_callback :pump_fn, [], :void
  ffi_callback :binding_fn, [:long], :void
  ffi_func :pixie_el, [:int32], :long
  ffi_func :pixie_str, [:long, :int32, :str], :void
  ffi_func :pixie_num, [:long, :int32, :double], :void
  ffi_func :pixie_int, [:long, :int32, :long], :void
  ffi_func :pixie_bool, [:long, :int32, :int32], :void
  ffi_func :pixie_push_str, [:long, :int32, :str], :void
  ffi_func :pixie_push_num, [:long, :int32, :double], :void
  ffi_func :pixie_list_break, [:long, :int32], :void
  ffi_func :pixie_on, [:long, :int32, :long], :void
  ffi_func :pixie_rows, [:long, :int32, :long], :void
  ffi_func :pixie_children, [:long, :int_array, :size_t], :void
  ffi_func :pixie_end, [:long], :long
  ffi_func :pixie_event_int, [], :long
  ffi_func :pixie_event_num, [], :double
  ffi_func :pixie_event_text_length, [], :long
  ffi_func :pixie_event_text_char, [:long], :long
  ffi_func :pixie_set_event_handler, [:event_fn], :void
  ffi_func :pixie_set_row_builder, [:row_fn], :void
  ffi_func :pixie_set_timer_handler, [:timer_fn], :void
  ffi_func :pixie_every, [:double, :long], :void
  ffi_func :pixie_set_task_handler, [:task_fn], :void
  ffi_func :pixie_task, [], :long
  ffi_func :pixie_task_done, [:long], :void
  ffi_func :pixie_set_pump_handler, [:pump_fn], :void
  ffi_func :pixie_set_binding_handler, [:binding_fn], :void
  ffi_func :pixie_shortcut, [:str, :long], :void
  ffi_func :pixie_on_key, [:long], :void
  ffi_func :pixie_menu_item, [:str, :str, :long], :void
  ffi_func :pixie_on_file_drop, [:long], :void
  ffi_func :pixie_answer_length, [], :long
  ffi_func :pixie_answer_char, [:long], :long
  ffi_func :pixie_clipboard_set, [:str], :void
  ffi_func :pixie_clipboard_get, [], :void
  ffi_func :pixie_dialog, [:int32, :str], :void
  ffi_func :pixie_run, [:str, :double, :double, :double, :build_fn], :int
end

# The one line the doors spell differently: a list of handles crosses as
# the array's own storage here and as a packed buffer there.
def wakakusa_ids(kids)
  kids
end


# spinel's trampolines take a top-level method(:name).
def wakakusa_start(title, width, height, padding)
  PixieC.pixie_set_event_handler(method(:wakakusa_on_event))
  PixieC.pixie_set_row_builder(method(:wakakusa_row_build))
  PixieC.pixie_set_timer_handler(method(:wakakusa_tick))
  PixieC.pixie_set_task_handler(method(:wakakusa_task_done))
  PixieC.pixie_set_pump_handler(method(:wakakusa_pump))
  PixieC.pixie_set_binding_handler(method(:wakakusa_binding))
  PixieC.pixie_run(title, width, height, padding, method(:wakakusa_build))
end
