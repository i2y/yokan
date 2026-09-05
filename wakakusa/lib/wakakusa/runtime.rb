# What the door holds between builds, and the calls the generated
# element methods make.
#
# There is one registry per kind of handler rather than one for all of
# them: a list of blocks that are all called with the same sort of value
# is a list a compiler can type, and a mixed one is not. The engine
# names the kind when it hands an event over, so the right registry is
# always the one that answers.
#
# A build starts the registries over, so a handler number means
# something only inside the build that handed it out. The engine holds
# no Ruby object at all: it has numbers, and asks for the rest.

# One registry, holding work to do rather than handlers to hand a value
# to. Every entry is a block of no arguments: the one an app wrote,
# wrapped in one that fetches what the event carried and passes it on.
#
# The reason is the compiler's. A list of blocks reached by index is a
# polymorphic list, and a call into one can only carry numbers — a
# string handed that way is read as an address, silently. Nothing but
# the wrapper ever crosses that call, and it takes no arguments at all.
$wakakusa_handlers = []
$wakakusa_rows = []
# The children being collected, when an element is being written as a
# block. Empty means the app is writing its children as arguments, and
# then nothing below does anything at all.
$wakakusa_frames = []
$wakakusa_row_index = 0
$wakakusa_app = nil
# Set once the window is up. A reload reads the app's file again, and
# the `run` at the bottom of it must not start a second one or throw
# away the object whose state the person has been building up.
$wakakusa_running = false
# What the app asked to be told about before it started. These outlive
# every build, so they are numbered in a list of their own.
$wakakusa_bindings = []
# Work the app started, and what it answered. The lock is the one place
# the window's thread and a worker meet.
$wakakusa_task_dones = []
$wakakusa_task_results = []
$wakakusa_task_lock = Mutex.new
$wakakusa_task_current = nil
# Timers are declared before the app runs and live for as long as it
# does, so they keep a list of their own that a build never clears.
$wakakusa_timers = []

# The text an event carried, built from the characters the engine counts
# out. It does not cross as a string for the same reason.
def wakakusa_event_text
  points = []
  n = PixieC.pixie_event_text_length
  i = 0
  while i < n
    points.push(PixieC.pixie_event_text_char(i))
    i += 1
  end
  points.pack("U*")
end

# The engine hands back the number an element carried; the work that was
# registered under it runs.
def wakakusa_on_event(id, _kind)
  $wakakusa_handlers[id].call
end

# Run `work` off the window's thread. When it is done the app's
# `on_done` method is called ON the window's thread, with what the work
# answered. Nothing inside the work may touch the app's state or the
# screen — the handler is where that belongs, and it is the reason the
# answer comes back this way rather than being written from the worker.
def task(&work)
  return 0 if work.nil?

  id = PixieC.pixie_task
  Thread.new do
    answer = work.call
    $wakakusa_task_lock.synchronize { $wakakusa_task_results[id] = answer }
    PixieC.pixie_task_done(id)
  end
  id
end

# What to do when that work is finished. The block runs on the window's
# thread, and `task_answer` inside it is what the work answered.
def on_done(job, &blk)
  return if blk.nil?

  $wakakusa_task_dones[job] = blk
end

# The engine is waiting for work and is handing us a turn. Without it
# nothing else in this program would ever run: the engine is on the
# stack from `run` until the window closes, and a thread scheduled by
# Ruby's own runtime gets no turn while that is true.
def wakakusa_pump
  Thread.pass
end

# The engine says a piece of work is finished, on the window's thread.
# The block is called with nothing and asks for the answer itself: what
# the work answered has no type known in advance, and a call whose
# argument cannot be typed is a call a compiled run cannot make.
def wakakusa_task_done(id)
  blk = $wakakusa_task_dones[id]
  return if blk.nil?

  $wakakusa_task_current = id.to_i
  blk.call
end

# What the work answered. Read it from the handler `on_done:` names.
def task_answer
  $wakakusa_task_lock.synchronize { $wakakusa_task_results[$wakakusa_task_current] }
end

# What the event now being delivered carried. A block is handed it, and
# a proc given through a keyword asks for it here.
def event_text
  wakakusa_event_text
end

