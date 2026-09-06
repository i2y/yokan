# 若草 言語ツアー

*若草を紹介する段落は所有者が書きます。
下書き: **若草は Ruby のデスクトップアプリを作るコンパイラです。
小さな要素の語彙に対して普通の Ruby を書きます。
書いているあいだは CRuby が動かし、出荷すると一つのネイティブバイナリになります。
その二つが同じプログラムであることを、ビルドのたびに検証します。***

この頁は、出会う順に並べた言語そのものです。
書いてあることはすべて動きます。
`tools/tour_check.rb` がこのファイルから完全なアプリをすべて取り出し、デモと同じコマンドに通すので、語彙の名前が変われば読者より先にこの頁が壊れます。

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
- [すべての要素が取る keyword](#すべての要素が取る-keyword)
- [テーマとアニメーション](#テーマとアニメーション)
- [窓まわり](#窓まわり)
- [Ruby 自身の標準ライブラリ](#ruby-自身の標準ライブラリ)
- [データベース](#データベース)
- [タイマー、窓の外の仕事、スレッド](#タイマー窓の外の仕事スレッド)
- [書いているあいだ](#書いているあいだ)
- [窓なしの実行と門](#窓なしの実行と門)
- [若草が断る書き方](#若草が断る書き方)
- [出荷](#出荷)
- [まだできないこと](#まだできないこと)

## いちばん小さいアプリ

アプリは `view` メソッドを持つオブジェクトです。
`view` は要素を一つ返し、`run` がそれに窓を開けます。

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
$ wakakusa run demo/hello.rb      # CRuby で窓が開く
$ wakakusa gate demo/hello.rb --script "dump"
GATE OK — 1 dump line identical in both runs
```

## 状態の持ち方

状態はオブジェクトのインスタンス変数です。
ハンドラはブロックで、ブロックはオブジェクトを閉じ込めるので、普通のメソッドと同じように書き換えられます。
ハンドラが終わると、今の状態からビュー全体が組み直されます。

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

別に用意する入れ物はありません。
監視を宣言する必要も、フィールドだと印をつける必要もありません。
アプリはオブジェクトで、状態は `initialize` から始まります。

## ビューの書き方

入れ物は子を引数として取ることも、ブロックとして取ることもできます。
どちらも同じ木を組み立てるので、選ぶ基準は画面の読みやすさだけです。

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

ブロックの形は読点を挟まずに入れ子になり、上から下へ読めます。
構造のある画面に向きます。
引数の形は、三つ並べるような場面に向きます。

要素を返すメソッドは画面の一部です。
それを呼ぶことが、ビューを分ける方法になります。

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

ビューの中のブロックは普通の Ruby です。
`if`、`unless`、三項演算子、繰り返し、メソッド呼び出し、ローカル変数、どれも使えます。
書いたものは、その場所で木に加わります。

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

断られる形は一つだけです。
繰り返しのブロックの中に、要素のブロックを書くことはできません。
コンパイルされた実行は、そこに来る頃には繰り返しの変数を失っています。
上の `line` がメソッドになっているのはそのためで、これが拒絶の文面が示す書き換えです。

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

どれも表示する値を受け取り、ブロックが変化を受け取ります。
裏で結び付けられるものはありません。
`@name` と書いたからその欄は `@name` を表示し、書き戻すのはブロックです。

## ハンドラ

要素のハンドラは、普通はそのブロックです。
二つ目のハンドラを持つ要素では、ブロックはすでに一つ目に使われています。
そこで二つ目は keyword として、引数を取らない proc で書きます。
proc は、イベントが運んできたものを自分で尋ねます。

```ruby
  text_field(@draft, on_submit: -> { add(event_text) }) { |s| @draft = s }
```

`event_text`、`event_number`、`event_index`、`event_on?` が、今届いているイベントの中身を答えます。
keyword で渡された proc は、コンパイルされた実行では引数を受け取りません。
だから自分で尋ねます。

## 一覧、グラフ、必要な行だけ作る一覧

十万行の一覧は、十万個の要素ではありません。
`list_view` と `table` は行数と、`i` 番目の行を作るブロックを取ります。
画面に出ている行だけが組み立てられます。

```ruby
  list_view(@rows.length, item_height: 26.0, height: 280.0) { |i| line(i) }

  table(["name", "size"], @files.length, widths: [3.0, 1.0],
        item_height: 22.0, height: 300.0,
        on_select: -> { @chosen = event_index }) { |i| cells(i) }

  bar_chart(@totals, labels: ["food", "transit", "fun"], axis: true, height: 90.0)
  line_chart(@series, min: 0.0, max: 100.0, height: 120.0)
```

行のブロックは行番号で呼ばれ、返したものがその行になります。
画面を描いている最中に呼ばれるので、状態を読むだけにして、書き換えません。

## キャンバス

`canvas` は仮想的な画素の格子で、ブロックの中の命令が絵を描きます。
中では色は**番号**です。
アプリが宣言した palette の何番目か、という意味になります。
ドット絵の道具はそう作られているので、そういう機械のために書かれた描画コードが、数字をそのままにして移せます。

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

命令は `pixel`、`line`、`rect`、`rect_outline`、`circle`、`circle_outline`、`triangle`、`triangle_outline`、`sprite`、`pixel_text` です。
これらは要素ではありません。
押すことも、テーマを変えることも、大きさを与えることも、動かすこともできません。
キャンバスの中の繰り返しは、ただの繰り返しです。

`sprite` は、キャンバスと palette を共有する画像から矩形を切って写します。
`colkey` は写さない色の番号です。

```ruby
  sprite(@px, @py, SHEET, 0, 0, 16, 16, colkey: SKY)
```

## キーボード

ゲームは、教えてもらうのを待つのではなく、今の手の状態を尋ねます。

```ruby
  def tick
    @x -= 2 if key_down("left")
    @x += 2 if key_down("right")
    fire if key_pressed("space")
    quit if key_pressed("q")
  end
```

`key_down` は「今押されている」、`key_pressed` は「前の frame から今のあいだに押された」（押しっぱなしでも一度だけ答えます）、`key_released` はその逆です。
読むのはタイマーの中だけにします。
ビューで読むと、窓では一つの絵を、スクリプトでは別の絵を描くことになります。

`demo/jump.rb` と `demo/shooter.rb` は Pyxel 自身の例（Takashi Kitao、MIT）をこの語彙に移し、門に通したものです。

キャンバスは窓なしで見られます。
`WAKAKUSA_FRAMES=<dir>` を与えると、スクリプトの一手ごとに最初のキャンバスを PNG で書き出します。
描くのは窓と同じ raster です。
`WAKAKUSA_FRAME_SCALE` は、アプリを変えずに格子を大きく描かせます。
dump が「その一コマが何であるか」を言うのに対して、これは「どう見えるか」を答えます。
ssh 越しでも、CI の中でも、画面がロックされていても動きます。

```console
$ WAKAKUSA_FRAMES=frames PIXIE_SCRIPT="advance:34,advance:34" ./demo/.gate/jump
$ ls frames
0000.png  0001.png
```

## すべての要素が取る keyword

十五の性質が、一つの名前と一つの意味で、すべての要素に乗ります。
`width`、`height`、`min_width`、`max_width`、`disabled`、`theme`、`animate`、`easing`、`enter`、`exit`、`col_span`、`row_span`、`role`、`a11y_label`、`tooltip` です。

```ruby
  button("save", disabled: @busy, tooltip: "write the file", width: 120.0)
  text "total", role: "heading", a11y_label: "the running total"
```

その名前を自分の意味で持っている要素は、自分のほうを使います。
`text` の `width` は文字の幅で、外側の箱は手を出しません。

## テーマとアニメーション

```ruby
  column(theme: "dark") { ... }
  text "saved", animate: 0.2, easing: "ease-out", enter: true
```

`theme:` は画面の一部の配色を差し替えます。
`animate:` は変化にかける時間で、`enter:` と `exit:` が現れるときと消えるときを受け持ちます。

## 窓まわり

```ruby
shortcut("cmd+s") { app.save }
menu_item("File", "Open…") { app.open }
on_key { |chord| app.typed(chord) }
on_file_drop { |path| app.load(path) }
```

これらは `run` の前に宣言し、アプリが生きているあいだ生きています。

```ruby
  clipboard_set(@text)
  @text = clipboard_get

  job = task { open_dialog("choose a file") }
  on_done(job) { @path = task_answer }
```

ダイアログは人を待つので `task` の中に置きます。
窓なしのスクリプトは `file:<path>` で答えます。
だからダイアログも、ほかと同じように検査できるやりとりになります。

## Ruby 自身の標準ライブラリ

Ruby 自身のライブラリが両方の実行で使えます。
`File`、`Dir`、`JSON`、`CSV`、`Time`、`Math`、`Net::HTTP`、ソケット、スレッド、`Enumerable` が答えることすべてです。
その手前に若草のライブラリはありません。
覚え直すものもありません。

```ruby
  require "json"

  def load
    @rows = JSON.parse(File.read(PATH))
  end
```

`demo/stdlib.rb`、`demo/files.rb`、`demo/reader.rb` が、両方の実行をそこに縛り付けています。

## データベース

データベースだけは例外です。
両方の実行が同じファイルを同じように読まなければ、意味がないからです。
エンジンを通して同じ sqlite に届きます。

```ruby
  sqlite_exec(DB, "CREATE TABLE IF NOT EXISTS notes(body TEXT)")
  sqlite_exec(DB, "INSERT INTO notes VALUES (?)", [@draft])
  rows = sqlite_rows(DB, "SELECT rowid, body FROM notes ORDER BY rowid")
  bodies = sqlite_column(DB, "SELECT body FROM notes")
```

文には `?` を書き、値は横に並べます。
そうすれば、人が打った文字が文の一部になることはありません。
値はすべて文字列で返り、入るときは列の型に従って変換されます。

## タイマー、窓の外の仕事、スレッド

```ruby
app = Clock.new
every(1.0) { app.tick }
run(app, title: "clock")
```

タイマーは `run` の前に宣言し、アプリと同じだけ生きます。
両方の実行が一つの時計で刻みます。
窓では frame が、スクリプトでは `advance:` が、その時計を進めます。

```ruby
  def start
    job = task { something_slow }
    # このブロックが、仕事が終わったあとに窓の側で走る。
    # 中の `task_answer` が、仕事の答え。
    on_done(job) { @answer = task_answer }
  end
```

どちらの呼び出しも待ちません。
`task` は仕事を始めて番号を返し、`on_done` はあとで何をするかを登録するだけです。
ハンドラはそこで終わり、窓は動き続けます。
仕事の中からアプリの状態や画面に触ってはいけません。
それはハンドラの仕事で、答えがこの形で返ってくる理由もそこにあります。

回り続けるものは普通の `Thread` にします。
そこで作られたものは、ワーカーからアプリの状態に書き込むのではなく、タイマーで拾います。
コンパイルされた実行では、そのスレッドは別の核に載るので窓は描き続けます。
CRuby では一度に一本しか走らないので、重い計算は、書いているあいだは窓を止め、出荷したものでは止めません。

## 書いているあいだ

`wakakusa run` はアプリのファイルを見ています。
保存すると窓がその変更を拾います。
クラスが読み直され、窓が持っているオブジェクトはそのクラスのものなので、新しい `view` で答えつつ、持っていた値をすべて保ちます。
`initialize` は走り直しません。
状態が生まれた場所だからです。
構文の通らないファイルは、窓をそのままにして、端末にそう言います。

## 窓なしの実行と門

`PIXIE_SCRIPT` が人の代わりをします。
エンジンが木を組み、手順で動かし、dump を印字します。

```
click:<label>      表示されている文字でボタンを押す
input:<text>       欄に打つ            submit    その欄で改行する
slide / select     つまみを動かす、選択肢を選ぶ
key:<chord>        shortcut に結ばれた打鍵
keydown:<key> / keyup:<key>    キーを押したままにする、離す
menu:<item>        メニュー項目を選ぶ  file:<path>   ダイアログの答え
drop:<path>        窓にファイルを落とす
advance:<ms>       時計を進める        theme:dark|light
dump               木を印字            a11y   読み上げが読むものを印字
```

`wakakusa gate` は、一つのスクリプトでアプリを二回走らせます。
CRuby が door 越しに走らせるものと、同じ C ABI をリンクしたコンパイル済みのバイナリです。
そして二つの記録を一バイトずつ比べます。

```console
$ wakakusa gate demo/counter.rb --script "click:+1,dump,input:Momo"
GATE OK — 3 dump lines identical in both runs
```

門が約束そのものです。
この頁のほかの部分は、門が守れるものを書くための方法です。

## 若草が断る書き方

`wakakusa check` はアプリを読み、受け取れない書き方を行番号と書き換え方つきで示します。
ビルドと門の前に毎回走ります。

- ブロックの中から書き換えるグローバル変数。アプリのオブジェクトに置きます。
- フィールドに対する `list + [item]` での追加。`dup` してから `push` と書きます。前者が何を返すかで、二つの実行は一致しません。
- リテラルのブロック以外で渡されたハンドラ、および keyword に渡す引数つきの proc。
- 繰り返しのブロックの中に書かれた、要素のブロック。必要なものを引数に取るメソッドへ移します。

どれも `test/refuse/` に文面ごと置いてあるので、拒絶の言葉が黙って変わることはありません。

## 出荷

```console
$ wakakusa build demo/todo.rb --release --app
built:  demo/.gate/todo (11.5 MB)
bundle: demo/dist/todo.app (11.7 MB)
```

`--release` は symbol table を落とします。
`--app` はバイナリを macOS のアプリケーションバンドルに包み、ad-hoc 署名をつけます。
アプリの横に `<stem>.png` か `<stem>.icns` があれば、それがアイコンになります。
バイナリはエンジンとコンパイル済みの Ruby を抱えていて、システム自身のライブラリ以外は何もリンクしません。
だからバンドルがプログラムのすべてです。
Ruby もコンパイラも入っていない機械で開きます。

## まだできないこと

- 音が出ません。エンジンに音の動詞がないので、移した二つのゲームは、原作と違って無音です。
- 種を与えた `Random` は、二つの実行で同じ生成器になりません。両方で同じ数列がほしいプログラムは、生成器を自分で書きます。二つのゲームがそうしていて、算術六行で足ります。
- コンパイラがまだ受け取れないために、決まった形で書かなければならないものが三つあります。[若草が断る書き方](#若草が断る書き方)に挙げてあります。
- `check` はそれらと、ほかに五つを見つけます。ただし、コンパイラが取りこぼすものすべてを見ているわけではありません。残りを捕まえるのは今も門です。
- macOS だけです。バイナリは描画に使うエンジンを抱えているので、小さなアプリでも 11 MB ほどになります。
