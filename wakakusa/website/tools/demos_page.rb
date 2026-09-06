#!/usr/bin/env ruby
# frozen_string_literal: true
#
# The site's Demos page, in both languages, written from demo/.
#
# A gallery is a page of usage samples, so the sample has to be on it:
# each demo gets its screenshot and, under it, its whole source in a
# collapsed block. The gloss for each one is written here, once per
# language, and a demo with no entry stops this program — so a new demo
# cannot quietly go missing from the gallery.
#
#   tools/demos_page.rb            write docs/demos.md and docs-ja/
#   tools/demos_page.rb --check    fail if either is behind demo/

require "fileutils"

SITE = File.expand_path("..", __dir__)
ROOT = File.expand_path("..", SITE)
DEMOS = File.join(ROOT, "demo")
SHOTS = File.join(DEMOS, "screenshots")

# Files under demo/ that are not apps.
NOT_APPS = %w[calc_style].freeze

GROUPS = %w[start state look lists canvas ruby window work].freeze

# group, name, the English gloss, the Japanese one.
CATALOGUE = [
  ["start", "counter",
   "the reference: an app is an object, its state is its instance variables, and a handler is a block that closes over it",
   "基本形。アプリはオブジェクトで、状態はそのインスタンス変数、ハンドラはそのオブジェクトが見えるブロック"],
  ["start", "blockform",
   "the same counter written the other way: a container takes its children as a block, and both spellings build the same tree",
   "同じカウンタを別の書き方で。コンテナは子をブロックとして取り、どちらの書き方も同じ木になる"],
  ["start", "control",
   "ordinary Ruby inside a view: `if`, `unless`, a ternary, a loop, and a method that answers part of the screen",
   "ビューの中はただの Ruby。`if`、`unless`、三項演算子、ループ、画面の一部を返すメソッド"],
  ["start", "todo",
   "a list whose rows are built on demand, and a field that submits with enter",
   "行を必要なぶんだけ作るリストと、enter で確定する入力欄"],
  ["start", "calc",
   "a calculator: one accumulator, one pending operation, and a look kept in a Hash and splatted onto each key",
   "電卓。途中の値ひとつと待っている演算ひとつを持ち、見た目は Hash にまとめて各キーに `**` で渡す"],
  ["start", "calcgrid",
   "the same calculator on a grid instead of five rows; `col_span:` is what makes the zero key twice as wide",
   "同じ電卓を 5 行ではなくグリッドで。0 キーが 2 つぶんの幅になるのは `col_span:`"],

  ["state", "mixer",
   "state an app keeps, and a field that writes into it",
   "アプリが持つ状態と、そこへ書き込む入力欄"],
  ["state", "lookup",
   "a Hash on the app: reading with a fallback, asking whether a key is there, and adding one while the window is open",
   "アプリが持つ Hash。既定値つきの読み出し、キーの有無の確認、ウィンドウを開けたまま増やす"],
  ["state", "points",
   "a small class of values, carried on the app's own state",
   "値のための小さなクラスを、アプリの状態として持つ"],
  ["state", "links",
   "objects that point at one another, written the way Ruby's collector allows",
   "互いを指すオブジェクト。Ruby の GC は循環を扱えるので、逆向きの参照も普通の参照でよい"],
  ["state", "moods",
   "values that are one of a few named things (symbols), and a value that may be nothing at all (nil), told apart by `case`",
   "いくつかの名前のどれかである値（シンボル）と、何もないかもしれない値（nil）を `case` で見分ける"],

  ["look", "forms",
   "the controls a person changes: a box, a switch, a track, and the four choosers",
   "人が動かす部品。チェックボックス、スイッチ、スライダ、そして 4 種類の選択"],
  ["look", "quantities",
   "the two fields that hold a number rather than text: enter commits, text that is not a number is dropped",
   "文字ではなく数を持つ 2 つの入力欄。enter で確定し、数でない文字は捨てられる"],
  ["look", "layout",
   "spacer and divider: a filler that pushes what follows to the edge, and a rule",
   "spacer と divider。続きを端まで押しやる余白と、区切り線"],
  ["look", "cards",
   "a piece of screen with a name is a method, and one that wraps other elements takes them as arguments",
   "名前のついた画面の一部はメソッド。他の要素を包むものは、それらを引数に取る"],
  ["look", "styled",
   "a look kept in one place: a Hash merged and splatted, and `theme:` flipping a whole panel",
   "見た目をひとところに。Hash を merge して `**` で渡し、`theme:` でパネルごと切り替える"],
  ["look", "badges",
   "text as a pill, and the rest of what a run of text can be: monospace, underlined, italic, clipped, clamped",
   "文字を丸いラベルに。等幅、下線、斜体、省略記号での打ち切り、行数の制限"],
  ["look", "panels",
   "the elements that arrange or cover: tracks, layers, panes that scroll, and a panel over the rest of the window",
   "並べる要素と覆う要素。グリッド、重ね、スクロールする面、ウィンドウの上に出る一枚"],
  ["look", "dialog",
   "a panel over the rest of the window, opened and closed by the app",
   "ウィンドウの上に出る一枚を、アプリが開いて閉じる"],
  ["look", "labels",
   "what a screen reader is told and what the pointer shows; `role:` takes a value, so a line is a heading until it is not",
   "画面読み上げに伝える名前と、ポインタが見せる説明。`role:` は値を取るので、見出しであることを途中でやめられる"],
  ["look", "shared",
   "the properties every element takes, on elements that have nothing else in common",
   "共通のプロパティを、種類の違う要素それぞれに付けてみる"],
  ["look", "loading",
   "the bar that fills, in its three forms, and the sweep for work with no known length",
   "満ちていくバーの 3 つの形と、終わりの見えないジョブのための行き来する表示"],
  ["look", "filter",
   "a chooser that changes what a list shows, with the rows built on demand",
   "リストの見せかたを変える選択。行は必要なぶんだけ作られる"],

  ["lists", "table",
   "`data_table` draws the table itself: the first row is the header, the later ones are shaded in alternation",
   "`data_table` が表そのものを描く。最初の row が見出しで、以降は交互に色の変わるデータ行"],
  ["lists", "roster",
   "the table that builds its rows on demand, with row selection and header sort the app performs itself",
   "行を必要なぶんだけ作る表。行の選択と見出しでの並べ替えは、アプリ自身が行う"],
  ["lists", "csv_viewer",
   "a hundred thousand rows, filtered as you type; only the rows in the window are ever built",
   "10 万行を、打ちながら絞り込む。作られるのは画面に入っている行だけ"],
  ["lists", "trend",
   "one list of numbers, drawn twice",
   "ひとつの数のリストを、2 通りに描く"],
  ["lists", "charts",
   "losses below the zero line, a pinned range, an axis with gridlines, and two series with their own colors",
   "0 の線より下に垂れる損失、固定した範囲、目盛りと補助線のある軸、色を持つ 2 本の系列"],

  ["canvas", "canvas",
   "a grid of virtual pixels painted command by command, colors by palette index, and the keyboard read from the tick",
   "仮想ピクセルの格子を、命令をひとつずつ並べて塗る。色はパレットの番号、キーボードはタイマーから読む"],
  ["canvas", "jump",
   "Pyxel's jump game, ported: gravity, floors that fall away when you land on them, fruit, and scenery scrolling at its own speed",
   "Pyxel のジャンプゲームの移植。重力、乗ると落ちる床、果物、それぞれの速さで流れる背景"],
  ["canvas", "shooter",
   "Pyxel's shoot-'em-up, ported: scenes, parallax stars, enemies that sway as they fall, collisions and expanding blasts",
   "Pyxel のシューティングの移植。場面の切り替え、視差のある星、揺れながら落ちてくる敵、当たり判定と広がる爆発"],

  ["ruby", "stdlib",
   "Ruby's own standard library under the gate: `Math`, `Time`, `JSON`, `CSV`, `format`, regular expressions, `Enumerable`",
   "Ruby 自身の標準ライブラリをゲートにかける。`Math`、`Time`、`JSON`、`CSV`、`format`、正規表現、`Enumerable`"],
  ["ruby", "files",
   "files with Ruby's own `File` and `Dir`: nothing here is Wakakusa's",
   "Ruby 自身の `File` と `Dir` でファイルを扱う。ここに Wakakusa のものは何もない"],
  ["ruby", "reader",
   "a page fetched off the window's thread, from a server the app runs for itself, so both runs read the same bytes",
   "ウィンドウのスレッドの外で取ってくるページ。相手はアプリが自分で立てたサーバなので、両方の実行が同じバイト列を読む"],
  ["ruby", "dbnotes",
   "a database reached through the engine, with the values bound rather than spliced",
   "エンジン越しに触るデータベース。値は文に埋め込まず、束縛して渡す"],
  ["ruby", "ledger",
   "money kept in sqlite: an item called o'brien is an apostrophe and never a piece of SQL",
   "sqlite に置いた家計簿。o'brien という品目はアポストロフィであって、SQL の一部にはならない"],
  ["ruby", "edges",
   "the edges: an index past the end of a list, and a number far past what a machine word holds",
   "端の話。リストの終わりを越えた添字と、64 ビットをはるかに越えた数"],
  ["ruby", "flow",
   "control flow in the handlers: a loop that skips, a loop that stops, a while, and a method that wraps another",
   "ハンドラの中の制御フロー。飛ばすループ、止まるループ、while、別のメソッドを包むメソッド"],

  ["window", "keys",
   "the keyboard as chords and the same handlers in the menu bar, driven with `key:cmd+s` and `menu:Save`",
   "キーの組み合わせにハンドラを結び付け、同じものをメニューバーにも置く。`key:cmd+s` と `menu:Save` で動かせる"],
  ["window", "picker",
   "the platform's own file panels, asked for off the window's thread, and a file dragged onto the window",
   "OS 自身のファイル選択と、ウィンドウへ落とされたファイル。選択は人を待つので、ウィンドウのスレッドの外で頼む"],
  ["window", "about",
   "links that open a page, and the system clipboard",
   "ページを開くリンクと、システムのクリップボード"],

  ["work", "dashboard",
   "a timer declared before the app runs, ticking in both runs (the gate steps it with `advance:`)",
   "アプリを走らせる前に宣言するタイマー。両方の実行で同じだけ時を刻む（ゲートは `advance:` で進める）"],
  ["work", "tasks",
   "work that takes a while, done off the window's thread; the answer comes back through `on_done`",
   "時間のかかるジョブをウィンドウのスレッドの外へ。答えは `on_done` で受け取る"],
].freeze