def event_number
  PixieC.pixie_event_num
end

def event_index
  PixieC.pixie_event_int
end

def event_on?
  PixieC.pixie_event_int != 0
end

# A shortcut, a menu item, a key or a dropped file reached the app.
def wakakusa_binding(id)
  $wakakusa_bindings[id].call
end

def wakakusa_bind(work)
  $wakakusa_bindings.push(work)
  $wakakusa_bindings.length - 1
end

# A chord, spelled the way the platform spells it ("cmd+s").
def shortcut(chord, &blk)
  return if blk.nil?

  PixieC.pixie_shortcut(chord, wakakusa_bind(proc { blk.call }))
end

# Every key, which arrives as the chord it was.
def on_key(&blk)
  return if blk.nil?

  PixieC.pixie_on_key(wakakusa_bind(proc { blk.call(wakakusa_event_text) }))
end

# One item in the application's menu bar. Declaration order is menu
# order.
def menu_item(menu, item, &blk)
  return if blk.nil?

  PixieC.pixie_menu_item(menu, item, wakakusa_bind(proc { blk.call }))
end

# What happens to a file dragged onto the window: the block is told its
# path.
def on_file_drop(&blk)
  return if blk.nil?

  PixieC.pixie_on_file_drop(wakakusa_bind(proc { blk.call(wakakusa_event_text) }))
end

# The last string the engine was asked for, a character at a time. See
# `wakakusa_event_text` for why it does not simply come back whole.
def wakakusa_answer
  points = []
  n = PixieC.pixie_answer_length
  i = 0
  while i < n
    points.push(PixieC.pixie_answer_char(i))
    i += 1
  end
  points.pack("U*")
end

# The system clipboard. A window exchanges it with the platform; a
# headless run keeps it to itself, so a copy and a paste are a checked
# interaction like any other.
#
# Plain methods rather than a `Clipboard` module: everything here shares
# a namespace with the app, and an app is entitled to call a class of
# its own `Dialog`.
def clipboard_set(text)
  PixieC.pixie_clipboard_set(text)
end

def clipboard_get
  PixieC.pixie_clipboard_get
  wakakusa_answer
end

# The platform's own panels. A dialog waits for a person, so it belongs
# inside `task`; a headless script answers one with `file:<path>`.
def open_dialog(title = "")
  PixieC.pixie_dialog(0, title)
  wakakusa_answer
end

def save_dialog(name = "")
  PixieC.pixie_dialog(1, name)
  wakakusa_answer
end

# A database. Both runs call one implementation, which is the whole
# reason it is reached through the engine rather than through a gem.
#
# Write `?` in the statement and put the values beside it: text a person
# typed can never become part of the statement that way. Every value
# comes back as text, and the column's affinity converts on the way in.
def sqlite_exec(path, sql, params = [])
  params.each { |v| PixieC.pixie_sqlite_bind(v.to_s) }
  PixieC.pixie_sqlite_exec(path, sql)
end

def sqlite_rows(path, sql, params = [])
  params.each { |v| PixieC.pixie_sqlite_bind(v.to_s) }
  count = PixieC.pixie_sqlite_query(path, sql)
  width = PixieC.pixie_sqlite_columns
  rows = []
  r = 0
  while r < count
    cells = []
    c = 0
    while c < width
      PixieC.pixie_sqlite_cell(r, c)
      cells.push(wakakusa_answer)
      c += 1
    end
    rows.push(cells)
    r += 1
  end
  rows
end

# The first column of each row, which is what most queries want.
def sqlite_column(path, sql, params = [])
  sqlite_rows(path, sql, params).map { |cells| cells[0] }
end

# A timer came due.
def wakakusa_tick(id)
  $wakakusa_timers[id].call
end

# Ask to be told every `seconds`. Declared before `run`; both runs tick
# off the same clock, a frame in a window and an `advance:` in a script.
def every(seconds, &blk)
  return if blk.nil?

  $wakakusa_timers.push(blk)
  PixieC.pixie_every(seconds, $wakakusa_timers.length - 1)
end

