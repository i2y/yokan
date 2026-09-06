# エージェントと作る

エージェントはファイルを書き、返ってきたものを読みます。
何往復かかるか、自分の間違いに自分で気付けるか、人が横で見ている必要があるかは、返ってくるものが決めます。

若草のコマンドは、その返事が読みやすいように作ってあります。
三つのうち二つはコンパイラを起動せず、ウィンドウも開きません。
三つが答えるのは別々のことです。
この書き方を受け取れるか、この画面はどうなるか、リリースしたアプリも同じ動きをするか。

![エージェントの作業の流れ。app.rb を書いて wakakusa check とウィンドウなしの dump を繰り返し、どちらも 1 秒かからない。仕上がったら wakakusa gate にかけ、そのあとリリースする](images/loop-ja.svg#only-dark)

![エージェントの作業の流れ。app.rb を書いて wakakusa check とウィンドウなしの dump を繰り返し、どちらも 1 秒かからない。仕上がったら wakakusa gate にかけ、そのあとリリースする](images/loop-ja-light.svg#only-light)

## 三つのコマンド、三つの答え

### `wakakusa check`：この書き方を受け取れるか

```console
$ ./bin/wakakusa check app.rb
app.rb:5:19: Wakakusa cannot take this — `text` has no `weight:`. It takes a11y_label, align, animate, background, bold, border_color, border_radius, border_width, col_span, color, disabled, easing, enter, exit, grow, height, italic, max_lines, max_width, min_width, mono, padding, role, row_span, size, text, theme, tooltip, underline, width, wrap
    text("hello", weight: 700.0)
                  ^
```

断りを `file:line:col` の形で印字し、その行を下に添えます。
受け取れるアプリには、何も出しません。
コンパイラを起動しないので、答えは 0.1 秒ほどで返ります。

出るのは「だめだ」だけではありません。
どう直すかが、直す場所の行と一緒に出ます。
一覧は[若草が断る書き方](refusals.md)にあり、どう書くとそうなるかも並べてあります。

### ウィンドウなしの実行：これは何を描くか

```console
$ PIXIE_SCRIPT="click:+1,dump" ./bin/wakakusa run app.rb
Column(spacing=12, padding=16)[Text(count: 0, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
```

`PIXIE_SCRIPT` があるとき、`run` はウィンドウを開きません。
木を組み、手順のとおりに動かして、画面を文字にして印字します。
最初に一度、`dump` のたびに一度、最後に一度です。
これも 0.7 秒ほどで、やはりコンパイラは起動しません。

「押したボタンが思ったとおりに効いたか」の答えがこれで、画面がなくても読めます。
使える手順の一覧は[検証と配布](tour-ship.md#ウィンドウなしの実行)にあります。

キャンバスに描くものなら、`WAKAKUSA_FRAMES=<dir>` が一手ごとに PNG を書き出します。
絵を描くのはウィンドウが使うのと同じ raster なので、エージェントは自分が描いたものを読むだけでなく、見ることもできます。

### `wakakusa gate`：リリースしたアプリも同じことをするか

```console
$ ./bin/wakakusa gate app.rb --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
```

これはコンパイルするので、2 秒ほどかかります。
一区切りついたところで示す証明であって、一回の編集ごとに走らせるものではありません。

食い違ったときは、その場所を示します。

```console
GATE FAILED — the two runs diverge:
  cruby:    Text("total: 3")
  compiled: Text("total: 0")
```

正しいのは `cruby:` の行です。
その実行が本物のインタプリタだからです。
食い違いはコンパイルされた実行の側の不具合で、すでに分かっているものは[二つの実行](two-runs.md)に挙げてあります。

## 一続きの作業として

```console
$ ./bin/wakakusa check app.rb                       # 断りが出る。その場で直す
$ ./bin/wakakusa check app.rb                       # 何も出ない
$ PIXIE_SCRIPT="click:add,dump" ./bin/wakakusa run app.rb   # 画面を読む
$ ./bin/wakakusa gate app.rb --script "click:add,dump"      # 二つの実行が一致する
```

はじめの三つはどれも 1 秒かかりません。
だから編集のたびに走らせる価値があります。
最後の一つが約束にあたるもので、作業が終わったと言う前に走らせます。

## エージェントに渡すもの

三つのページを、この順で渡します。

- [最初のアプリ](tour.md)とツアーの続き。
  言語そのものが、出会う順に並んでいます。
- [若草が断る書き方](refusals.md)。
  代わりにどう書くかと、それぞれが実際に印字する文面です。
- [要素](elements.md)。
  すべての要素と、そのキーワード、型、既定値です。
  エンジンが作られるのと同じ表から生成しています。

これを読んでおけば、断られてから直すという手戻りが減ります。

## 先に伝えておくとよい二つ

**ビューは読むだけです。**
いちばん多い断りは、`view` の中の書き換えです。
状態が変わるのはハンドラで、ビューはそのあとの状態から組み直されます。

**要素のブロックは、繰り返しの中に書けません。**
書き換え方はいつも同じ形です。
行に必要なものを引数に取るメソッドを作り、繰り返しからそれを呼びます。