WORDS = {
  en: {
    title: "Demos",
    intro: <<~MD,
      Forty-three apps, every one of them gated: the interpreted run and
      the compiled one, driven by the same script, compared byte for
      byte. Each runs as-is from `wakakusa/` in the repository.

      ```console
      $ ./bin/wakakusa run demo/counter.rb     # substitute any demo's name
      $ ./tools/gate_all.sh                    # gate every demo at once
      ```

      Every screenshot shows the state right after launch, except the two
      games, which show a recording of play. The source under each one is
      the whole file.
    MD
    groups: {
      "start" => "Start here",
      "state" => "Holding state",
      "look" => "Elements, layout and look",
      "lists" => "Lists, tables and charts",
      "canvas" => "The canvas, and the two games",
      "ruby" => "Ruby, files and data",
      "window" => "The window",
      "work" => "Time, and work off the thread",
    },
    source: "source",
  },
  ja: {
    title: "デモ",
    intro: <<~MD,
      43 本のアプリが並んでいます。
      どれもゲートを通っています。
      つまり、インタプリタで動かした実行と、コンパイルした実行を、同じスクリプトで動かして、1 バイトずつ突き合わせてあります。
      どれもリポジトリの `wakakusa/` からそのまま動きます。

      ```console
      $ ./bin/wakakusa run demo/counter.rb     # デモの名前はどれでも
      $ ./tools/gate_all.sh                    # 全部まとめてゲートにかける
      ```

      スクリーンショットは、どれも起動直後の画面です。
      ゲーム 2 本だけは、遊んでいるところを録画したものを載せています。
      それぞれの下にあるのは、そのファイルの全文です。
    MD
    groups: {
      "start" => "まずここから",
      "state" => "状態を持つ",
      "look" => "要素、配置、見た目",
      "lists" => "リストと表とグラフ",
      "canvas" => "キャンバスと、2 本のゲーム",
      "ruby" => "Ruby、ファイル、データ",
      "window" => "ウィンドウ",
      "work" => "時間と、ウィンドウの外のジョブ",
    },
    source: "ソース",
  },
}.freeze

