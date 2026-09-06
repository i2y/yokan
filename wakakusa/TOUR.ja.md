# 若草 言語ツアー

*若草を紹介する段落は所有者が書きます。
下書き: **若草は Ruby のデスクトップアプリを作るコンパイラです。
小さな要素の語彙に対して普通の Ruby を書きます。
書いているあいだは CRuby が動かし、リリースすると一つのネイティブバイナリになります。
その二つが同じプログラムであることを、ビルドのたびに検証します。***

このページには、言語そのものが読者の出会う順に並んでいます。
ここに書いたものはすべて実際に動きます。
`tools/tour_check.rb` がこのファイルから完全なアプリをすべて取り出し、デモと同じコマンドに通すからです。
語彙の名前を変えれば、読者がそれを目にする前にこのページが壊れます。
英語版は [TOUR.md](TOUR.md) です。

## 目次

- [いちばん小さいアプリ](#いちばん小さいアプリ)
- [状態の持ち方](#状態の持ち方)
- [ビューの書き方](#ビューの書き方)
- [ビューの中の制御構造](#ビューの中の制御構造)
- [入力の部品](#入力の部品)
- [ハンドラ](#ハンドラ)
- [一覧、グラフ、必要な行だけ作る一覧](#一覧グラフ必要な行だけ作る一覧)
- [キャンバス](#キャンバス)
- [キーボード](#キーボード)
- [すべての要素が取るキーワード](#すべての要素が取るキーワード)
- [テーマとアニメーション](#テーマとアニメーション)
- [ウィンドウまわり](#ウィンドウまわり)
- [Ruby 自身の標準ライブラリ](#ruby-自身の標準ライブラリ)
- [データベース](#データベース)
- [タイマー、ウィンドウの外のジョブ、スレッド](#タイマーウィンドウの外のジョブスレッド)
- [書いているあいだ](#書いているあいだ)
- [ウィンドウなしの実行とゲート](#ウィンドウなしの実行とゲート)
- [若草が断る書き方](#若草が断る書き方)
- [リリース](#リリース)
- [まだできないこと](#まだできないこと)

## いちばん小さいアプリ

アプリは `view` メソッドを持つオブジェクトです。
`view` は要素を一つ返し、そのオブジェクトを `run` に渡すとウィンドウが開きます。

<!-- script: dump -->
```ruby
require "wakakusa"

class Hello
  def view
    text "hello", size: 28.0
  end
end

run(Hello.new, title: "hello")
```

```console
$ wakakusa run demo/hello.rb      # CRuby でウィンドウが開く
$ wakakusa gate demo/hello.rb --script "dump"
GATE OK — 1 dump line identical in both runs
```

## 状態の持ち方

状態はオブジェクトのインスタンス変数です。
ハンドラはブロックで、ブロックの中からはそのオブジェクトがそのまま見えます。
だから書き換え方は、普通のメソッドと変わりません。
ハンドラを抜けると、そのときの状態からビュー全体が組み直されます。

<!-- script: click:+1,click:+1,dump,input:Momo,dump -->
```ruby
require "wakakusa"

class Counter
  def initialize
    @count = 0
    @name = ""
  end

  def view
    column(
      text("count: #{@count}", size: 34.0),
      row(
        button("+1") { @count += 1 },
        button("+10") { @count += 10 },
        button("reset") { @count = 0 },
        spacing: 8.0
      ),
      text_field(@name, placeholder: "your name") { |s| @name = s },
      text("hello, #{@name}"),
      spacing: 12.0,
      padding: 16.0
    )
  end
end

run(Counter.new, title: "counter")
```

ストアを別に用意することはありません。
監視の設定を書く必要も、これはフィールドだと印を付ける必要もありません。
アプリはオブジェクトであり、その状態は `initialize` から始まります。

## ビューの書き方

コンテナは、子を引数として取ることも、ブロックとして取ることもできます。
どちらも組み立てる木は同じなので、画面が読みやすくなるほうを選べば足ります。

<!-- script: click:+1,dump -->
```ruby
require "wakakusa"

class Two
  def initialize
    @count = 0
  end

  def view
    column(spacing: 12.0, padding: 16.0) {
      text "count: #{@count}", size: 34.0
      row(spacing: 8.0) {
        button("+1") { @count += 1 }
        button("reset") { @count = 0 }
      }
    }
  end
end

run(Two.new, title: "two")
```

ブロックの形はカンマを挟まずに入れ子になり、上から下へ読めます。
構造のある画面にはこの形が向き、三つほどを並べるだけの場面には引数の形が向きます。

要素を返すメソッドは画面の一部にあたり、それを呼ぶことでビューを分けて書けます。

```ruby
  def field(label, value)
    row(spacing: 6.0) {
      text label, width: 90.0
      text value, bold: true
    }
  end

  def view
    column(spacing: 4.0, padding: 14.0) {
      field("name", @name)
      field("size", @size)
    }
  end
```

## ビューの中の制御構造

ビューの中に書くブロックは、普通の Ruby です。
`if`、`unless`、三項演算子、繰り返し、メソッド呼び出し、ローカル変数、どれも使えます。
書いた要素は、書いた場所にそのまま木へ加わります。

<!-- script: dump,click:show,dump -->
```ruby
require "wakakusa"

class Control
  def initialize
    @open = false
    @rows = ["one", "two", "three"]
  end

  def line(s, i)
    row(spacing: 6.0) {
      text "#{i + 1}.", width: 22.0
      text s
    }
  end

  def view
    column(spacing: 8.0, padding: 14.0) {
      button(@open ? "hide" : "show") { @open = !@open }
      if @open
        @rows.each_with_index { |s, i| line(s, i) }
      else
        text "#{@rows.length} rows", color: "#8a8f98"
      end
      divider
      text "done"
    }
  end
end

run(Control.new, title: "control")
```

若草が断る形は一つだけで、繰り返しのブロックの中に要素のブロックを書くことはできません。
コンパイルされた実行では、そのブロックを実行する時点で繰り返しの変数がもう残っていないからです。
上の例で `line` をメソッドにしてあるのはそのためで、断るときの文面もこの書き換えを伝えます。

## 入力の部品

```ruby
  text_field(@name, placeholder: "name") { |s| @name = s }
  int_field(@qty, min: 0, max: 99) { |n| @qty = n }
  number_field(@rate, min: 0.0, max: 1.0, step: 0.05) { |v| @rate = v }
  checkbox("ready", checked: @ready) { |on| @ready = on }
  switch("dark", checked: @dark) { |on| @dark = on }
  slider(value: @vol, min: 0.0, max: 1.0) { |v| @vol = v }
  select(options: ["red", "green", "blue"], selected: @pick) { |i| @pick = i }
  radio_group(options: ["one", "two"], selected: @pick) { |i| @pick = i }
  segmented(options: ["day", "week"], selected: @span) { |i| @span = i }
  tab_bar(labels: ["files", "settings"], active: @tab) { |i| @tab = i }
```

どれも表示する値を引数に取り、変化した値はブロックが受け取ります。
知らないうちに結び付けられるものはありません。
`@name` と書いたからその欄には `@name` が表示され、値を書き戻すのはブロックです。

## ハンドラ

要素のハンドラは、普通はそのブロックです。
ハンドラを二つ持つ要素では、ブロックはすでに一つ目に使われています。
そこで二つ目は、引数を取らない proc をキーワード引数に渡して書きます。
イベントが運んできた値は、その proc の中で自分で取り出します。

```ruby
  text_field(@draft, on_submit: -> { add(event_text) }) { |s| @draft = s }
```

`event_text`、`event_number`、`event_index`、`event_on?` が、今届いているイベントの中身を返します。
キーワード引数で渡した proc は、コンパイルされた実行では引数を受け取りません。
値を自分で取り出すのは、そのためです。

## 一覧、グラフ、必要な行だけ作る一覧

十万行の一覧は、十万個の要素ではありません。
`list_view` と `table` が受け取るのは、行数と、`i` 番目の行を作るブロックです。
組み立てられるのは、画面に出ている行だけです。

```ruby
  list_view(@rows.length, item_height: 26.0, height: 280.0) { |i| line(i) }

  table(["name", "size"], @files.length, widths: [3.0, 1.0],
        item_height: 22.0, height: 300.0,
        on_select: -> { @chosen = event_index }) { |i| cells(i) }

  bar_chart(@totals, labels: ["food", "transit", "fun"], axis: true, height: 90.0)
  line_chart(@series, min: 0.0, max: 100.0, height: 120.0)
```

行のブロックは行番号を渡して呼ばれ、返したものがその行になります。
画面を描いている最中に呼ばれるので、ブロックの中では状態を読むだけにとどめ、書き換えません。

## キャンバス

`canvas` は仮想的な画素の格子で、ブロックの中に書いた命令がそこに絵を描きます。
キャンバスの中では、色を**番号**で指定します。
その番号は、`canvas` に渡した palette の何番目かという意味です。
ドット絵の道具はどれもこの作りなので、そうした環境のために書かれた描画コードは、数字を書き換えずにそのまま移せます。

<!-- script: advance:50,dump -->
```ruby
require "wakakusa"

PALETTE = ["#11111b", "#89b4fa", "#f38ba8", "#eeeeee", "#a6e3a1"].freeze

class Sky
  def initialize
    @frame = 0
  end

  def tick
    @frame += 1
  end

  def view
    canvas(64, 40, scale: 6, background: 0, palette: PALETTE) {
      rect(2, 2, 12, 6, 1)
      rect_outline(16, 2, 12, 6, 2)
      circle_outline(34, 5, 4, 3)
      line(2, 11, 61, 11, 2)
      triangle(3, 37, 8, 28, 13, 37, 4)
      circle(30, 18, 3, 3)
      pixel_text(2, 14, "FRAME #{@frame}", 3)
    }
  end
end

app = Sky.new
every(0.05) { app.tick }
run(app, title: "sky")
```

使える命令は `pixel`、`line`、`rect`、`rect_outline`、`circle`、`circle_outline`、`triangle`、`triangle_outline`、`sprite`、`pixel_text` です。
これらは要素ではないので、押すことも、テーマを変えることも、大きさを指定することも、アニメーションをつけることもできません。
キャンバスの中に書いた繰り返しは、ただの繰り返しです。

`sprite` は、キャンバスと palette を共有する画像から矩形を切り出して写します。
`colkey` に番号を渡すと、その色だけは写りません。

```ruby
  sprite(@px, @py, SHEET, 0, 0, 16, 16, colkey: SKY)
```

## キーボード

ゲームでは、キーが押されたと知らせてもらうのを待たずに、今どのキーが押されているかをアプリの側から尋ねます。

```ruby
  def tick
    @x -= 2 if key_down("left")
    @x += 2 if key_down("right")
    fire if key_pressed("space")
    quit if key_pressed("q")
  end
```

`key_down` は今押されているかどうかを、`key_pressed` は前のティック以降に押されたかどうかを答えます（押しっぱなしでも、答えるのは一度だけです）。
`key_released` はその逆で、離されたかどうかを答えます。
これらを読むのはタイマーの中だけで、ビューの中では読みません。
ビューでキーボードを読むと、ウィンドウとスクリプトで違う絵を描くことになります。

`demo/jump.rb` と `demo/shooter.rb` は、Pyxel 自身の例（Takashi Kitao、MIT）をこの語彙に移植して、ゲートに通したものです。

キャンバスは、ウィンドウを開かずに見られます。
`WAKAKUSA_FRAMES=<dir>` を与えると、スクリプトの一手ごとに、最初のキャンバスを PNG に書き出します。
絵を描くのは、ウィンドウが使うのと同じ raster です。
`WAKAKUSA_FRAME_SCALE` を与えると、アプリを書き換えずに格子を大きく描けます。
dump がその一コマの中身を示すのに対して、こちらはそれがどう見えるかを残します。
ssh 越しでも、CI の中でも、画面がロックされていても書き出せます。

```console
$ WAKAKUSA_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

## すべての要素が取るキーワード

どの要素も、次の十五の性質を同じ名前と同じ意味で取ります。
`width`、`height`、`min_width`、`max_width`、`disabled`、`theme`、`animate`、`easing`、`enter`、`exit`、`col_span`、`row_span`、`role`、`a11y_label`、`tooltip` です。

```ruby
  button("save", disabled: @busy, tooltip: "write the file", width: 120.0)
  text "total", role: "heading", a11y_label: "the running total"
```

同じ名前をもともと自分の意味で持っている要素では、要素側の意味がそのまま通ります。
`text` の `width` は文字の幅を指し、外側のコンテナが上書きすることはありません。

## テーマとアニメーション

配色と動きも、キーワードで指定します。

```ruby
  column(theme: "dark") { ... }
  text "saved", animate: 0.2, easing: "ease-out", enter: true
```

`theme:` は、書いた要素から下だけ配色を差し替えます。
`animate:` には変化にかける時間を渡し、現れるときと消えるときの動きは `enter:` と `exit:` で指定します。

## ウィンドウまわり

ウィンドウから受け取るものは、次の四つです。

```ruby
shortcut("cmd+s") { app.save }
menu_item("File", "Open…") { app.open }
on_key { |chord| app.typed(chord) }
on_file_drop { |path| app.load(path) }
```

これらは `run` の前に書きます。
一度書けば、アプリが動いているあいだずっと有効です。

```ruby
  clipboard_set(@text)
  @text = clipboard_get

  job = task { open_dialog("choose a file") }
  on_done(job) { @path = task_answer }
```

ダイアログは人の操作を待つので、`task` の中に置きます。
ウィンドウなしで走らせるときは、スクリプトの `file:<path>` がその答えになります。
だからダイアログも、ほかの操作と同じように検査できます。

## Ruby 自身の標準ライブラリ

Ruby 自身のライブラリは、どちらの実行でも使えます。
`File`、`Dir`、`JSON`、`CSV`、`Time`、`Math`、`Net::HTTP`、ソケット、スレッド、それに `Enumerable` が答えることまで、どれもそのまま動きます。
これを若草のライブラリで置き換えてはいないので、覚え直すものもありません。

```ruby
  require "json"

  def load
    @rows = JSON.parse(File.read(PATH))
  end
```

`demo/stdlib.rb`、`demo/files.rb`、`demo/reader.rb` は、どちらの実行もそのとおりに動くことを確かめるために置いてあります。

## データベース

データベースだけは例外です。
どちらの実行も同じファイルを同じように読まなければ、意味がないからです。
どちらの実行も、エンジンを通して同じ sqlite に届きます。

```ruby
  sqlite_exec(DB, "CREATE TABLE IF NOT EXISTS notes(body TEXT)")
  sqlite_exec(DB, "INSERT INTO notes VALUES (?)", [@draft])
  rows = sqlite_rows(DB, "SELECT rowid, body FROM notes ORDER BY rowid")
  bodies = sqlite_column(DB, "SELECT body FROM notes")
```

文には `?` を書き、値は別の引数として渡します。
こう書けば、人が打った文字が文の一部になることはありません。
読み出した値はすべて文字列で、書き込むときは列の型に従って変換されます。

## タイマー、ウィンドウの外のジョブ、スレッド

くり返したい処理は `every` に渡します。

```ruby
app = Clock.new
every(1.0) { app.tick }
run(app, title: "clock")
```

タイマーも `run` の前に書きます。
一度書いたタイマーは、アプリが終わるまで刻み続けます。
どちらの実行も、一つの時計で刻みます。
その時計を進めるのは、ウィンドウではフレーム、スクリプトでは `advance:` です。

ハンドラの中で待ってはいけません。
待っているあいだ、ウィンドウは固まります。
時間のかかる処理は `task` に渡し、終わったあとにすることを `on_done` に書きます。

```ruby
  def start
    job = task { something_slow }
    # このブロックが、ジョブが終わったあとにウィンドウの側で走る。
    # 中の `task_answer` が、ジョブの答え。
    on_done(job) { @answer = task_answer }
  end
```

どちらの呼び出しも、ジョブが終わるのを待ちません。
`task` はジョブを始めて番号を返し、`on_done` はあとで何をするかを登録するだけです。
ハンドラはそこで終わり、ウィンドウは動き続けます。
ジョブの中からアプリの状態や画面に触ってはいけません。
それはハンドラの役目です。
ジョブの答えが `on_done` に渡される形になっているのも、そのためです。

動かし続けたい処理は、普通の `Thread` にします。
その結果は、そのスレッドからアプリの状態に書き込むのではなく、タイマーの側で拾います。
コンパイルされた実行では、そのスレッドが別のコアで走るので、ウィンドウは描き続けます。
CRuby ではスレッドが一度に一本しか走りません。
そのため重い計算は、書いているあいだはウィンドウを止めますが、リリースしたバイナリでは止めません。

## 書いているあいだ

`wakakusa run` はアプリのファイルを監視しています。
保存すると、その変更がウィンドウに反映されます。
ファイルが読み直され、クラスも定義し直されますが、ウィンドウが持っているオブジェクトはそのクラスのインスタンスのままです。
だから、新しい `view` を返しながら、それまでの値をすべて保ちます。
そのオブジェクトの `initialize` は走り直しません。
状態が生まれるのはそこなので、走り直さないことに意味があります。
構文が通らないファイルを保存しても、ウィンドウは直前の表示のままです。
端末には、構文が通らなかったことが表示されます。

ファイルを読み直すので、その一番下も走ります。
そこで作られるオブジェクトは二つめで、すぐ捨てられます。
`initialize` がデータベースを開いたりスレッドを立てたりしているなら、それは保存のたびに起きます。
`run` より前に宣言したものは、ウィンドウが開いたときのまま残ります。
タイマーは渡された周期のまま動き続けます。
ウィンドウを開いたまま足したタイマーは、次に起動したときから効きます。

## ウィンドウなしの実行とゲート

`PIXIE_SCRIPT` が人の操作の代わりになります。
エンジンが木を組み、そこに書かれた手順のとおりに動かして、dump を印字します。

```
click:<label>      表示されている文字でボタンを押す
input:<text>       欄に打つ            submit    その欄で改行する
slide / select     つまみを動かす、選択肢を選ぶ
key:<chord>        shortcut に結ばれた打鍵
keydown:<key> / keyup:<key>    キーを押したままにする、離す
menu:<item>        メニュー項目を選ぶ  file:<path>   ダイアログの答え
drop:<path>        ウィンドウにファイルを落とす
advance:<ms>       時計を進める        theme:dark|light
dump               木を印字            a11y   読み上げが読むものを印字
```

`wakakusa gate` は、一つのスクリプトでアプリを二回走らせます。
片方は CRuby が C ABI 越しに走らせるもので、もう片方は同じ ABI をリンクしたコンパイル済みのバイナリです。
最後に、二つの記録を一バイトずつ比べます。

```console
$ wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

ゲートが、若草の約束そのものです。
このページのほかの部分では、そのゲートを通るものをどう書くかを説明しています。

## 若草が断る書き方

`wakakusa check` はアプリを読み、受け取れない書き方があれば、その行番号と書き換え方を示します。
ビルドの前にもゲートの前にも、毎回走ります。

- ブロックの中から書き換えるグローバル変数に、状態を置くこと。
  アプリのオブジェクトに持たせます。
- フィールドに持たせた配列に、`list + [item]` で要素を足すこと。
  `dup` してから `push` と書きます。
  `list + [item]` が何を返すかは、二つの実行で一致しないからです。
- リテラルのブロックか、引数を取らない proc をキーワード引数で渡すか。
  ハンドラの渡し方はこの二つだけで、ほかの書き方は断ります。
- 繰り返しのブロックの中に、要素のブロックを書くこと。
  必要なものを引数に取るメソッドへ移します。

どれも、印字される文面ごと `test/refuse/` に置いてあります。
だから、断りの文面が黙って変わることはありません。

## リリース

```console
$ wakakusa build demo/todo.rb --release --app
built:  demo/.gate/todo (11.5 MB)
bundle: demo/dist/todo.app (11.7 MB)
```

`--release` は symbol table を落とします。
`--app` はバイナリを macOS のアプリケーションバンドルに包み、ad-hoc 署名をつけます。
アプリと同じ場所に `<stem>.png` か `<stem>.icns` を置いておくと、それがアイコンになります。
バイナリはエンジンとコンパイル済みの Ruby を含んでいて、システム自身のライブラリ以外は何もリンクしません。
だから、このバンドルだけでプログラムが完結します。
Ruby もコンパイラも入っていないマシンで、そのまま開きます。

## まだできないこと

- 音が出ません。
  エンジンに音を鳴らす仕組みがないので、移植した二つのゲームは、原作と違って無音のままです。
- 種を与えた `Random` は、二つの実行で同じ生成器になりません。
  どちらの実行でも同じ数列がほしいなら、生成器を自分で書きます。
  二つのゲームがそうしていて、算術だけの六行で足ります。
- 決まった形で書かなければならないところが三つあります。
  ほかの書き方をコンパイラがまだ受け取れないためで、どれも[若草が断る書き方](#若草が断る書き方)に挙げてあります。
- `check` が見つけるのは、その三つとほかの五つです。
  コンパイラが取りこぼすものをすべて見ているわけではないので、残りを見つけるのは今もゲートです。
- macOS だけです。
  バイナリが描画に使うエンジンを含むので、小さなアプリでも 11 MB ほどになります。
