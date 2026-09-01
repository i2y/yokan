# pixie 言語ツアー

pixie の全体像を一周するガイドです。ここに載っているコードは全部、
今日のツリーでそのままコンパイル・実行できます(できないことは
最後の「今できないこと」にまとめてあります)。英語版は
[TOUR.md](TOUR.md)。

## 目次

1. [30秒の pixie](#1-30秒の-pixie)
2. [コンパイルの仕組み — 検証器は二人いる](#2-コンパイルの仕組み)
3. [基本文法](#3-基本文法)
4. [型システム — trait・ジェネリクス・`T?`](#4-型システム)
5. [メモリ管理 — World とハンドル](#5-メモリ管理)
6. [クラス・store・リアクティビティ](#6-クラスstoreリアクティビティ)
7. [view とスタイル](#7-view-とスタイル)
8. [エラー処理と `T?`](#8-エラー処理と-t)
9. [async と HTTP](#9-async-と-http)
10. [Rust バインディング — crates.io が標準ライブラリ](#10-rust-バインディング)
11. [モジュールとパッケージ](#11-モジュールとパッケージ)
12. [二層実行とホットリロード](#12-二層実行とホットリロード)
13. [CLI 早見表](#13-cli-早見表)
14. [今できないこと](#14-今できないこと)

---

## 1. 30秒の pixie

```ruby
store Session {
  state name : String = ""
  state saved : Bool = false

  fn update(t: String) {
    name = t
  }

  async fn save {
    await Fs.writeString("/tmp/pixie-hello.txt", name)
    saved = true
  }
}

view Main {
  Column {
    TextField {
      text: Session.name
      placeholder: "your name"
      onTextChanged: Session.update(text)
    }
    Button { text: "save"; onClick: Session.save() }
    if Session.saved {
      Text { text: "saved: #{Session.name}" }
    }
  }
}
```

- `store` はプロセス唯一のリアクティブな状態置き場
- `view` は宣言的な UI ツリー。状態が変われば勝手に再描画
- `async fn` + `await` はブロッキング処理をワーカースレッドへ
- `Fs` は Rust の `std::fs::write` への 2 行のバインディング

実行は:

```sh
pixie build hello.pix --run     # ウィンドウが開く
pixie watch hello.pix           # 保存ごとに約1msでホットリロード
PIXIE_SCRIPT="input:Ada,click:save" ./hello   # ヘッドレスでUI操作を再生
```

---

## 2. コンパイルの仕組み

pixie の設計上いちばん大事な絵がこれです:

```
  .pix ソース
     │
     ▼
┌─────────────────┐   パース・型検査・可視性・スタイル展開
│  pixie フロント  │   (Cute からのフォーク)
│  エンド          │──── ここで pixie としてのエラーは全部出す
└─────────────────┘
     │  Rust コードを「生成」
     ▼
┌─────────────────┐
│  生成された Rust │   借用検査を必ず通る形しか
│  (人は読まない)  │   pixie は書かない
└─────────────────┘
     │
     ▼
┌─────────────────┐
│  rustc          │◀─── 第二の検証器。ここでエラーが出たら
│  (第二の検証器)  │     それは「あなたのバグ」ではなく
└─────────────────┘     「pixie コンパイラのバグ」
     │
     ▼
  ネイティブバイナリ(gpui で GPU 描画)
```

pixie が書く Rust は借用検査を必ず通ります。
所有権や借用が書き手に降りてくることはなく、
その代わり rustc が pixie の書いたプログラムを一つ残らず検証します。

---

## 3. 基本文法

### コメント・リテラル・補間

```ruby
# コメントは # から行末まで

let n = 42               # Int   (i64)
let x = 3.14             # Float (f64)
let ok = true            # Bool
let s = "hi"             # String
let msg = "n = #{n}"     # 文字列補間は #{expr}
let xs = [1, 2, 3]       # List<Int>
let m : Map<String, Int> = { a: 1, "b-key": 2 }
                         # Map リテラル。識別子キーは文字列の糖衣
```

小数が来る場所に整数を書けます。
`fontSize: 14`、`let ratio : Float = 3`、`30.0 * count` はどれもそのまま通り、
一つの式に両方が混ざれば広いほうの型になります。

補間には式と書式指定が書けます。

```ruby
"#{n * 2} of #{n + 1}"     # 算術
"#{v:.2f}"                 # 3.14
"#{n:>6}"   "#{n:04}"      # 幅、ゼロ埋め
"#{s:*^9}"                 # 中央寄せ、* で埋める
```

指定に書けるのは、幅、寄せ(`<` `^` `>`)、ゼロまたは任意の埋め文字、
`.精度`、基数(`x` `X` `o` `b`)です。末尾の型文字(`f` や `d`)は
書いても構いませんが無視されます。それ以外はコンパイルエラーになり、
その指定を名指しします。

### let / var と代入

```ruby
fn demo String {
  let a = 1          # 不変
  var b = 10         # 可変
  b += 5             # 複合代入: += -= *= /=
  var s : String = "x"
  s += "!"           # 文字列連結は + / +=
  "#{a} #{b} #{s}"   # 末尾の式が返り値
}
```

### 関数

```ruby
fn add(a: Int, b: Int) Int {   # 返り値型は矢印なしで後置
  a + b                        # 最後の式が返る。return も使える
}

fn greet(name: String) {       # 返り値なし = Void
  # ...
}
```

### 制御構文

```ruby
if n > 2 {
  # ...
} else {
  # ...
}

case color {                   # 列挙体のパターンマッチ
  when Red { ... }
  when Green { ... }
  when Blue { ... }
}

for x in xs {                  # List<T> の走査
  if x == 2 { continue }
  if x > 8 { break }
  sum += x
}
for i in 0..n { ... }          # 範囲: 0..n は末尾除外、0..=n は含む
while i > 0 { i -= 1 }
```

宣言済み列挙体への `case` は**網羅性チェック付き**です。variant の
抜けは不足を名指しするコンパイル**エラー**になります(`when _` が
キャッチオール)。演算子: `+ - * / %`、比較 `< <= > >= == !=`、
論理 `&& || !`。

### リストとマップの読み方

```ruby
xs[i]            # T   — 要素 i が無ければプログラムエラー
xs.get(i)        # T?  — 無ければ nil
xs.first()       # T?
m[k]             # V?  — マップは「無い」がふつうなので nil
xs.length        # Int
```

リストは「リスト型の式」であれば何でもよく、フィールド経路もそのまま書けます。
`for k in node.kids { ... }`、`bag.items.length`、`kept[0].tag.label`。

### struct(値型)

```ruby
struct Point {
  var x: Int
  var y: Int

  fn sum Int {
    self.x + self.y     # struct メソッド内は self.field
  }
}

let p = Point(3, 4)     # 位置引数で構築
p.sum()                 # => 7
```

### enum と error

```ruby
enum Color {
  Red
  Green
  Blue
}

error MathError {        # エラー用 enum(§8参照)
  divByZero
  negative(v: Int)       # ペイロード付きバリアント
}
```

### テスト

```ruby
test fn addition {
  assert_eq(add(2, 2), 4)
}

suite "edge cases" {
  test "zero" { assert_eq(add(0, 0), 0) }
}
```

`pixie test file.pix` が TAP 形式で実行します。
アサート: `assert_eq / assert_neq / assert_true / assert_false`。

---

## 4. 型システム

**静的・名前的型付け + 全プログラムを rustc が再検証**、が骨格です。

```
                     型の風景
┌──────────────────────────────────────────────┐
│ プリミティブ   Int  Float  Bool  String  Bytes │
│                                              │
│ コレクション   List<T>     Map<K, V>          │
│  (COW 値)     (可変長)    (キー順序ソート)     │
│                                              │
│ 修飾          T?          !T                 │
│               nullable    エラー union        │
│                                              │
│ ユーザー定義   class C     struct S    enum E │
│               (World 常駐) (値)       (値)    │
│                                              │
│ 抽象          trait X  +  ジェネリクス <T: X>  │
└──────────────────────────────────────────────┘
```

### trait — 複数の型が共有する振る舞いに名前を付ける

`trait` は「何ができるか」を宣言し、`impl` は「ある型がそれをどうやるか」を
書きます。型システムの両側が参加できます。World に住む `class` も、
値である `struct` もです。

```ruby
trait Labeled {
  fn tag String
}

class Dog    { pub prop name : String, default: "rex" }
struct Badge { var text: String }

impl Labeled for Dog   { fn tag String { "dog:#{name}" } }
impl Labeled for Badge { fn tag String { "badge:#{self.text}" } }
```

その trait で境界を付けた関数は、どちらも受け取れます。
呼ばれた型ごとに専用版が生成されるので、trait 経由の呼び出しは
直接呼ぶのと同じコストです。

```ruby
fn describe<T: Labeled>(x: T) String {
  "<#{x.tag()}>"
}

describe(Dog())            # => <dog:rex>
describe(Badge("hi"))      # => <badge:hi>
```

trait を実装していない型を渡すと、呼び出し側でコンパイルエラーになります
(`type Int does not implement trait Labeled`)。
trait を実装しても、その型が元から持っているメソッドは影響を受けません。
上の `Badge` は他のメソッドをそのまま持てます。

未対応(いずれも名前付きエラー): ジェネリッククラスへの trait impl と、
クラス/struct の型パラメータへの境界。

### ジェネリクス — 単相化は rustc の仕事

pixie のジェネリクスは**本物の Rust ジェネリクスへそのまま**
コンパイルされます。pixie 側でのコード膨張・スタンプ処理はゼロ。

```ruby
# ジェネリック struct — 構築時に型推論
struct Pair<T> {
  var a: T
  var b: T

  fn swapped Pair<T> { Pair(self.b, self.a) }
}
let p = Pair(1, 2)          # Pair<Int> と推論

# ジェネリッククラス — 構築時は型引数を明示
class Basket<T> {
  pub prop items : List<T>, default: []
  pub fn put(v: T) { items.push(v) }
}
# view 内: let names = Basket<String>()
```

- クラス/struct の型パラメータは現状**非境界のみ**(境界付きは
  名前付きエラー)

### nullable `T?` と `nil`

```ruby
fn pick(id: Int) String? {
  if id == 1 {
    return "ada"      # 自動で some に包まれる
  }
  nil                 # absent
}

case pick(2) {
  when some(v) { ... }   # v : String
  when nil { ... }
}

if let some(v) = pick(1) {     #2アームの case を短く書いた形
  ...
} else {
  ...                          # else は省略できる
}

let held : String? = pick(1)   # T? はローカルにも持てる
```

どちらもメソッド、ハンドラ、view 本体のどこでも書けます。
view の中ではアームが要素を持ちます。

```ruby
if let some(name) = App.user {
  Text { text: "hello #{name}" }
  Button { text: "sign out"; onClick: App.signOut() }
} else {
  Text { text: "not signed in" }
}
```

---

## 5. メモリ管理

オブジェクトは**自動参照カウント(ARC)**、値は**COW**です。
手で解放するものはなく、裏でヒープを走査するものもなく、
借用検査も表に出てきません。
二本柱と、何が回収され何が回収されないかの説明を1本目の後に置きます。

### 柱1: World とハンドル(オブジェクト)

クラスのインスタンスはすべて **World**(世代番号付きスロットマップ)
に住みます。手元に残るのは **`Handle<T>`** — ただの
(インデックス, 世代番号) の Copy 値です。

```
        World(プロセスに1つ、メインスレッド専有)
        ┌───────────────────────────────┐
        │ slot 0: gen=2 │ Counter {...}  │◀─┐
        │ slot 1: gen=0 │ Session {...}  │  │
        │ slot 2: gen=5 │ (空き)         │  │
        └───────────────────────────────┘  │
                                            │
   Handle<Counter> { ix: 0, gen: 2 } ───────┘
   (Copy。クロージャにも自由に握らせられる)

   アクセス:  handle.count(w)      ← World 経由で読む
   破棄後:    世代番号が合わない → 安全に「死んでる」と分かる
              (ダングリングポインタが構造的に存在しない)
```

- 参照を文をまたいで持つことはありません。
  読み書きは毎回「World → 値 → World」と往復するので、
  借用が衝突する状況そのものが起きません
- クロージャ(イベントハンドラ)が捕まえられるのは
  **Copy ハンドルと値だけ**(捕獲規則)。これが
  「UI コールバックの寿命問題」を消しています

### 回収されるもの、されないもの

メソッドが作り、どこにも渡さなかったオブジェクトは、
**スコープの終わりで回収されます**。
どこにも渡っていないことをコンパイラが確かめられるので、
スロットを解放し、次の確保がそれを再利用します。
ループが作る一時オブジェクトは、プログラム上の決まった地点で片づきます。

```ruby
for i in 0..1000000 {
  let n = Node()      # 100万回作って100万回解放される
  n.v = i             # スロットは1つ
}
```

返す、別のオブジェクトに入れる、リストに push する、何かに渡す、
別名に束ね直す。このどれか1つでもすると、その扱いは変わります。
そのオブジェクトはもう他の誰かが持つものになり、次の規則に移ります。
渡っていないと確かめられないときは、コンパイラは渡ったものとして扱います。
安全側に倒す判断です。

**保持された**オブジェクト、つまり store のプロパティや
別のオブジェクトのプロパティやリストに入っているものは、
最後の参照が消えた時点で解放されます。
そのオブジェクトが持っていたものも一緒に解放されます。

```ruby
Doc.rows = []       # 古い行が消え、その子も一緒に消える
```

解放は、最後の参照が消えるその書き込みの時点で起きます。
待たされる瞬間はありません。
**例外は閉路です。** 互いを指し合う2つのオブジェクトは、
互いを生かし続けます。片方向を `weak` にすれば断ち切れます。

```ruby
class Node {
  pub prop kids : List<Node>, default: []
  pub weak prop parent : List<Node>, default: []   # 親への逆参照
}
```

`weak` は参照先を生かしません。
参照先が消えたあとに読んでも安全で、消えたことは判別できます。
ハンドルは、指している先がまだ在るかどうかを常に知っています。

リストプロパティへの追加は1操作です。
そのプロパティがどこにあっても同じで、
このオブジェクトのものでも、store のものでも、
別のオブジェクト経由で辿ったものでも書けます。

```ruby
top.kids.push(kid)      # オブジェクト経由
Doc.rows.push(row)      # store 経由
```

view が持つオブジェクトは、view が持っている限り生きています。
store に渡してもリストに入れてもよく、そのリストが手放しても無事です。

```ruby
view Main {
  let mine = Tally()
  Column {
    Button { text: "stash"; onClick: Bin.take(mine) }
    Button { text: "bump";  onClick: mine.bump() }   # Bin が手放した後も使える
  }
}
```

行シート(行ごとのコンポーネント状態)だけは設計として grow-only なので、
一度100万行に達したリストは100万個の行オブジェクトを保持し続けます。
大きなリストでは、選択状態は行ごとの状態ではなく
store のインデックスで持ってください。

とはいえ既定として安いのは値のほうです。
store のプロパティは `List<なにかのstruct>` でも書き込みで通知を出すので、
リストも文書も、World オブジェクトを1つも作らずに
プロパティ単位のリアクティビティが手に入ります。

### 柱2: COW 値(データ)

`String / List / Map / Bytes` は **COW(copy-on-write)値**です。
代入・受け渡しは参照カウントの共有、書き込む瞬間だけ複製:

```
   let a = [1, 2, 3]      a ──┐
   let b = a                  ├──▶ [1, 2, 3]   (共有・コピーなし)
                          b ──┘

   b.push(4)              a ─────▶ [1, 2, 3]    (a は無傷)
                          b ─────▶ [1, 2, 3, 4] (ここで初めて複製)
```

値はコピーのつもりで雑に配ってよく、実コピーは書くときだけ起きます。
100万要素のリストを渡してもポインタが1つ増えるだけです。

---

## 6. クラス・store・リアクティビティ

### class — prop / init / signal

```ruby
pub class Counter {
  pub prop count : Int, default: 0     # 自動で countChanged 通知が付く

  pub fn increment {
    count += 1        # 裸の prop 名 = セッター経由 = 自動通知
  }
}

pub class Tag {
  pub prop label : String              # default なし = init が必ず代入
  pub prop weight : Int, default: 1

  init(l: String, extra: Int) {        # コンストラクタ(1個まで)
    label = l
    weight = weight + extra            # init 本文は World 非依存
  }
}
# 構築: Tag("hi", 4)
```

### メンバーの種類

```ruby
pub class Person {
  pub prop first : String, default: "Ada"   # 観測される表面
  pub prop last : String, default: "L"

  pub let id : Int                          # init で決まり、以後変わらない
  pub var seen : Int = 0                    # ふつうの可変フィールド

  # 導出プロパティ。何も保持せず、読まれるたびに評価され、常に最新。
  pub prop full : String, bind { first + " " + last }

  init(n: Int) { id = n }

  pub fn greet String {
    seen += 1
    "hello #{full}"
  }

  # 最後の参照が消えるときに走ります。この時点ではまだ自分自身を読めます。
  deinit {
    Log.note("bye #{full}")
  }
}
```

`let` はコンパイラが守ります。
`init` 以外の場所で `id` に代入すると、その旨を述べるエラーになります。
導出プロパティへの代入もエラーです（書き込む先がないため、読んでいる側に代入します）。

プロパティにはどの値型でも置けます。
`List<T>`、`Map<K, V>`、`T?`、`Bytes`、`struct`、そして別のオブジェクト。

```ruby
struct Row {
  var name : String
  var score : Int = 0        # 構築側が省略できるデフォルト
  var note : String? = nil
}

store Sheet {
  state rows : List<Row> = []
  state tally : Map<String, Int> = {}
  state picked : String? = nil
  state raw : Bytes = []                  # [] が空のバイト列
}
# Row("ada") と書けば score と note はデフォルトで埋まる
```

view の中では、map の `keys` と `values` はリピータが回せるリストで、
`m[k]` は `T?` を返し、値のないオプショナルは何も出力しません。

```ruby
for k in Sheet.tally.keys {
  Text { text: "#{k} = #{Sheet.tally[k]}" }
}
for r in Sheet.rows {
  Text { text: "#{r.name} #{r.score} [#{r.note}]" }
}
```

メソッドの中の **`this`** は、そのメソッドが走っている対象のオブジェクトです。
誰かに渡すことも、返すこともできます。

```ruby
pub fn adopt(k: Node) {
  kids.push(k)
  k.attach(this)        # 子が親を名指しできるようになる
}

pub fn me Node { this }
```

### class と struct のどちらを使うか

|  | `class` | `struct` |
|---|---|---|
| 居場所 | World(`Handle` 経由で触る) | どこでもない。値そのもの |
| 代入 | 同じ実体を共有 | コピー |
| 変更通知 | どのフィールドへの書き込みでも自動 | なし |
| 構築 | `init`(1クラス1つ) | 位置引数 |
| 回収 | されない(§5) | ふつうの値と同じ |

判断は2つの問いで決まります。

1. 変化を誰かが**観測**する必要があるか。あるいは2箇所が**同じ実体**を掴んで、
   片方の書き込みをもう片方が見る必要があるか → `class`。
   prop とシグナルとハンドルの同一性は、そのためだけにあります
2. それ以外 → `struct`。安く、コピーされ、World に入りません

実際のアプリデータはたいてい2番です。struct は自由に入れ子にできます。
struct の中の struct、`List<Struct>` のフィールド、再帰 struct(つまり木)、
いずれも動きます。

```ruby
struct Node {
  var v: Int
  var kids: List<Node>
}
```

class も入れ子にできます。そしてここが class の存在理由です。
class 型のフィールドは**参照**を持つので、2つの持ち主が同じ1つの
オブジェクトを指し、片方からの書き込みがもう片方に見えます。
値では表現できないのはこれだけです。

```ruby
class Tag  { pub prop weight : Int, default: 0 }
class Note {
  pub prop tag : Tag                 # コピーではなくハンドル
  init(t: Tag) { tag = t }
}

let t = Tag()
let a = Note(t)
let b = Note(t)
a.tag.weight = 3
b.tag.weight                         # => 3。同じオブジェクト
```

`static fn` はインスタンスではなくクラスに属します。
レシーバも状態も持たず、クラス名から呼びます。

```ruby
class Temp {
  pub prop celsius : Float, default: -40.0

  pub static fn fromF(f: Float) Float {
    (f - 32.0) * 5.0 / 9.0
  }
}

Temp.fromF(212.0)     # => 100.0
```

### store — プロセス唯一のシングルトン

```ruby
store App {
  state user : String = ""
  state theme : String = "dark"
  state session : Session = Session("guest")   # store がオブジェクトを持てる

  fn login(u: String) { user = u }
}
# どこからでも App.user / App.login(u) / App.session.token
```

view は連鎖の先まで読めます。
`App.session.token` への書き込みは、store 自体が変わっていなくても view に届きます。

### リアクティブループ

```
   クリック
      │
      ▼
 ┌─ メソッド実行 ──────────────────────────────┐
 │   count += 1                               │
 │      │ セッターが notify をキューに積む      │
 │      ▼                                     │
 │   flush ── 購読リスナーへ配送(遅延・再入なし)│
 │      │                                     │
 │      ▼                                     │
 │   dirty view にマーク                       │
 └────────────────────────────────────────────┘
      │
      ▼
   view の build() 再実行 → 新しい Element ツリー → gpui が再描画
```

バインディングは**単方向**です。view は状態を読むだけ、
書き込みは必ずメソッド経由。データの流れが一目で追えます。

書き込みは自動的にまとめられます。
メソッドが返るまで再構築は起きず、同じプロパティに3回書いても通知は1回です。

---

## 7. view とスタイル

### 18 ウィジェット

Column / Row / Grid / Stack / Text / Button / TextField(IME 対応)/
ListView(仮想化オプション付き)/ ScrollView / HScrollView /
Image / Svg / DataTable / Modal / BarChart / LineChart /
ProgressBar / Spinner。

```ruby
view Main {
  let items = Basket<String>()      # view 所有のオブジェクト
  state note : String = ""          # view ローカルのリアクティブセル

  Column {
    Text { text: "count: #{items.items.length}" }
    if App.theme == "dark" {        # 条件レンダリング
      Text { text: "dark side" }
    }
    ListView {
      virtualized: true             # 10万行でも可視分(約14行)しか作らない
      itemHeight: 24.0
      for x in items.items {        # リピータ
        Text { text: x }
      }
    }
  }
}
```

`for` の本体と `if` の分岐は、書いた数だけ要素を持てます。
互いに入れ子にもできます。
リピータが回す先は名前で辿れるリストなら何でもよく、行そのものが持つリストも回せます。
表はこの形になります。

```ruby
for row in App.rows {
  Text { text: row.name }
  if row.flagged {
    Text { text: "!" }
  }
  for cell in row.cells {
    Text { text: cell }
  }
}
```

例外は `virtualized:` のリストです。
これは1行につき1要素を必要な分だけ作る仕組みなので、`for` の本体はちょうど1要素です
（複数並べたいときは `Column` で包みます）。

3つ以上に分けたいときは `case` を使います。
対象はオプショナルか enum です。

```ruby
case App.mode {
  when idle { Text { text: "waiting" } }
  when busy {
    Text { text: "working" }
    ProgressBar { value: App.pct }
  }
  when _ { Text { text: "done" } }
}
```

アームが名指ししていないバリアントは何も出力しません。
view の構築は途中で失敗しない、という規則がここにも効いています。

`Image` と `Svg` はデコード結果を1度だけ持ち、上限つきのキャッシュを共有します。
長いジャケット一覧をスクロールしても、通り過ぎた分を全部抱えたままにはなりません。
上限はデコード済みピクセルで 256 MB、`PIXIE_IMAGE_BUDGET_MB` で変更できます。

### Grid

`Column` と `Row` は一方向に積むだけですが、`Grid` は均等なトラックを
順に埋め、行が埋まると自分で次の行へ折り返します。

```ruby
Grid {
  columns: 4        # 列は常に均等
  rows: 5           # 行も均等に。書かないと行は内容の高さになります
  spacing: 8.0      # 縦横どちらにも効く間隔
  Button { text: "7"; onClick: Pad.press("7") }
  # ... 残り 14 キー ...
  Button { text: "0"; colSpan: 2; onClick: Pad.press("0") }
}
```

- 要素は入ったセルを埋めます(`grow:` は要りません)
- `colSpan:` / `rowSpan:` は複数トラックにまたがらせる指定で、
  **どの要素にも**書けます(Column でも、チャートでも、コンポーネントの
  Button でも)。要素自身の性質ではなく、親グリッドでの置き方だからです
- トラックが均等なのは実装上の制約です。エンジンのテンプレートは
  `repeat(n, minmax(0, 1fr))` なので、列ごとに幅を変える指定
  (`100px 1fr auto`)は書けません。幅の違う列が必要なら、これまでどおり
  `grow:` 付きの `Row` を使います
- `examples/calcgrid` は `examples/calc` のキーパッドを Grid ひとつに
  書き直したものです。見た目は同じまま、`Row` 5 つと幅の計算が消えます

### ハンドラ

`onClick:`(や `onTextChanged:` / `onSubmitted:`)の本体は、
メソッド本体と同じ文が書けます。制御フロー、ローカル、
その場で作るオブジェクトです。

```ruby
Button {
  text: "go"
  onClick: {
    var i = 0
    while i < 10 {
      i = i + 1
      if i > 3 { break }
    }
    for k in 0..i {
      Board.note("k#{k}")
    }

    let c = Chip("a")     # ここで作る
    c.hits = 2
    c.bump()
    Board.keep(c)         # store に渡す
  }
}
```

呼び出しが1つだけならブロックは不要です: `onClick: Board.reset()`。
裸の `return` はハンドラを途中で抜けます。ループの中からでも抜けます。

```ruby
onClick: {
  if Board.locked { return }
  Board.commit()
}
```

ハンドラにできて、周りの view 本体にできないことが1つあります。
**メソッドの呼び出し**です。view の構築は状態を読むだけで、
それが再構築を安全にしています。view 本体はプロパティを読み、
何かを変えるのはハンドラの役目です。

### カスタムコンポーネント — 再利用できるステートフルビュー

`Main` 以外のすべての `view` は**コンポーネント**です: パラメータを
取り、要素として使え、インスタンスごとの状態を持ちます。
解決はすべてコンパイル時です。使用箇所にインライン展開されるので
エンジンの語彙は増えず、両実行ティアが同一に展開します。

```ruby
view Counter(label: String, step: Int) {
  state n : Int = 0                  # ← 使用箇所ごとに独立のセル

  Row {
    Text { text: "#{label}: #{n}" }
    Button { text: "+#{step}"; onClick: { n = n + step } }
  }
}

view Card(title: String) {
  Column {
    Text { fontSize: 18.0; text: title }
    Slot { }                         # ← 使用側の子要素がここに入る
  }
}

view Main {
  Column {
    Card {
      title: "counters"
      Counter { label: "a"; step: 1 }
      Counter { label: "b"; step: 10 }   # "a" とは独立
    }
  }
}
```

- 使用側のプロパティが宣言パラメータに束縛されます(デフォルト値可)
- コンポーネント内の `state` / `let` はインスタンスごとにホイスト —
  ステートフルな使用箇所の増減はリビルド、本体編集は他の
  ビュー編集同様ホットリロード
- `Slot { }` は1コンポーネントに1つ。再帰はコンパイルエラー
- **行ごとの状態**: `for` リピータ内のステートフルコンポーネントは
  **行ごとに独立した状態**を持ちます。キーは位置なので、リストが
  縮んで再び伸びると、元の行の状態がそのまま戻ります(作り直しには
  なりません)。リピータの深さは問わず、`virtualized:` リストの中でも
  持てます。行ごとの `let` オブジェクトだけが未対応です
- **クロスモジュール**: `pub view` はモジュールを越えます —
  修飾(`ui.Card { }`)、エイリアス(`use ui as U` → `U.Card`)、
  選択(`use ui.{Card as MyCard}`)。pub コンポーネントの本体は
  自モジュールのビュー(private 兄弟含む)で解決されます

### スタイルとテーマ

```ruby
style Key {
  background: "#313244"
  hover.background: "#45475a"       # 擬似状態はドット付きキー
}
style Hot { background: "#fab387" }
style KeyOp = Key + Hot             # 右勝ちマージ

view Main {
  Column {
    style: Pad                      # 適用はプロパティとして
    Button { style: KeyOp; text: "÷"; onClick: ... }
    Text { color: "accent" }        # 色はテーマトークン名でも書ける
  }
}
```

箱を描く要素は、角の丸み、枠の太さ、枠の色も取れます。

```ruby
style Card {
  padding: 10.0
  background: "panel"
  borderRadius: 10.0
  borderWidth: 1.0
  borderColor: "accent"     # トークンなので、パレットの切り替えに追従します
}
```

- スタイルはコンパイル時に**完全インライン**(実行時コストゼロ)
- スタイル名は、それを**書いたモジュール**で解決します。
  だから公開したコンポーネントは、`pub` を付けていないスタイルも
  含めて自分のスタイルを連れて動きます。
  `pub` を付けるかどうかは「他のモジュールがその名前を**書けるか**」の話で、
  そこまで届くかどうかとは別の問いです
- 色トークン(`"accent"` / `"panel"` など)はダーク/ライトテーマに
  追従。`PIXIE_THEME=light` で起動、実行中は Cmd+T で切替
- `theme:` はパレットを部分木に限定します。暗いウィンドウの中に
  明るいパネルを1枚置けます。値は式なので、アプリ自身のテーマを
  ふつうの状態として持てます

```ruby
store App { state mode : String = "dark"
  fn light { mode = "light" } }

view Main {
  Column {
    theme: App.mode           # アプリの切替ボタンがここを書く
    grow: 1.0
    background: "windowBg"
    Button { text: "light"; onClick: App.light() }
    Column {
      theme: "light"          # 部分木は自分のパレットに固定できる
      background: "panel"
      Text { color: "text" }
    }
  }
}
```

- トークンは要素ツリー上で、リビルドごとに1度だけ解決されます。
  そのため `PIXIE_SCRIPT="theme:light"` はライトのツリーを出力し、
  `animate:` の付いた要素はパレット切替時にクロスフェードします
- スタイルの編集は view 本体と同じく**約1msでホットリロード**。
  別モジュールの `pub style` も同じです

### アニメーション

アニメーションは、値が動く要素そのものに宣言します。
値を動かす更新のほうを囲むのではありません。

```ruby
Button {
  text: "box"
  width: Panel.boxWidth             # この値が変わると補間される
  background: Panel.boxColor
  animate: 300.0                    # ミリ秒。これがスイッチ
  easing: "linear"                  # linear | in | out(既定) | inOut
  onClick: Panel.narrow()
}

if Panel.openOn {
  Text { text: "hello"; animate: 200.0; enter: true; exit: true }
}
```

- 有効にするのは `animate:` です。`easing:` / `enter:` / `exit:` を
  `animate:` なしで書くと、黙って無視されるのではなく名前付きエラーになります
- 4つとも式を取ります。曲線もフェードの有無も状態として持てるので、
  アプリ側で切り替えを用意できます(`easing: App.curve`、`exit: App.fades`)
- `enter:` は初めて現れたときのフェードイン、`exit:` は view が
  出力をやめたあとも描画を保持してからフェードアウトします。
  この保持があるので `if` ブロックが消えるときも間が飛びません
- 数値はそのまま補間されます。色は両端が16進リテラルのときだけ補間され、
  テーマトークンはエンジン側で解決されるため即座に切り替わります
- 補間は描画側ではなく要素ツリー上で走るので、ヘッドレス実行からも見えます。
  `PIXIE_SCRIPT` の `advance:<ms>` は時計をその瞬間に立たせます

```sh
PIXIE_SCRIPT="click:show,advance:100" ./app   # フェード途中の1フレームを出力
```

- 時間に触れないスクリプトは、dump の前にすべての補間を着地させます。
  アニメーションがスクリプトの意味を変えることはありません
- 視差効果を減らす設定が有効なときは、すべての duration が 0 になります

### アクセシビリティ

アクセシビリティツリーの大半は導出できるので、pixie は導出します。
Button はラベルを名前とする button、TextField はプレースホルダを名前とし
入力内容を値とする textInput、ProgressBar は数値を読み上げます。
レイアウト用のコンテナは何も報告せず、子を上へ渡します。
「グループ、グループ、グループ」と読み上げられるより、黙っているほうがましだからです。

導出できないものは、2つの rider で補います。

```ruby
Text { text: Doc.title; fontSize: 22.0; role: "heading" }

Row {
  role: "group"
  label: "toolbar"
  Svg { source: "save.svg"; label: "Save" }   # 代替テキスト
  Button { text: "save"; onClick: Doc.save() }
}
```

- `role:` の語彙は閉じています(`button` / `label` / `heading` /
  `textInput` / `image` / `list` / `listItem` / `table` / `dialog` /
  `progress` / `group`)。リテラルならビルド時に検査され、
  式も書けるので、行のデータ次第で見出しにも項目にもできます。
  実行時に知らない名前が来たときは、その要素が自分で導く役割に戻ります
- `label:` は任意の文字列式なので、代替テキストに補間も書けます
- ツリーは要素ツリー上で計算されるため、スクリプトから出力できます

```sh
PIXIE_SCRIPT="a11y,click:open,a11y" ./app
# group[label "...", button "open"]
# group[label "...", button "open", dialog[label "Leave a note", ...]]
```

---

## 8. エラー処理と `T?`

```ruby
error MathError {
  divByZero
  negative(v: Int)
}

fn safeDiv(a: Int, b: Int) !Int {      # !T = 失敗しうる
  if b == 0 {
    return MathError.divByZero         # エラーを返す
  }
  a / b
}

fn divideTwice(a: Int, b: Int) !Int {
  let once = try safeDiv(a, b)         # try = エラーはそのまま伝播
  try safeDiv(once, b)
}

case safeDiv(1, 0) {
  when ok(v) { ... }
  when err(e) { ... }                  # e はエラー enum。case で更に分解可
}
```

「失敗しうる」は型に出る(`!Int`)、握りつぶしは書けない
(`case` は ok と err の両腕が要る)、が方針です。
`T?`(§4)は「失敗ではなく不在」を表す別の道具です。

---

## 9. async と HTTP

```
     メインスレッド                     ワーカープール(gpui)
 ┌────────────────────┐             ┌──────────────────┐
 │  World + UI ループ  │   await     │ ブロッキング処理    │
 │                    │────────────▶│  fs / http / ...  │
 │  async fn の本文は  │             └──────────────────┘
 │  16ms ごとに再開    │◀────────────  完了キュー(Completion)
 └────────────────────┘   結果+変換
```

```ruby
store Net {
  state body : String = ""

  async fn hit {
    case await Http.get("https://example.com/") {
      when ok(b) { body = b }
      when err(e) { body = "failed: #{e}" }
    }
  }
}
```

- `await` できるのはバインディング呼び出し。実行はワーカーへ、
  結果はメインスレッドに戻ってから pixie の値に変換されます。
  非同期でできるのは今のところここまでで、`async fn` は値を返せず、
  別の `async fn` を `await` することもできません
- ランタイムは1つ(gpui のプール)。tokio 等の第二ランタイムは
  持ち込みません
- HTTP クライアントは組み込み:
  `Http.get / getBytes(→Bytes) / post / getWith(url, headers)` —
  ヘッダは `Map<String, String>`
- ウィンドウ実行とヘッドレス実行で**実行セマンティクスは同一**
  (ヘッドレスは決定的に「静まるまで」待つ)

---

## 10. Rust バインディング

**crates.io が標準ライブラリ**です。`.rpi` ファイルが Rust crate の
表面を宣言し、呼び出し点でアダプタが型を変換します。

```ruby
# fs.rpi(手書きするならこれだけ)
class Fs {
  static fn writeString(path: String, contents: String) !Void @rust("std::fs::write")
  static fn read(path: String) !Bytes @rust("std::fs::read")
}
```

手書きしなくても **rpi-gen** が rustdoc JSON から導出します:

```
 Rust crate ──▶ rustdoc JSON ──▶ rpi-gen ──▶ .rpi
                                   │
                                   └─ 束縛できない関数は
                                      「理由付きでスキップ」報告
```

アダプタの型対応(抜粋)。引数と戻り値の両方です。

| Rust 側 | pixie 側 |
|---|---|
| `&str` / `String` / `PathBuf` | `String` |
| `i64`(戻り値なら他の幅も拡張) | `Int` |
| `Vec<T>` | `List<T>` |
| `Vec<u8>` / `&[u8]` | `Bytes` |
| `Option<T>` | `T?` |
| `Result<T, E>` | `!T`(戻り値のみ) |
| kernel の `Map<K, V>` | `Map<K, V>` |
| C ライクな `enum` | rpi-gen が宣言する `enum`(下記) |
| `struct`(タプル struct も) | rpi-gen が宣言する `struct`(下記) |

**戻り値のほうが引数より緩い**点に注意してください。
戻り値はどの整数幅も `PathBuf` も広がりますが、
引数は pixie 側の型が持つ Rust の型(`i64`、`String`)で受け取ります。
struct のフィールドは書き戻す先の型を名乗れるので、どちらにせよ両方向に渡ります。

### enum は `.rpi` に対応を書けば渡ります

pixie の `enum` と Rust の `enum` は別の型です。
推測させずに、`.rpi` に対応を書きます。
**この宣言は rpi-gen が出力します。**
束縛するモジュールにある公開の C ライク enum を、すべて宣言します。

```ruby
enum PathKind @rust("pixie_kernel::PathKind") {
  Missing
  File
  Dir
}

class Kernel {
  static fn pathKind(path: String) PathKind @rust("pixie_kernel::path_kind")
  static fn kindName(kind: PathKind) String @rust("pixie_kernel::kind_name")
}
```

`@rust` を書かなかったバリアントは自分の名前を使います。
生成された形の属性が1つで済んでいるのはそのためです。
pixie 側で別の綴りにしたいときだけ `dir @rust("Dir")` と書きます。

返ってきた値はふつうの pixie の値で、`case` でそのまま照合できます。

```ruby
case Kernel.pathKind("/tmp") {
  when Dir { note = "ディレクトリ" }
  when _ { note = "それ以外" }
}
```

限界が2つあり、どちらも名前付きエラーになります。
**ペイロードを持つバリアント**は対応づけられません。
変換はバリアントを1対1で照合するので、
ペイロードを関係づけるにはそのフィールドまで関係づける必要があるからです
（そのフィールドを渡してください）。
もう1つは、属性なしで名前だけを頼りに対応づける案を採らなかったことです。
Rust 側でバリアント名が変わったときに黙って壊れるからで、
バインディングは黙って壊れるといちばん困る場所です。

### struct も同じ形で渡ります

`struct` には対応する Rust の struct を書き、
各フィールドには対応する Rust のフィールドを書きます。
名前だけでは足りないときは、Rust 側の型も書きます。

```ruby
struct FileStat @rust("pixie_kernel::FileStat") {
  var len : Int @rust("len: u64")
  var readonly : Bool
}

class Kernel {
  static fn fileStat(path: String) FileStat @rust("pixie_kernel::file_stat")
  static fn statLine(stat: FileStat) String @rust("pixie_kernel::stat_line")
}
```

`readonly` に属性は要りません。
名前も型も両側で一致しているからです。
`len` に要るのは Rust 側が `u64` だからで、理由は下で説明します。
この宣言は rpi-gen が出力します。
フィールド名はキャメルケースに直し、
属性は何かが食い違うところにだけ付きます。

返ってきた値はふつうの pixie の値で、
pixie 側で組み立てた値はそのまま Rust 側へ戻ります。

```ruby
let s = Kernel.fileStat("notes.txt")
size = s.len
line = Kernel.statLine(FileStat(1024, true))
```

フィールドが渡れるかどうかの規則は、値全体のそれと同じです。
struct は struct や enum、リスト、オプショナルを持てますし、
`List<FileStat>` や `FileStat?` は要素ごとに渡ります。

```ruby
struct Entry @rust("pixie_kernel::Entry") {
  var name : String
  var kind : PathKind
  var stat : FileStat
}

class Kernel {
  static fn dirStats(path: String) List<Entry> @rust("pixie_kernel::dir_stats")
  static fn statTotal(entries: List<Entry>, only: PathKind?) Int @rust("pixie_kernel::stat_total")
}
```

`Entry` のフィールドはどれも型を書いていません。
そのまま書き戻せば Rust 側が期待する型になるからです。

**`len` に型が要る理由。**
数値はこちらへ来るときに広がります(`Int` がどの整数幅も吸収します)。
戻すときは幅をぴったり合わせる必要があり、それを知っているのは `.rpi` だけです。
属性は Rust のフィールド宣言と同じ形に読めます。
名前だけか、名前と型かです。
文字列を `PathBuf` に書き戻すときも同じ書き方です。
幅のキャストは巻き戻るので、負の `Int` を `u64` に書くと巨大な値になります。
幅を書いたのは `.rpi` の側であり、変換はそれに従います。

**タプル struct** は位置で対応します。
pixie 側が名前を付け、Rust 側は `.0` で届き、rpi-gen が両方を書きます。

```ruby
struct Perms @rust("pixie_kernel::Perms") {
  var value : Int @rust("0: u32")
}
```

渡れないフィールドが1つでもあると struct 全体が渡れなくなり、
エラーはそのフィールドを名指しします。
rpi-gen も同じ struct を理由付きでスキップします。
理由は非公開フィールド(pixie 側から埋められません)、
対応の付かない型のフィールド、
それに要素の側が独自の Rust 型を必要とするフィールドです。
属性が書けるのは型1つなので、`Vec<u32>` のフィールドには書く場所がありません。

---

## 11. モジュールとパッケージ

### モジュール

ファイル = モジュール、パスはディレクトリを鏡写し:

```ruby
use model                  # 兄弟ファイル model.pix(pub 項目が見える)
use ui.card                # ui/card.pix。card.cardTitle(..) と修飾も可
use model as m             # エイリアス: m.decorate(..)
use model.{decorate}       # 選択インポート
use model.{decorate as d}  # リネーム付き
pub use ui.card.{cardTitle} # 再エクスポート(パッケージの顔を作る)
```

同名アイテムがモジュール間にあっても共存できます(内部で
モジュール別にマングリング)。曖昧な裸参照は両出所を名指しで
エラーになるので、修飾するか選択インポートで解けます。

### パッケージ — pixie.toml

```toml
[package]
name = "myapp"
version = "0.1.0"

[crates]                    # Rust crate を「そのまま」依存に
serde_json = "1"
mathkit = { path = "vendor/mathkit" }

[dependencies]              # pixie パッケージ
ui-kit = { git = "https://…", tag = "v0.2" }
strkit = { path = "packages/strkit" }
kit = "1"                   # ← レジストリ経由(下図)

[registry]
index = "https://…/index"   # <name>.toml を置いた静的インデックス
```

```
                ビルド時の流れ
 pixie.toml
   │  [crates] serde_json = "1"
   │     │
   │     ├─▶ rustdoc JSON ─▶ rpi-gen ─▶ .pixie/rpi/serde_json.rpi
   │     │                     (キャッシュ。コミット推奨 —
   │     │                      共同開発者は nightly 不要)
   │     └─▶ 生成 Cargo.toml に依存として注入
   │             └─▶ バージョン解決とロックは cargo 自身
   │
   │  [dependencies] kit = "1"
   │     └─▶ インデックスで解決 ─▶ git 取得 ─▶ pixie.lock に rev 固定
   │            (ロック済みなら完全オフライン)
   ▼
 pixie build   (プロジェクト直下なら引数なしで src/main.pix)
```

- pixie は **semver ソルバを持ちません** — Rust 依存の解決は
  cargo、pixie パッケージは lock の rev が正
- 依存パッケージの `pub style` もスタイル展開に乗ります

この管理には CLI コマンドがあり、手編集は不要です:

```sh
pixie new my_app                    # pixie.toml + src/main.pix の足場生成
pixie add kit --git https://…       # pixie 依存: fetch + pixie.lock へピン
pixie add kit 1                     # …レジストリインデックス経由
pixie add serde_json 1 --crate      # Rust クレート: cargo 依存 + バインディング導出
pixie update [kit]                  # ピンを解いて再解決、old → new rev を報告
pixie remove kit                    # エントリ + lock ピン + キャッシュを掃除
```

`add` は即座に同期し、fetch や導出が失敗したら**ロールバック**します
— URL の打ち間違いでプロジェクトが壊れたままになることはありません。

---

## 12. 二層実行とホットリロード

同じ `.pix` に実行係が二人います:

```
            ┌── ティア1: AOT コンパイル(本番の姿)
 .pix ──────┤
            └── ティア2: view スライスインタプリタ
                (実行中バイナリが自分の view 本体を
                 再パースして生きた World に再構築 ≈ 1ms)

 常設の分岐ゲート:
   PIXIE_SCRIPT で同じ操作列を両ティアに流し、
   出力ツリーが 1 バイトでも違えばテスト失敗
   (31 デモで常時運転)
```

- `pixie watch` は保存ごとに指紋(fingerprint)で判定:
  view 本体・スタイルの編集 → ティア2で**約1ms**、
  それ以外 → 再ビルド(約0.5秒)。
  インポート先も見ているので、別モジュールの `pub style` や
  公開コンポーネントの本体も、編集中のファイルと同じように
  その場で反映されます
- `pixie build --release` は**ティア2を丸ごと剥がします**:
  リロード機構もインタプリタ依存も生成物から消え、
  counter デモで 60MB → 13MB。挙動はバイト単位で同一

---

## 13. CLI 早見表

```sh
pixie build app.pix --run        # ビルドして起動
pixie build --release            # AOT 専用・最適化ビルド
pixie build                      # pixie.toml があれば src/main.pix
pixie check app.pix              # 型検査のみ
pixie test values.pix            # TAP テストランナー
pixie fmt app.pix [--check]      # フォーマッタ
pixie watch app.pix              # ホットリロード監視
pixie install-runtime            # マシンごとに一度(gpui を事前ビルド)
pixie new my_app                 # プロジェクトの足場生成
pixie add kit --git URL          # 依存追加(--crate = Rust クレート)
pixie update [kit]               # pixie 依存を再解決、lock を更新
pixie remove kit                 # 依存 + キャッシュを削除

PIXIE_SCRIPT="click:go,input:hi" ./app     # ヘッドレス操作再生
PIXIE_SCRIPT="click:go,dump,click:go" ./app # 途中の画面も出力
PIXIE_SCRIPT="click:go,advance:100" ./app  # 100ms 進めた地点で出力
PIXIE_SCRIPT="a11y" ./app                  # アクセシビリティツリーを出力
PIXIE_SCRIPT="theme:light" ./app           # ルートのパレットを切替
PIXIE_SCRIPT="mem" ./app                   # 生存オブジェクト数を出力
PIXIE_TIER=interp PIXIE_SCRIPT=... ./app   # ティア2で同じ操作
PIXIE_THEME=light ./app                    # ライトテーマ起動
```

---

## 14. 今できないこと

できないことの一覧です。すべて**名前付きエラー**で塞がれており、
黙って壊れるものはありません。

- **コンポーネント**: 行ごとの `let` オブジェクト、他モジュールの
  コンポーネント本体に書いた修飾参照(`ui.Card`)。コンポーネント自体は
  モジュールを越えられますし、行ごとの状態はリピータの深さを問わず、
  `virtualized:` リストの中でも持てます(§7、いずれもゲート済み)
- `virtualized:` リストの行の中でのアニメーション(補間パスは遅延行を
  意図的に展開しません)、チャートのデータ差し替え時の補間、
  フェード以外のトランジション
- class 型の struct フィールド。これは穴ではなく**規則**です。
  struct は代入でコピーされ、コピーされた参照は1つのオブジェクトへの
  2本目の参照になります。持ち主を `class` にするか、id を持って
  引き直してください
- 独自パレットの定義(`theme:` が取るのは組み込みの2つです)と、
  トークン単位の上書き
- 役割・名前・値より先のアクセシビリティ。ラベルのない画像に対する
  ビルド時の警告、フォーカス順序の明示指定(Tab リングは文書順のまま)、
  AccessKit のアクションやライブリージョンはまだです
- Grid の均等トラック以外の指定。列ごとに幅を変えるトラック
  (`100px 1fr auto`)と明示配置(`colStart:` / `rowStart:`)はまだで、
  エンジンが公開しているのは均等トラックと span までです
- init のオーバーロード(1クラス1 init)
- 境界付きのクラス/struct 型パラメータ(`class Sorted<T: Comparable>`)。
  自由関数の型パラメータには境界を書けますが、クラスと struct はまだです
- ジェネリッククラスへの trait impl、ジェネリックな impl、
  ジェネリックな store
- ホットリロード中のハンドラからジェネリックメソッドを直接呼ぶこと
  (具象のラッパーメソッド越しなら呼べます)
- ビットフラグ(`flags Perms of Perm`)。型検査は形を知っていますが、
  まだ動きません
- 入れ子のパターン(`when err(bad(v))`)とリテラルとの照合(`when 42`)。
  `case` のアームが書けるのはバリアント名、`some` / `nil`、`_` です
- Rust 側がペイロード付き `enum` またはジェネリックな struct を
  取る、または返すバインディング。
  C ライクな `enum` と `struct` は渡ります。タプル struct も渡ります
  (rpi-gen が宣言を出力します)。
  それ以外も両方向に渡ります。数値、真偽値、文字列、バイト列、map、
  リスト、オプショナル、そして戻り値の `!T` です
- HTTP **サーバー**(構想は `service` ブロック。意図的に後回し)
- Linux / Windows(エンジンは macOS で検証中)
