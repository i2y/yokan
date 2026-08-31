# インストール

## 開発は uv だけ

Yokan のアプリは普通の Python ファイルです。
PEP 723 のヘッダに依存を書けば、あとは uv が揃えます。

```python
# /// script
# dependencies = ["yokan"]
# ///
import yokan as ui
```

```console
$ uv run app.py
```

これで開発体験の全部（GPU 描画のウィンドウ、状態を保つライブリロード、ヘッドレス実行）が動きます。
ここに Rust は要りません。

スクリプトではなくプロジェクトに入れるなら `uv add yokan`、`yokan` コマンドは `uv tool install yokan`。pip でも入ります。

!!! note "対応環境"
    現在は **Apple silicon の macOS**、Python **3.14 以上**です。
    Linux にはまもなく対応します。

## リリースはリポジトリの clone と Rust

ネイティブビルド（`yokan build` / `yokan gate`）だけは、コンパイルが依存する Rust クレート群がリポジトリに入っているため、clone と Rust ツールチェーンが要ります。

```console
$ git clone https://github.com/i2y/yokan && cd yokan
$ yokan gate path/to/app.py --script "click:+1"
$ yokan build path/to/app.py --release --onefile
```

- Rust は [rustup](https://rustup.rs) で入れておきます。コンパイラの版はリポジトリ側で固定してあり、初回ビルド時に自動で取得されます。
- macOS では Xcode の Metal ツールチェーンも必要です（GPU エンジンのシェーダをビルドするため）。
- `yokan` コマンドは、チェックアウトの中で実行すればリポジトリを自動で見つけます。外から実行するときは環境変数 `PIXIE_REPO` にチェックアウトの場所を渡します。
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
