# エージェントと作る

エージェントはファイルを書き、返ってきたものを読みます。
その往復が何回で終わるか、間違いに自分で気付けるか、人が横で見ている必要があるか。
どれも、返ってくるものが何かで決まります。

Yokan のコマンドは、読まれることを前提に返します。
三つあって、それぞれが一つの問いに答えます。
この書き方は方言の中か、アプリは何をするか、出荷する側も同じことをするか。
最初の二つはコンパイラを起動せず、ウィンドウも開きません。

![端末での一回分。エージェントが app.py を書き、yokan check が拒否して直し方を名指しし、同じ check が今度は何も言わず、yokan show が画面をテキストで印字し、yokan gate が両方の実行で同じ画面になったと報告する。どの答えもテキストなので、エージェントはそれを読んでまた回る](images/loop-ja.svg#only-dark)

![端末での一回分。エージェントが app.py を書き、yokan check が拒否して直し方を名指しし、同じ check が今度は何も言わず、yokan show が画面をテキストで印字し、yokan gate が両方の実行で同じ画面になったと報告する。どの答えもテキストなので、エージェントはそれを読んでまた回る](images/loop-ja-light.svg#only-light)

## 三つのコマンド、三つの答え

### `yokan check`：この書き方は方言の中か

```console
$ yokan check app.py
app.py:8:17: not in the dialect — rect()'s `y` is a whole number of pixels — this reads as a float, so write `int(...)` around it
        rect(8, y() * 1.5, 8, 8, 1)
                ^
```

アプリが import するモジュールをすべて読み、最初の拒否を `file:line:col` の形で、その行を添えて印字します。
方言の中にあるときは何も言いません。
コンパイラを起動しないので、答えは約1秒で返ります。

拒否は、代わりに何を書くかを名指しします。
返ってくるのは断り文句ではなく直し方で、しかも直す場所に添えて返ります。
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

スクリプトを流してアプリを動かし、ウィンドウは開かずに画面をテキストで印字します。
スクリプトの語彙は、人がアプリにできることです（クリック、入力、キー、ファイルのドロップ、33 ms 進めること）。
だから「左を押したまま2フレーム進んだとき、画面に何があるか」は、印字できる答えを持つ問いになります。

`--frames` を付けると、各ステップのキャンバスを PNG でも残します。
描くのはウィンドウと同じラスタライザです。
テキストは1フレームが何であるかをコマンド単位で言い、PNG はそれがどう見えるかを言います。

### `yokan gate`：出荷する側も同じことをするか

```console
$ yokan gate app.py --script "click:+1,input:Momo" --release
GATE OK — 2 dump lines identical in both runs
```

こちらはコンパイルします。
同じスクリプトを開発実行とコンパイル済みバイナリの両方に流し、画面をバイト単位で比べます。
違えば、その行を両方の実行から並べて出します。
遅いのはこれで、コンパイラが要るのもこれだけです。
作業は前の二つで進み、これは変更が終わったときに走らせます。

## 答えがテキストであること

最初の二つは画面装置を使いません。
ssh 越しでも CI でも、エージェントが動いている場所がどこであっても動きます。
ウィンドウが描かれるのを待つものもありません。

画面のテキストは安定しています。
同じ要素が同じ順で、毎回同じ形で並びます。
だから二つの実行の差分に意味があり、1行に対する検査がそのままテストになります。

## 同じ実行を、テストから

アプリは普通の Python のモジュールなので、テストも普通の Python のテストです。
使い慣れた実行係をそのまま使えます。
`yokan.headless(view, state, script)` は `yokan show` が行う実行そのもので、Python から呼べて、画面を文字列で返します。

```python
# test_app.py
import app
from yokan import headless


def test_clicking_counts():
    assert "count: 2" in headless(app.view, None, "click:+1,click:+1")
```

ハンドラもストアのメソッドも value クラスも普通の Python なので、計算だけの部分は直接呼んで確かめられます。
テストは「アプリが正しいことをする」と言い、ゲートは「コンパイル済みのアプリが同じことをする」と言います。
どちらもツアーの[テストの節](tour-ship.md#テスト)に詳しく書いてあります。

## この往復でわからないこと

ゲートが証明するのは、二つの実行が一致することです。
ウィンドウが正しく見えることではありません。
レイアウトは、両方で同じように間違っていることがあります。
余白も色も、そもそも画面が読めるかどうかも、目で見るしかない部分です。
ビルドして一度起動し、見てください。
人がいるなら、そこが訊く場面です。

## エージェントにガイドを渡す

[`skills/yokan/SKILL.md`](https://github.com/i2y/yokan/blob/main/skills/yokan/SKILL.md) は、方言の全体を1ファイルにまとめた、エージェントのためのガイドです。
拒否される書き方と、代わりに何を書くかが全部入っています。
エージェントがスキルを探す場所に置いてください。Claude Code なら `~/.claude/skills/` です。

```console
$ curl --create-dirs -o ~/.claude/skills/yokan/SKILL.md \
    https://raw.githubusercontent.com/i2y/yokan/main/skills/yokan/SKILL.md
```

これを読ませておくと、最初から部分集合の中に書けます。
ビルドで断られてから知る、という手戻りがなくなります。
外に出たときのために拒否は残っていますし、だから三つのうち最初のものは、編集のたびに走らせる価値があります。