# The engine asks for one row of a list that builds its rows on demand.
# The row number is left where the wrapper can read it rather than
# handed to the call: a list of blocks reached by index can only be
# called with nothing at all.
def wakakusa_row_build(handler, index)
  $wakakusa_row_index = index.to_i
  $wakakusa_rows[handler].call.to_i
end

# The engine asks for the tree.
def wakakusa_build
  $wakakusa_handlers = []
  $wakakusa_rows = []
  $wakakusa_frames = []
  $wakakusa_app.view
end

def wakakusa_work(el, key, work)
  $wakakusa_handlers.push(work)
  PixieC.pixie_on(el, key, $wakakusa_handlers.length - 1)
end

# One registration per kind of thing a handler is told, each wrapping
# the app's block in the fetch it needs.
def wakakusa_on_none(el, key, &blk)
  return if blk.nil?

  wakakusa_work(el, key, proc { blk.call })
end

def wakakusa_on_text(el, key, &blk)
  return if blk.nil?

  wakakusa_work(el, key, proc { blk.call(wakakusa_event_text) })
end

def wakakusa_on_bool(el, key, &blk)
  return if blk.nil?

  wakakusa_work(el, key, proc { blk.call(PixieC.pixie_event_int != 0) })
end

def wakakusa_on_int(el, key, &blk)
  return if blk.nil?

  wakakusa_work(el, key, proc { blk.call(PixieC.pixie_event_int) })
end

def wakakusa_on_float(el, key, &blk)
  return if blk.nil?

  wakakusa_work(el, key, proc { blk.call(PixieC.pixie_event_num) })
end

# The other way to give a handler: a proc, for the second handler on an
# element, where the block is already spoken for. It is called with
# nothing and asks for what the event carried itself — a proc handed
# through a keyword does not receive an argument in a compiled run.
def wakakusa_proc_none(el, key, cb)
  wakakusa_work(el, key, proc { cb.call })
end

def wakakusa_proc_text(el, key, cb)
  wakakusa_work(el, key, proc { cb.call })
end

def wakakusa_proc_bool(el, key, cb)
  wakakusa_work(el, key, proc { cb.call })
end

def wakakusa_proc_int(el, key, cb)
  wakakusa_work(el, key, proc { cb.call })
end

def wakakusa_proc_float(el, key, cb)
  wakakusa_work(el, key, proc { cb.call })
end

def wakakusa_rows(el, key, &blk)
  return if blk.nil?

  $wakakusa_rows.push(proc { blk.call($wakakusa_row_index) })
  PixieC.pixie_rows(el, key, $wakakusa_rows.length - 1)
end

# An element is finished. If a container is collecting, it joins that
# container's children; either way its handle is the answer.
def wakakusa_done(el)
  h = PixieC.pixie_end(el)
  frame = $wakakusa_frames.last
  frame.push(h) unless frame.nil?
  h
end

# The children of a container: the ones handed to it as arguments, then
# the ones its block wrote. An element handed over as an argument was
# already collected by whatever container is open, so it is taken back
# out — it belongs to this one now.
def wakakusa_collect(kids, &blk)
  frame = $wakakusa_frames.last
  unless frame.nil?
    kids.each { |h| frame.delete(h) }
  end
  return kids if blk.nil?

  $wakakusa_frames.push([])
  blk.call
  kids + $wakakusa_frames.pop
end

def wakakusa_children(el, kids)
  return if kids.empty?

  PixieC.pixie_children(el, wakakusa_ids(kids), kids.length)
end

def wakakusa_riders(el, riders, owns_label)
  riders.each do |name, v|
    if owns_label && name == :a11y_label
      raise ArgumentError, "this element's own `label` is already the name " \
                           "a screen reader reads; there is no second one to give"
    end
    wakakusa_rider(el, name, v)
  end
end

# Open the window and hand it the app: an object with a `view` method
# that answers one element. The app's state lives in its instance
# variables, and a handler is a block that closes over it.
#
# Under PIXIE_SCRIPT there is no window: the engine builds the tree,
# prints it, replays the script and returns.
def run(app, title: "wakakusa", width: 0.0, height: 0.0, padding: -1.0)
  return if $wakakusa_running

  $wakakusa_running = true
  $wakakusa_app = app
  wakakusa_start(title, width, height, padding)
end
