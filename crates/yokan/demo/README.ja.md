# デモ集

[English](README.md)

どれも 1 ファイル（opsboard と multi はディレクトリ）で、リポジトリの `crates/yokan/` からそのまま動きます。

```console
$ uv run demo/counter.py            # そのデモの名前に置き換える
$ ./tools/gate_all.sh               # 全デモをゲートで一括チェック
```

numpy を使う 3 本（pystats / csv_viewer / app）は `uv run --with numpy` で。
`transcribe` は依存を自分で宣言しているので `uv run demo/transcribe/app.py` がそれを取ってきます。
初めて文字起こしをするときに Whisper のモデルを取得し、ゲートはスイープではなく単独（`just transcribe-gate`）で走ります。
`app` と `csv_viewer` の 2 本は辞書 state を使う開発専用デモで、ゲート対象外です（ツアーの[今できないこと](../TOUR.ja.md#今できないこと)参照）。
スクリーンショットはすべて初期状態（起動直後）のものですが、`transcribe` だけは文字起こしを終えた状態です。
起動直後は表が空で、何も伝わらないからです。

## まず動きを見る

#### counter — いちばん小さいアプリ。同じアプリの別の書き方が counter_state.py（型付き State セル）と counter_with.py です
<img src="screenshots/counter.png" width="360">

#### opsboard — 旗艦デモ。3 モジュール構成のダッシュボード（ストア 2 つ、直和型のヘルスモデル、チャート、仮想化アラートフィード、テーマ切替、fs へのレポート出力）
<img src="screenshots/opsboard.png" width="720">

#### forms — フォーム部品一式。checkbox / switch / slider / select / radio_group / tab_bar、ハンドラは新しい値をひとつ受け取る
<img src="screenshots/forms.png" width="360">

#### calc — 定番の電卓。レイアウトは `grow` だけで組んであり（行が高さを分け合い、キーが行の幅を分け合い、0 キーは 2 コマ分）、ウィンドウを伸ばすとパッド全体が隙間なく追従する
<img src="screenshots/calc.png" width="300">

#### calcgrid — 同じ電卓を `grid(columns=4, rows=5)` で。等分トラックの一つのコンテナに全キーが並び、0 キーは `col_span=2` で 2 セルにまたがる
<img src="screenshots/calcgrid.png" width="300">

## 状態の持ち方

#### stores — 名前付きストア。クラス名がそのままシングルトンで、ストア同士のメソッド呼び出しもできる
<img src="screenshots/stores.png" width="360">

#### models — @model と Protocol。観測されるオブジェクトと、静的ディスパッチされるインターフェース
<img src="screenshots/models.png" width="360">

#### links — モデルがモデルを参照する。所有は `Node | None`、逆向きは `Weak[Node]`（循環しないので、根を手放すと連鎖ごと解放される）
<img src="screenshots/links.png" width="360">

#### stateful — @component + local。呼び出し位置ごとに独立した状態を持つ部品
<img src="screenshots/stateful.png" width="360">

#### lookup — 辞書セル。読みは `.get(key, default)`、`in`、そして `cell[k] = v` のその場書き込み
<img src="screenshots/lookup.png" width="360">

#### mixer — フィールドだけの @store。注釈付きフィールドへの直接代入で画面が追随する
<img src="screenshots/mixer.png" width="360">

## 値と型

#### points — Value クラス（frozen dataclass）。書き換えは `replace` の関数的更新
<img src="screenshots/points.png" width="360">

#### vecops — Value クラスの演算子。`__add__` / `__sub__` / `__mul__` を定義すると `+` `-` `*` がその意味になる
<img src="screenshots/vecops.png" width="360">

#### geometry — Protocol による静的ディスパッチ。トレイト相当がコンパイルされる
<img src="screenshots/geometry.png" width="360">

#### moods — Enum と Optional とアニメーション
<img src="screenshots/moods.png" width="360">

#### pyops — CPython と同じ算術。`/` `//` `%` `**`、負のインデックス、キーによる並べ替えまで両実行でバイト一致
<img src="screenshots/pyops.png" width="360">

#### pytext — 素の float / bool / Enum の表示が Python の str() と一致する
<img src="screenshots/pytext.png" width="360">

## 制御フローとエラー

#### flow — ハンドラの中の本物の制御フロー（if / elif / while / for / break / continue）
<img src="screenshots/flow.png" width="360">

#### edges — 封じ込めの実証。範囲外アクセスもオーバーフローも、両実行で同じ文が同じように止まり、アプリは落ちない
<img src="screenshots/edges.png" width="360">

#### tryfetch — try/except の全形。失敗する http 呼び出しを捕まえ、`f"{e}"` の文言まで両実行で一致する
<img src="screenshots/tryfetch.png" width="360">

## 画面部品

#### todo — 定番の TODO リスト
<img src="screenshots/todo.png" width="360">

#### table — data_table。最初の `row` がヘッダー行、以降の `row` が交互に色の付くデータ行になり、枠は要素が描く
<img src="screenshots/table.png" width="360">

#### dialog — モーダル。「存在すること」が「開いていること」なので、`if` で包む
<img src="screenshots/dialog.png" width="360">

#### trend — ライン / バーチャート
<img src="screenshots/trend.png" width="360">

#### styled — 名前付きスタイル（`style` + `**` 展開 + `|` 合成）とテーマスコープ
<img src="screenshots/styled.png" width="360">

#### cards — スロット付きコンポーネント（子要素を受け取る部品）
<img src="screenshots/cards.png" width="360">

#### layout — spacer と divider。spacer がボタンを行の端に押しやり、divider が罫線を引く（節の間は太い accent 色の線）
<img src="screenshots/layout.png" width="360">

#### about — link。URL を開くテキストと、URL をクリップボードに写すボタン
<img src="screenshots/about.png" width="360">

#### badges — 自分の箱を持つ text。状態のピル、等幅のハッシュ、下線付きの注記、省略記号、二行での打ち切り
<img src="screenshots/badges.png" width="360">

#### filter — segmented。トグルボタン群で絞り込むリスト
<img src="screenshots/filter.png" width="360">

#### quantities — number_field と int_field。enter で確定し、範囲に収め、step に吸着する型付きの数値入力
<img src="screenshots/quantities.png" width="360">

#### loading — progress の見出しと大きさ、長さの分からない作業のための不確定の往復
<img src="screenshots/loading.png" width="360">

#### canvas — 描画面。仮想的なピクセルの格子を1命令ずつ描き、色はパレットの番号で指定します。キャンバスの中の `for` と、ティックからキーの状態を読む例です
<img src="screenshots/canvas.png" width="360">

#### shooter — Pyxel のシューティングの例を移植。三つの場面、視差で流れる100個の星、揺れながら落ちてくる敵、矩形の当たり判定、広がる爆発をキャンバスの上で
<img src="screenshots/shooter.gif" width="240">

#### jump — Pyxel のジャンプゲームを移植。重力、乗ると落ちていく床、果物、そしてそれぞれの速さで流れる山と木と二層の雲
<img src="screenshots/jump.gif" width="320">

#### charts — 0 の線の下に垂れる負の値、固定した範囲、グリッド線付きの軸、色の異なる二つの系列
<img src="screenshots/charts.png" width="360">

#### roster — table。列トラック、行の選択、見出しでのソートを持つ仮想化された表（並べ替えはアプリ側）
<img src="screenshots/roster.png" width="360">

#### labels — アクセシビリティのプロパティ `role=` と `a11y_label=`。スクリプトの `a11y` ステップが印字する
<img src="screenshots/labels.png" width="360">

#### shared — 共通プロパティを要素の種類ごとに一つずつ。theme 付きの spacer、animate 付きの segmented、grid の 2 トラックにまたがるフィールド、role 付きの link、tooltip 付きの divider、disabled のボタンとフィールド、幅を指定した列
<img src="screenshots/shared.png" width="360">

## 標準ライブラリ

#### picker — ファイルダイアログと落とされたファイル。`task` の中の `fs.open_dialog` / `save_dialog`、`on_file_drop`。スクリプトは `file:<path>` で答え、`drop:<path>` で落とす
<img src="screenshots/picker.png" width="360">

#### keys — ショートカット、キー、クリップボード、メニューバー。`shortcut("cmd+s", save)`、`on_key(typed)`、`clipboard.set_text` / `get_text`、`menu_item("Count", "Save", save)`。スクリプトからは `key:cmd+s` と `menu:Save` で動かす
<img src="screenshots/keys.png" width="360">

#### files — yokan.fs。書く、足す、ディレクトリを並べる、消す（両実行が同じ実装を呼ぶ）
<img src="screenshots/files.png" width="360">

#### dbnotes — yokan.sqlite。行は SQL で形作り、ORDER BY で並べる
<img src="screenshots/dbnotes.png" width="360">

#### ledger — 実用アプリの形をした家計簿。sqlite に永続化し、値はすべてバインドで渡す
<img src="screenshots/ledger.png" width="360">

#### webfetch — yokan.http。GET、ヘッダ、POST、ステータス（@py のフィクスチャサーバを両実行に立てるので、ゲートはネットワーク不要）
<img src="screenshots/webfetch.png" width="360">

#### reader — http + jsondoc のフィードリーダー
<img src="screenshots/reader.png" width="360">

#### stdlib — Python の `math`、`random`、`statistics`、`json`、`datetime`、`time`、`re`、`collections`、`itertools` と、Yokan の jsondoc、clock
<img src="screenshots/stdlib.png" width="360">

#### dice — Python の `random`。種を撒けば両実行で同じ列
<img src="screenshots/dice.png" width="360">

#### postcard — 画像とベクタアイコン、そして `notify.send`（`.app` バンドルとして動かすと通知センターに届く）
<img src="screenshots/postcard.png" width="360">

## Rust crate

#### rustcrate — `yokan add` で足した Rust crate。手元の path crate と crates.io の version crate が同居し、crate 本来の snake_case 名で呼ぶ。同じ宣言の pyproject 綴りが `demo/proj/`
<img src="screenshots/rustcrate.png" width="360">

#### dashboard — every()。モジュールレベルで宣言したタイマーが両方の実行で動く（ゲートは `advance:` で進める）
<img src="screenshots/dashboard.png" width="360">

#### tasks — task()。重い処理を UI スレッドの外へ、両方の実行で
<img src="screenshots/tasks.png" width="360">

## CPython エスケープと開発専用

#### pystats — @py + numpy。エスケープした関数はリリースバイナリに CPython ごと同梱される
<img src="screenshots/pystats.png" width="360">

#### pyjob — @py のエスケープの中の重い Python を task で回すデモ。ウィンドウは描き続け、エスケープは載ったワーカースレッドから進み具合を報告します
<img src="screenshots/pyjob.png" width="360">

#### transcribe — Buzz の移植。@py と mlx-whisper で文字起こしをして、進捗バー、区間の表、TXT と SRT と VTT の書き出しを持ちます
<img src="screenshots/transcribe.png" width="720">

#### multi — マルチモジュール構成（state.py と widgets.py に分割、ヘルパはコンポーネントになる）
<img src="screenshots/multi.png" width="360">

#### app — numpy 入りのダッシュボード（開発専用: 辞書 state）
<img src="screenshots/app.png" width="360">

#### csv_viewer — 10 万行の仮想化テーブル + numpy（開発専用: 辞書 state）
<img src="screenshots/csv_viewer.png" width="360">
