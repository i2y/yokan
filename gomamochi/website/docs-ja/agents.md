# エージェントと一緒に書く

エージェントはファイルを書き、返ってきたものを読みます。
何往復かかるか、自分の誤りに自分で気付けるか、人が横で見ている必要があるか。
それは、返ってくるものの形で決まります。

Gomamochi のコマンドは、その返事が読みやすいように作ってあります。
三つのうち二つは何もビルドせず、ウィンドウも開きません。
三つはそれぞれ、別の問いに答えます。
Gomamochi はこれを受け取れるか、これは何を描くか、配ったアプリも同じことをするか。

![端末でのひとつづきの作業。エージェントが app.go を書き、gomamochi check が断って直し方を示し、直すと check は何も言わなくなり、ウィンドウなしの実行が画面を文字で返し、gomamochi gate が二つの実行の一致を報告する。速い二つは 1 秒前後、ビルドは 2.2 秒](images/loop-ja.svg#only-dark)

![端末でのひとつづきの作業。エージェントが app.go を書き、gomamochi check が断って直し方を示し、直すと check は何も言わなくなり、ウィンドウなしの実行が画面を文字で返し、gomamochi gate が二つの実行の一致を報告する。速い二つは 1 秒前後、ビルドは 2.2 秒](images/loop-ja-light.svg#only-light)

## 三つのコマンドと、三つの答え

### `gomamochi check`：Gomamochi はこれを受け取れるか

```console
$ ./bin/gomamochi check app.go
app.go:9:35: Gomamochi cannot take this — `min` is Go 1.21's, and the interpreted run does not know it. Write the comparison out (`if a < b { … }`), or a small function of your own
    func (c *Counter) clamp() { c.n = min(c.n, 10) }
                                      ^
```

`ファイル:行:桁` の形で断りを出力し、その下にその行と、桁を指す記号を置きます。
受け取れるときは何も出力しません。
まず Go 自身のパーサでファイルを読みます。
パスに `go` があれば、続けてコンパイラと同じやり方で型を検査します。
だから、綴りを間違えたメソッドや型の誤りは、Go 自身の誤りとして、Go 自身の言葉で返ります。
どちらの段階でも何もビルドしないので、答えは 1 秒もかからずに返ります。

文面は「だめだ」ではありません。
直し方そのものを、直す場所に置いています。
一覧は[Gomamochi が断る書き方](refusals.md)にあり、それぞれ文面を保持しているファイルから引いています。

### ウィンドウなしの実行：これは何を描くか

```console
$ PIXIE_SCRIPT="click:+1,dump" ./bin/gomamochi run app.go
Column(spacing=12, padding=16)[Text(count: 0, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
Column(spacing=12, padding=16)[Text(count: 1, fontSize=34), Row(spacing=8)[Button(+1), Button(+10), Button(reset)], TextField(), Text(hello, )]
```

`PIXIE_SCRIPT` があるとき、`run` はウィンドウを開きません。
木を組み、書かれた手順で動かし、画面を文字として出力します。
最初に一度、`dump` のたびに一度、最後に一度です。
かかるのは 1 秒ほどで、ここでもアプリはビルドしません。
インタプリタがファイルをそのまま読むだけです。

「押したボタンは思ったとおりに効いたか」への答えがこれで、画面がなくても読めます。
手順の一覧は[ウィンドウなしの実行とゲート](tour-ship.md#ウィンドウなしの実行とゲート)にあります。

キャンバスに描くものなら、`PIXIE_FRAMES=<dir>` が手順ごとに PNG を書き出します。
描くのはウィンドウと同じラスタライザなので、エージェントは自分の描いたものを読むだけでなく、見ることができます。

### `gomamochi gate`：配ったアプリも同じことをするか

```console
$ ./bin/gomamochi gate app.go --script "click:+1,dump"
GATE OK — 3 dump lines identical in both runs
  script:   click:+1,dump
  binary:   demo/.gate/app/app (2.9 MB)
```

これはビルドします。
`go build` でアプリをコンパイルし、そのバイナリとインタプリタを一つのスクリプトで動かし、記録を 1 バイトずつ比べます。
エンジンを最初にビルドするときは数分かかり、そのあとは毎回 2 秒ほどです。
赤いゲートは、二つの記録と、最初に食い違った行を出力します。

## 一続きの作業でのループ

1. ファイルを書く。
2. `check` が黙るまで直す。
   どの答えにも直し方が書かれているので、ここで要るのは往復であって、考え込むことではありません。
3. 足したものを押すスクリプトで、ウィンドウなしの実行。
   dump を読みます。
   それが画面です。
4. 形が決まったら `gate`。
   ここでビルドし、ここで証明します。
5. できあがったら `build --release --app`。

2 と 3 がループで、どちらも何もビルドしません。
4 でループを出ます。

## エージェントに渡すもの

- **ツアー**（[ここから](tour.md)）。
  言語そのものが、読者の出会う順に並んでいます。
  完全な例はすべて `tools/gate_all.sh` がゲートに通すので、`check` が断る書き方はそこにありません。
- **[要素のページ](elements.md)**。
  唯一の表から生成しています。
  アプリが呼べるメソッドが型と既定値つきで並んでいて、ここにないメソッドは存在しません。
  ないメソッドを呼べば、Go 自身がそう言います。
- **[Gomamochi が断る書き方](refusals.md)**。
  断りを、迂回するものではなく見覚えのあるものにするためです。
- **作りたいものに近いデモ**を[ギャラリー](demos.md)から一つ。
  どれもファイル全体で、ゲートを通っていて、短いものです。

## 伝えておくとよいこと二つ

**断りは読む。迂回しない。**
どの文面にも書き換え方が書かれています。
断りを壁として扱うエージェントは、もっと妙なものを書きます。
読むエージェントは、意図したとおりの Go を書きます。

**dump は画面である。**
木が正しいと言うまでは、ウィンドウの見た目を人に尋ねる必要はありません。
ゲートが突き合わせているのも dump なので、dump を読むエージェントは、ゲートと同じものを見ています。
