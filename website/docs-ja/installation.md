# インストール

## 開発は uv だけ

Yokan のアプリは普通の Python ファイルです。
PEP 723 のヘッダに依存を書けば、あとは uv が揃えます。

```python
# /// script
# dependencies = ["yokan"]
# ///
from yokan import State, button, column, run, text
```

```console
$ uv run app.py
```

これで開発体験の全部（GPU 描画のウィンドウ、状態を保つライブリロード、ヘッドレス実行）が動きます。
ここに Rust は要りません。

スクリプトではなくプロジェクトに入れるなら `uv add yokan`、`yokan` コマンドは `uv tool install yokan`。pip でも入ります。

## 最初のファイルからリリースまで

```console
$ uv tool install yokan                     # yokan コマンド
$ yokan init app.py                         # 最初のファイル
$ uv run app.py                             # 開発: ウィンドウとライブリロード
$ yokan check app.py                        # 方言の内側かどうか
$ yokan gate app.py --script "click:+1"     # 二つの実行を突き合わせる
$ yokan build app.py --release --onefile    # 1 ファイルで配る
```

最初の四つは uv だけで動きます。
下の二つはコンパイルするので Rust が要り、コンパイル先のクレートは自分で取ってきます。

`yokan translate app.py` は、途中のどこででも、リリースビルドがコンパイルする `.pix` を出力します。

!!! note "対応環境"
    現在は **Apple silicon の macOS**、Python **3.14 以上**です。
    Linux にはまもなく対応します。

## リリースに要るのは Rust ツールチェーン

- Rust は [rustup](https://rustup.rs) で入れておきます。コンパイラの版はリポジトリ側で固定してあり、初回ビルド時に自動で取得されます。
- macOS では Xcode の Metal ツールチェーンも必要です（GPU エンジンのシェーダをビルドするため）。
- コンパイル先の Rust クレート群はリポジトリに入っていますが、最初の `gate` か `build` が、使っている版に合うチェックアウトを `~/.cache/yokan/` に取ってきます（約 11 MB）。手で clone するものはありません。チェックアウトの中で `yokan` を実行すればそちらを使い、`PIXIE_REPO` を指せば別の場所も使えます。
- 初回はエンジンごとコンパイルするので数分かかります。二回目からは差分だけです。

## ビルドで何ができるか

`@py` を使っていないアプリなら、実行ファイルに CPython は入りません。
Python へのリンクもゼロで、**13.4 MB**（strip 後 10.4 MB）、起動は数ミリ秒です。

`@py` を使うアプリは CPython ごと同梱します。

```console
$ yokan build app.py --release --bundle    # ランタイム同梱のアプリフォルダ
$ yokan build app.py --release --onefile   # 1 ファイル配布
```

`--onefile` は stdlib のみで約 **17 MB**、numpy 込みで約 **21 MB**。
初回起動でキャッシュへ展開し、以後は約 40 ms で起動します。
`--app` を足せば（単独でも `--bundle` と組でも）`dist/` に macOS の `.app` バンドルができます。Dock 名とダブルクリック起動、隣に `<名前>.png` があればアイコンも付きます。
どちらの場合も、受け取る側のマシンに Python も pip も要りません。

実測値（macOS/arm64、リリースビルド）: 起動 4.7 ms、ライブリロード約 1 ms。
