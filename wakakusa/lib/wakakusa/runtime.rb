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
$wakakusa_row_index = 0
$wakakusa_app = nil
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

# The other way to name a handler: a symbol, which is the name of one of
# the app's own methods. A block cannot be handed through a keyword —
# a compiled run receives the address rather than the block, silently —
# so a second handler on the same element is written this way instead.
def wakakusa_send_none(el, key, name)
  wakakusa_work(el, key, proc { $wakakusa_app.send(name) })
end

def wakakusa_send_text(el, key, name)
  wakakusa_work(el, key, proc { $wakakusa_app.send(name, wakakusa_event_text) })
end

def wakakusa_send_bool(el, key, name)
  wakakusa_work(el, key, proc { $wakakusa_app.send(name, PixieC.pixie_event_int != 0) })
end

def wakakusa_send_int(el, key, name)
  wakakusa_work(el, key, proc { $wakakusa_app.send(name, PixieC.pixie_event_int) })
end

def wakakusa_send_float(el, key, name)
  wakakusa_work(el, key, proc { $wakakusa_app.send(name, PixieC.pixie_event_num) })
end

def wakakusa_rows(el, key, &blk)
  return if blk.nil?

  $wakakusa_rows.push(proc { blk.call($wakakusa_row_index) })
  PixieC.pixie_rows(el, key, $wakakusa_rows.length - 1)
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
  $wakakusa_app = app
  wakakusa_start(title, width, height, padding)
end