# The two games are recordings, at the size they were recorded.
WIDTHS = { "jump" => 320, "shooter" => 240 }.freeze

def shot(name)
  %w[png gif].each do |ext|
    return "#{name}.#{ext}" if File.file?(File.join(SHOTS, "#{name}.#{ext}"))
  end
  abort "demos_page: demo/screenshots has no shot for `#{name}`"
end

def source(name)
  path = File.join(DEMOS, "#{name}.rb")
  abort "demos_page: no demo/#{name}.rb" unless File.file?(path)
  File.read(path).rstrip
end

def page(lang)
  w = WORDS.fetch(lang)
  out = ["# #{w.fetch(:title)}", "", w.fetch(:intro).strip, ""]
  GROUPS.each do |group|
    out << "## #{w.fetch(:groups).fetch(group)}"
    out << ""
    CATALOGUE.select { |g,| g == group }.each do |_, name, en, ja|
      gloss = lang == :en ? en : ja
      out << "#### #{name} — #{gloss}"
      out << %(<img src="images/demos/#{shot(name)}" width="#{WIDTHS.fetch(name, 360)}">)
      out << ""
      out << %(??? note "#{name}.rb")
      out << ""
      out << "    ```ruby"
      source(name).each_line { |l| out << (l.strip.empty? ? "" : "    #{l.chomp}") }
      out << "    ```"
      out << ""
    end
  end
  out.join("\n").gsub(/\n{3,}/, "\n\n") + "\n"
