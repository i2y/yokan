# インストール

Gomamochi は今のところリポジトリの中にあります。
インストールする配布物はなく、手元の `go.mod` に足すものもありません。
チェックアウトを取ってくれば、あとは `./bin/gomamochi` だけで足ります。

## 必要なもの

- **Apple silicon の macOS、または Linux**。
  エンジンはプラットフォーム自身の GPU の仕組みで描きます。
  macOS では Metal、Linux では Vulkan で、ウィンドウは Wayland か X11 に開きます。
- **Go 1.25 以降**。
  コマンド自身を一度ビルドするのに使い、`build` と `gate` がアプリをコンパイルするのにも使います。
  コマンドがビルドできていれば、`run` に Go のツールチェインは要りません。
  インタプリタはコマンドの中にあります。
- **Rust**（[rustup](https://rustup.rs) から）。
  コンパイラのバージョンはリポジトリが固定していて、最初のビルドで取ってきます。
- **macOS では Xcode の Metal ツールチェーン**。
  エンジンがビルド時にシェーダをコンパイルするからです。
  **Linux では C コンパイラと、エンジンがリンクするライブラリの開発パッケージ**が要ります。
  alsa、fontconfig、freetype、xkbcommon（x11 の分も）、xcb、そして Vulkan のローダとドライバです。

インタプリタ（[yaegi](https://github.com/traefik/yaegi)）と、cgo なしで Go からエンジンのライブラリを開くもの（[purego](https://github.com/ebitengine/purego)）は Go のモジュールで、コマンドを最初にビルドするときに `go` が取ってきます。
ほかに要るものはありません。

## マシンごとに一度

```console
$ export CARGO_TARGET_DIR=$HOME/.cache/pixie/target
```

`CARGO_TARGET_DIR` は、チェックアウトの中のすべてのクレートが共有します。
二度目以降のビルドが速いのはこれのおかげです。
シェルの設定に書いて、あとは忘れてかまいません。

ほかに黙って取ってくるものはありません。
エンジンは同じチェックアウトの中のクレートで、最初にどれかのコマンドを実行したときに `cargo` がビルドします。
どのコマンドも先にエンジンをビルドするので、二つの実行のどちらかが古い版になることはありません。
コマンドは Go のプログラムで、`bin/gomamochi` がそれを `~/.cache/gomamochi/bin/` にビルドして実行します。

## コマンド

`gomamochi/` から実行します。

```console
$ ./bin/gomamochi run   demo/counter.go                   # ウィンドウが開く。保存で読み直す
$ ./bin/gomamochi check demo/counter.go                   # 受け取れない書き方
$ ./bin/gomamochi gate  demo/counter.go --script "click:+1,dump"
$ ./bin/gomamochi build demo/counter.go --release --app   # バイナリと、バンドル
```

`translate` はありません。
翻訳するものがないからです。
コンパイルした実行は、書いたファイルをそのまま Go のコンパイラに通したものです。

`run` はウィンドウを開き、ファイルを見ています。
保存すればその場で効き、アプリの値は残ります。
`check` は何もビルドしません。
肝心なのは `gate` です。
二つの実行を一つのスクリプトで動かし、1 バイトずつ突き合わせます。

`build` には四つのフラグがあります。
`--release` は symbol table を落とします。
`--app` はバイナリとエンジンのライブラリをアプリケーションとして包みます。
macOS では `dist/` の下のバンドルで、ダブルクリックで開き、アプリの横に `<stem>.icns` か `<stem>.png` があればそれがアイコンになります。
Linux では `dist/` の下の AppDir で、`--appimage` がそれを一つのファイルに詰めます。
`--carry-libs` は、Linux のパッケージにエンジンがリンクするデスクトップのライブラリまで持たせます。
それらが入っていないかもしれないマシンのためです。
`gate` には `--fresh <path>` があり、実行のたびにその場所を消します。
ファイルやデータベースを持つアプリを、二つの実行とも同じ「何もない」状態から始めるためです。

## 最初のファイル

```go
package main

import (
	"fmt"

	. "github.com/i2y/yokan/gomamochi"
)

type Counter struct {
	count int
}

func (c *Counter) View() Element {
	return Column(
		Text(fmt.Sprintf("count: %d", c.count)).Size(34),
		Button("+1").OnClick(func() { c.count += 1 }),
	).Spacing(12).Padding(16)
}

func main() {
	Run(&Counter{}, Title("counter"))
}
```

```console
$ ./bin/gomamochi run app.go
```

そのウィンドウを開いたまま編集して保存してみてください。
新しいインタプリタがファイルを読み直し、ウィンドウの持っている構造体の値は、すべて新しいコードに引き継がれます。

## ビルドすると何ができるか

macOS/arm64 で、エンジンをビルドし終え、キャッシュが温まった状態で測っています。

| もの | 値 |
|---|---|
| `gate` がビルドするバイナリ | 2.9 MB |
| 配るバイナリ（`--release`） | 1.9 MB |
| その横に置くエンジンのライブラリ | 20.4 MB |
| 両方を入れたアプリケーションバンドル（`--app`） | 22.2 MB |
| `bin/gomamochi` 経由の `check` | 0.6 秒。検査そのものは数十ミリ秒で、残りは `go build` がコマンドが最新かを確かめる時間 |
| `PIXIE_SCRIPT` でのウィンドウなしの実行 | 1.0 秒 |
| ゲート 1 回 | 約 2 秒 |
| 全部を 1 回通す（デモ 44 本、断り、ツアー） | 約 1 分 |

バイナリは Go のコンパイラが作るそのもので、cgo を使わず、システム自身のライブラリ以外は何もリンクしません。
エンジンは一つの共有ライブラリとして、その横に置かれます。
受け取る人に、Go もツールチェインも要りません。

## 全体が動いているかを確かめる

```console
$ just gomamochi-sweep
```

生成した語彙と表の照合、Go 自身の `vet`、断りの文面を置いてあるファイル、すべてのデモの両方の実行、そしてツアーの完全な例を、すべて確かめます。
要素の表やエンジンを開くパッケージ、エンジンに手を入れたときに通すものですが、手元の用意が揃っているかを確かめるいちばん速い方法でもあります。

## 次に読むもの

- [はじめてのアプリ](tour.md)。
  言語そのものが、読者の出会う順に並んでいます。
- [デモ](demos.md)。
  44 本のアプリを、画面写真とソースつきで。
