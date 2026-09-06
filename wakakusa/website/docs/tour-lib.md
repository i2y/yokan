# Ruby, data, and work

Everything so far has been the vocabulary Wakakusa adds. This page is
what it does not add: Ruby's own library, a database, and the two ways
work happens outside a handler.

## Ruby's own standard library

Ruby's own library is in both runs — `File`, `Dir`, `JSON`, `CSV`,
`Time`, `Math`, `Net::HTTP`, sockets, threads, `format`, the regular
expressions, everything `Enumerable` answers. There is no library of
ours in front of it, and nothing to learn twice.

```ruby
  require "json"

  def load
    @rows = JSON.parse(File.read(PATH))
  end
```

```ruby
  @spread = format("mean %.1f median %d", mean, median)
  @tally  = @votes.tally.sort_by { |name, n| [-n, name] }
  @steps  = @scores.each_cons(2).map { |a, b| b - a }
  @stamp  = Time.at(1_700_000_000).utc.strftime("%Y-%m-%d %H:%M:%S UTC")
```

You write against Ruby's library in both runs; underneath, one run is
CRuby's implementation of it and the other the compiler's, and what
the gate says is that they answer the same. That is the only claim
worth making about a library shared between two implementations —
[The two runs](two-runs.md) is where that arrangement is set out.
`demo/stdlib.rb`, `demo/files.rb` and `demo/reader.rb` are there to
hold them to it.

## A database

A database is the exception, because it is no use unless both runs read
the one file the same way. It reaches the same sqlite through the
engine.

```ruby
  sqlite_exec(DB, "CREATE TABLE IF NOT EXISTS notes(body TEXT)")
  sqlite_exec(DB, "INSERT INTO notes VALUES (?)", [@draft])
  rows = sqlite_rows(DB, "SELECT rowid, body FROM notes ORDER BY rowid")
  bodies = sqlite_column(DB, "SELECT body FROM notes")
```

Write `?` in the statement and put the values beside it: text a person
typed can never become part of the statement that way. An item called
`o'brien` is an apostrophe and never a piece of SQL — which is what
`demo/ledger.rb` types under its gate, every time.

`sqlite_rows` answers a list of rows, each a list of columns;
`sqlite_column` answers the first column of each row, flat. Every value
comes back as text, and the column's affinity converts on the way in.

## A timer

Work you want repeated goes to `every`.

```ruby
app = Clock.new
every(1.0) { app.tick }
run(app, title: "clock")
```

A timer is declared before `run` and lives as long as the app. Both
runs tick off one clock: a frame in a window, an `advance:<ms>` step in
a script, so the same number of ticks lands in both.
`demo/dashboard.rb` is a timer under a script.

## Work off the window's thread

A handler that blocks freezes the window. Hand the slow work to `task`,
and write what to do when it is done in `on_done`.

```ruby
  def start
    @status = "working"
    job = task do
      # deliberately slow, and deliberately arithmetic: both runs have
      # to agree about what it answers.
      total = 0
      i = 0
      while i < 300_000
        total += i % 7
        i += 1
      end
      total
    end
    # This block runs on the window's thread once the work has
    # finished; `task_answer` inside it is what the work answered.
    on_done(job) do
      @answer = task_answer
      @status = "done"
    end
  end
```

Neither call waits. `task` starts the work and answers a number,
`on_done` only says what to do later; the handler ends and the window
carries on.

Nothing inside the work may touch the app's state or the screen. That
is the whole rule, and it is why the answer comes back through
`on_done` rather than from the work itself.

A dialog is slow work of the same kind — it waits for a person — so it
goes through `task` too:

```ruby
  job = task { open_dialog("choose a file") }
  on_done(job) { @path = task_answer }
```

## An ordinary thread

Anything perpetual — a poller, a watcher, a server the app runs for
itself — is an ordinary `Thread`, and what it produces should be picked
up by a timer rather than written into the app's state from the worker.

The two runs differ here, and the shipped one is the better half. Two
seconds of arithmetic in each of two threads takes two seconds in the
compiled run: they are on two cores, and the window goes on drawing at
its usual rate. The same app under CRuby takes twice that, because the
interpreter runs one thread at a time, and the window stops until the
work is done. So a long computation can look worse while you are
writing the app than it will when you ship it.

How far such a thread gets is not something the two runs agree about —
one is on the clock the machine keeps and the other on the clock a
script sets — which is why `task` exists: the engine waits for the
answer, so both runs reach the same place before the next step.

## While you are writing it

`wakakusa run` watches the app's file. Save, and the window picks the
edit up: the file is read again, the class with it, and the object the
window is holding is an instance of that same class, so it answers with
the new `view` and keeps every value it had.

`initialize` is not run again on that object, which is the point — that
is where the state came from. A file that does not parse leaves the
window on what it had and says so in the terminal.

Reading the file runs the bottom of it too, so the object made there is
a second one and it is dropped: an `initialize` that opens a database
or starts a thread does so once per save. What was declared before
`run` keeps what it was given when the window opened — a timer goes on
ticking at the period it had, and one added while the window is open
takes effect the next time you start the app.

## Where next

- [Verify and ship](tour-ship.md) — the headless script vocabulary,
  the gate, and the one binary.
- [The two runs](two-runs.md) — what each run is made of, and the few
  places they are not the same.