end

# Every app under demo/ has to be in the catalogue, and every entry has
# to be an app: a gallery that quietly drops a demo is worse than none.
apps = Dir[File.join(DEMOS, "*.rb")].map { |f| File.basename(f, ".rb") }
             .reject { |n| n.start_with?(".") } - NOT_APPS
listed = CATALOGUE.map { |_, name,| name }
missing = apps.sort - listed.sort
extra = listed.sort - apps.sort
abort "demos_page: not in the catalogue: #{missing.join(", ")}" unless missing.empty?
abort "demos_page: in the catalogue and not in demo/: #{extra.join(", ")}" unless extra.empty?

FILES = { en: File.join(SITE, "docs", "demos.md"),
          ja: File.join(SITE, "docs-ja", "demos.md") }.freeze

check = ARGV.include?("--check")
stale = []
FILES.each do |lang, path|
  text = page(lang)
  if check
    stale << path unless File.exist?(path) && File.read(path) == text
  else
    FileUtils.mkdir_p(File.dirname(path))
    File.write(path, text)
    puts "wrote #{path.delete_prefix("#{ROOT}/")} (#{listed.length} demos)"
  end
end

if check
  unless stale.empty?
    warn "demos_page: behind demo/: #{stale.map { |p| p.delete_prefix("#{ROOT}/") }.join(", ")}"
    exit 1
  end
  puts "demos page: up to date"
end
