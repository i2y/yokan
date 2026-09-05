# Work that takes a while, done off the window's thread. `task` runs
# the block on a thread of its own; when it answers, the app's
# `on_done:` method is called on the window's thread with the answer.
#
# Nothing inside the work touches the app's state or the screen. That
# is the whole rule, and it is why the handler asks for the answer with
# `task_answer` rather than the worker writing it anywhere.
require "wakakusa"

class Jobs
  def initialize
    @status = "idle"
    @answer = 0
    @done = 0
  end

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
    on_done(job) do
      @answer = task_answer
      @done += 1
      @status = "done"
    end
  end

  def view
    column(spacing: 10.0, padding: 14.0) {
      text "background work", size: 18.0, bold: true
      text "status: #{@status}"
      text "answer: #{@answer}  (#{@done} finished)"
      button("start slow work") { start }
    }
  end
end

run(Jobs.new, title: "tasks")
