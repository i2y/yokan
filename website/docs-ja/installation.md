# インストール

## 開発は uv だけ

Yokan のアプリは普通の Python ファイルです。
PEP 723 のヘッダに依存を書けば、あとは uv が揃えます。

```python
# /// script
# requires-python = ">=3.14"
# dependencies = ["yokan"]
# ///
from yokan import State, button, column, run, text
```

```console
$ uv run app.py
```

GPU で描くウィンドウも、状態を保ったままのライブリロードも、ヘッドレス実行も、これだけで動きます。
Rust は要りません。

スクリプトではなくプロジェクトで使うなら `uv add yokan` です。
`yokan` コマンド自体を入れるなら `uv tool install yokan` を使います。
pip でも入ります。

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
下の二つはコンパイルするので Rust が要ります。
コンパイル先のクレートは、この二つが自動で取ってきます。

この流れのどこででも `yokan translate app.py` を実行すれば、リリースビルドがコンパイルする `.pix` が出てきます。

!!! note "対応環境"
    現在は **Apple silicon の macOS**、Python **3.14 以上**です。
    Linux にはまもなく対応します。

## リリースに要るのは Rust ツールチェーン

- Rust は [rustup](https://rustup.rs) で入れておきます。
  コンパイラの版はリポジトリ側で固定してあり、初回ビルドのときに自動で取得されます。
- macOS では Xcode の Metal ツールチェーンも必要です（GPU エンジンのシェーダをビルドするため）。
- コンパイル先の Rust クレート群はリポジトリに入っています。
  最初の `gate` か `build` が、使っている版に合うチェックアウトを `~/.cache/yokan/` に取ってきます（約 11 MB）。
  手で clone するものはありません。
  チェックアウトの中で `yokan` を実行すればそちらを使い、`PIXIE_REPO` を指せば別の場所も使えます。
- 初回はエンジンごとコンパイルするので数分かかります。
  二回目からは差分だけです。

## 更新とキャッシュ

更新は `uv tool upgrade yokan`（`pip install -U yokan` でも同じ）です。
次のネイティブビルドが新しい版のチェックアウトを取ってきて、古いほうを消します。
ビルドの成果物はチェックアウトの中ではなく、その隣に置いてあります。
だから版を上げても、変わったところだけコンパイルすれば済みます。

```console
$ yokan version
yokan 0.2.1
  checkout  ~/.cache/yokan/repo-0.2.1 (v0.2.1, fetched)
  builds    ~/.cache/yokan/target (3.4G)
$ yokan clean                                # キャッシュを捨てる
```

`~/.cache/yokan/` の中身は、取り直しとビルドし直しで元に戻せるものだけです。
だから、状態が怪しくなったときは `clean` で捨てて作り直せます。

## エージェントに書かせる

[`skills/yokan/SKILL.md`](https://github.com/i2y/yokan/blob/main/skills/yokan/SKILL.md) は、エージェントのために書いたガイドです。
方言の全体に加えて、拒否される書き方と、代わりに何を書くかが入っています。
エージェントがスキルを探す場所に置いてください。
Claude Code なら `~/.claude/skills/` です。

```console
$ curl --create-dirs -o ~/.claude/skills/yokan/SKILL.md \
    https://raw.githubusercontent.com/i2y/yokan/main/skills/yokan/SKILL.md
```

これを読ませておくと、ビルドで断られてから直す、という手戻りが減ります。

エージェントが回す往復（三つのコマンドと、それぞれが返すもの）は[エージェントと作る](agents.md)で説明しています。

## ビルドで何ができるか

`@py` を使っていないアプリなら、実行ファイルに CPython は入りません。
Python へのリンクは一つもなく、大きさは **14.7 MB**（strip 後 11.3 MB）、起動は数ミリ秒です。

`@py` を使うアプリは CPython ごと同梱します。

```console
$ yokan build app.py --release --bundle    # ランタイム同梱のアプリフォルダ
$ yokan build app.py --release --onefile   # 1 ファイル配布
```

`--onefile` は stdlib のみで約 **17 MB**、numpy 込みで約 **21 MB**。
初回起動でキャッシュへ展開し、以後は約 40 ms で起動します。
`--app` を足せば（単独でも `--bundle` と組でも）`dist/` に macOS の `.app` バンドルができます。
Dock に名前が出て、ダブルクリックで起動できます。
隣に `<名前>.png` を置いておけば、それがアイコンになります。
どちらの場合も、受け取る側のマシンに Python も pip も要りません。

実測値（macOS/arm64、リリースビルド）: 起動 4.7 ms、ライブリロード約 1 ms。
