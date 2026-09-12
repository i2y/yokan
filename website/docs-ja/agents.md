# エージェントと作る

エージェントはファイルを書き、返ってきたものを読みます。
その往復が何回で終わるか、間違いに自分で気付けるか、人が横で見ている必要があるか。
どれも、返ってくるものが何かで決まります。

Yokan のコマンドは、エージェントが読むことを前提に答えを返します。
三つあって、それぞれが一つの問いに答えます。
この書き方は方言の中か、アプリは何をするか、リリースする側も同じことをするか。
最初の二つはコンパイラを起動せず、ウィンドウも開きません。

![端末での一回分。エージェントが app.py を書き、yokan check が直し方を示して拒否し、同じ check が今度は何も言わず、yokan show が画面をテキストで出力し、yokan gate が両方の実行で同じ画面になったと報告する。どの答えもテキストなので、エージェントはそれを読んでまた回る](images/loop-ja.svg#only-dark)

![端末での一回分。エージェントが app.py を書き、yokan check が直し方を示して拒否し、同じ check が今度は何も言わず、yokan show が画面をテキストで出力し、yokan gate が両方の実行で同じ画面になったと報告する。どの答えもテキストなので、エージェントはそれを読んでまた回る](images/loop-ja-light.svg#only-light)

## 三つのコマンド、三つの答え

### `yokan check`：この書き方は方言の中か

```console
$ yokan check app.py
app.py:8:17: not in the dialect — rect()'s `y` is a whole number of pixels — this reads as a float, so write `int(...)` around it
        rect(8, y() * 1.5, 8, 8, 1)
                ^
```

アプリが import するモジュールをすべて読み、見つかった拒否を全部出力します。
`file:line:col` の形で、問題の行も添えて出てきます。
一度の実行でファイル全体の答えが返ります。
そうでなければ、拒否の数だけ往復することになります。
方言の中にあるときは何も言いません。
コンパイラを起動しないので、答えは約1秒で返ります。

拒否のメッセージには、代わりに何を書けばよいかまで書いてあります。
`--strict` を付けると警告でも失敗します。

### `yokan show`：何をして、どう見えるか

```console
$ yokan show app.py --script "keydown:left,advance:33,advance:33" --frames shots/ --scale 3
Column[Canvas(160x120, scale=4, bg=#000000)[
  Sprite(assets/sheet.png, 0,0 8x8 at 54,100)
  PixelText(4, 4, "SCORE 0", #eeeeee)
]]

3 frames in shots/
```

スクリプトを流してアプリを動かし、ウィンドウは開かずに画面をテキストで出力します。
スクリプトに書けるのは、人がアプリにできる操作です（クリック、入力、キー、ファイルのドロップ、33 ms 進める）。
だから「左を押したまま2フレーム進んだとき、画面に何があるか」も、そのまま出力して確かめられます。

`--frames` を付けると、各ステップのキャンバスを PNG でも残します。
描くのはウィンドウと同じラスタライザです。
テキストはフレームの中身をコマンド単位で書き出したもので、PNG はその見た目です。

### `yokan gate`：リリースする側も同じことをするか

```console
$ yokan gate app.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical in both runs
```

こちらはコンパイルします。
同じスクリプトを開発実行とコンパイル済みバイナリの両方に流し、画面をバイト単位で比べます。
違っていたら、食い違った行を両方の実行から並べて表示します。
三つのうち時間がかかるのはこれで、コンパイラが要るのもこれだけです。
普段の作業は前の二つで進めて、これは変更が終わったときに走らせます。

## 答えがテキストであること

最初の二つはディスプレイを使いません。
ssh 越しでも CI でも、エージェントが動いている場所がどこでも使えます。
ウィンドウが描かれるのを待つ必要もありません。

画面のテキストは安定しています。
同じ要素が同じ順で、毎回同じ形で並びます。
だから二つの実行の差分に意味があり、1行を検査するだけでテストになります。

## 同じ実行を、テストから

アプリは普通の Python のモジュールなので、テストも普通の Python のテストです。
wheel が pytest のプラグインを登録するので、実行はそのままフィクスチャとして使えます。
`app` はアプリのモジュールを実行せずに読み込んだもので、`run` はウィンドウを出さずにそれを動かします。

```python
# tests/test_app.py
def test_clicking_counts(app, run):
    assert "count: 2" in run(app, "click:+1,click:+1")


def test_the_screen_is_what_it_was(app, run, snapshot):
    snapshot(run(app, "click:+1"))       # 記録して、以後は突き合わせる


def test_the_compiled_run_agrees(gate):
    gate("click:+1")                     # テストの中から呼ぶゲート
```

新しいアプリを作ると、`yokan init` がこの一つめをアプリの隣に書き、それを走らせるワークフローも書きます。

ハンドラもストアのメソッドも value クラスも普通の Python なので、計算だけの部分は直接呼んで確かめられます。
テストはアプリが正しく動くことを確かめ、ゲートはコンパイル済みのアプリが同じように動くことを確かめます。
どちらもツアーの[テストの節](tour-ship.md#テスト)に詳しく書いてあります。

## この往復でわからないこと

ゲートが証明するのは、二つの実行が一致することです。
ウィンドウが正しく見えることではありません。
レイアウトは、両方で同じように間違っていることがあります。
余白も色も、そもそも画面が読めるかどうかも、目で見るしかない部分です。
ビルドして一度起動し、見てください。
近くに人がいるなら、そこで訊いてください。

## エージェントにガイドを渡す

[`skills/yokan/SKILL.md`](https://github.com/i2y/yokan/blob/main/skills/yokan/SKILL.md) は、方言の全体を1ファイルにまとめた、エージェントのためのガイドです。
拒否される書き方と、代わりに何を書くかが全部入っています。
エージェントがスキルを探す場所に置いてください。
Claude Code なら `~/.claude/skills/` です。

```console
$ curl --create-dirs -o ~/.claude/skills/yokan/SKILL.md \
    https://raw.githubusercontent.com/i2y/yokan/main/skills/yokan/SKILL.md
```

これを読ませておくと、ビルドで断られてから直す、という手戻りが減ります。
それでも方言から外れることはあるので、`yokan check` は編集のたびに走らせる価値があります。
