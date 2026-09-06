# pixie 言語ツアー

pixie の言語全体を一通り見て回るガイドです。
ここに載っているコードは、どれも現在のツリーでそのままコンパイルして実行できます。
できないことは、最後の「今できないこと」にまとめてあります。
英語版は [TOUR.md](TOUR.md) です。

## 目次

1. [30秒の pixie](#1-30秒の-pixie)
2. [コンパイルの仕組み — 検証する仕組みは二つ](#2-コンパイルの仕組み)
3. [基本文法](#3-基本文法)
4. [型システム — trait、ジェネリクス、`T?`](#4-型システム)
5. [メモリ管理 — World とハンドル](#5-メモリ管理)
6. [クラス、store、リアクティビティ](#6-クラスstoreリアクティビティ)
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

- `store` はプロセスにただ一つあるリアクティブな状態の置き場
- `view` は宣言的な UI ツリーで、状態が変わると描き直される
- `async fn` と `await` は、ブロッキングする処理をワーカースレッドへ移す仕組み
- `Fs` は Rust の `std::fs::write` を包む 2 行のバインディング

実行するには次のようにします。

```sh
pixie build hello.pix --run     # ウィンドウが開く
pixie watch hello.pix           # 保存ごとに約1msでホットリロード
PIXIE_SCRIPT="input:Ada,click:save" ./hello   # ヘッドレスでUI操作を再生
```

---

## 2. コンパイルの仕組み

pixie の設計でいちばん大事な図がこれです。

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

pixie が書く Rust は、必ず借用検査を通ります。
だから所有権や借用を書き手が扱うことはありません。
その代わり、rustc が pixie の書いたプログラムを一つ残らず検証します。

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
`fontSize: 14`、`let ratio : Float = 3`、`30.0 * count` はどれもそのまま通ります。
一つの式に整数と小数が混ざったときは、式全体が広いほうの型になります。

補間には式と書式指定を書けます。

```ruby
"#{n * 2} of #{n + 1}"     # 算術
"#{v:.2f}"                 # 3.14
"#{n:>6}"   "#{n:04}"      # 幅、ゼロ埋め
"#{s:*^9}"                 # 中央寄せ、* で埋める
```

書式指定に書けるのは、幅、寄せ（`<` `^` `>`）、ゼロまたは任意の埋め文字、`.精度`、基数（`x` `X` `o` `b`）です。
末尾の型文字（`f` や `d`）は、書いても構いませんが無視されます。
それ以外を書くとコンパイルエラーになり、エラーメッセージがその指定を示します。

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

宣言した列挙体に対する `case` には**網羅性チェック**が付きます。
variant が抜けているとコンパイル**エラー**になり、足りない variant がメッセージに並びます。
`when _` がキャッチオールです。
演算子は `+ - * / %`、比較は `< <= > >= == !=`、論理は `&& || !` です。

### リストとマップの読み方

```ruby
xs[i]            # T   — 要素 i が無ければプログラムエラー
xs.get(i)        # T?  — 無ければ nil
xs.first()       # T?
m[k]             # V?  — マップは「無い」がふつうなので nil
xs.length        # Int
```

リストを取る場所には、リスト型の式ならなんでも書けます。
フィールドをたどった式もそのまま使えます（`for k in node.kids { ... }`、`bag.items.length`、`kept[0].tag.label`）。

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

`pixie test file.pix` を実行すると、テストが TAP 形式で走ります。
アサートは `assert_eq / assert_neq / assert_true / assert_false` です。

---

## 4. 型システム

型付けは**静的で名前的**です。
そのうえで、書いたプログラムはすべて rustc がもう一度検証します。

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

`trait` には「何ができるか」を書き、`impl` には「ある型がそれをどうやるか」を書きます。
World に住む `class` にも、値である `struct` にも `impl` を書けるので、型システムの両側で使えます。

```ruby
trait Labeled {
  fn tag String
}

class Dog    { pub prop name : String, default: "rex" }
struct Badge { var text: String }

impl Labeled for Dog   { fn tag String { "dog:#{name}" } }
impl Labeled for Badge { fn tag String { "badge:#{self.text}" } }
```

その trait を境界に指定した関数は、どちらの型も受け取れます。
コンパイラは呼び出しに使われた型ごとに専用の版を作るので、trait を経由した呼び出しも直接呼ぶのと同じコストで済みます。

```ruby
fn describe<T: Labeled>(x: T) String {
  "<#{x.tag()}>"
}

describe(Dog())            # => <dog:rex>
describe(Badge("hi"))      # => <badge:hi>
```

trait を実装していない型を渡すと、その呼び出しの場所でコンパイルエラーになります（`type Int does not implement trait Labeled`）。
trait を実装しても、その型がもとから持っているメソッドは変わりません。
上の `Badge` は、ほかに書いたメソッドをそのまま持ち続けます。

未対応のものが二つあり、どちらも名前付きエラーになります。
ジェネリッククラスへ trait を実装することと、クラスや struct の型パラメータに境界を付けることです。

### ジェネリクス — 単相化は rustc の仕事

pixie のジェネリクスは、**本物の Rust のジェネリクス**へそのままコンパイルされます。
pixie 側でコードを複製することも、コードサイズのために細工をすることもありません。

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

- クラスと struct の型パラメータには、今のところ**境界を付けられません**（付けると名前付きエラーになります）

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

どちらもメソッド、ハンドラ、view 本体のどこにでも書けます。
view の中では、それぞれのアームに要素を並べます。

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

オブジェクトは**自動参照カウント（ARC）**で、値は **COW** で管理します。
手で解放するものはありません。
裏でヒープを走査するものもなく、借用検査が表に出てくることもありません。
柱は二本あり、何が回収されて何が回収されないかは、一本目のあとで説明します。

### 柱1：World とハンドル（オブジェクト）

クラスのインスタンスはすべて **World**（世代番号付きスロットマップ）に住みます。
手元に残るのは **`Handle<T>`** で、インデックスと世代番号を組にしただけの Copy 値です。

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

- 参照が文をまたいで残ることはありません。
  読み書きのたびに World から値へ、値から World へと往復するので、借用が衝突する状況そのものが生まれません
- クロージャ（イベントハンドラ）が捕まえられるのは**Copy ハンドルと値だけ**です（捕獲規則）。
  よくある「UI コールバックの寿命問題」は、この規則によって起きなくなります

### 回収されるもの、されないもの

メソッドが作り、どこにも渡さなかったオブジェクトは、**スコープの終わりで回収されます**。
ほかのどこにも渡っていないことをコンパイラが確かめられるので、スロットはそこで解放され、次に確保するオブジェクトがそれを使います。
ループが作る一時オブジェクトもこれに当てはまり、片づく地点はプログラムの上で決まっています。

```ruby
for i in 0..1000000 {
  let n = Node()      # 100万回作って100万回解放される
  n.v = i             # スロットは1つ
}
```

返したり、別のオブジェクトに入れたり、リストに push したり、どこかに渡したり、別の名前に束縛したりすると、この扱いは変わります。
そのオブジェクトを持つのは別の場所になり、ここから先は次の規則が働きます。
外に出ていないと確かめきれないときは、コンパイラは安全な側に寄せて、出ていったものとして扱います。

store のプロパティ、別のオブジェクトのプロパティ、リストのどれかに**保持された**オブジェクトは、最後の参照が消えた時点で解放されます。
そのオブジェクトが持っていたものも、このとき一緒に解放されます。

```ruby
Doc.rows = []       # 古い行が消え、その子も一緒に消える
```

解放が起きるのは、最後の参照を消したその書き込みの時点です。
待たされる瞬間はありません。
**例外は参照の循環です。**
互いを指し合う二つのオブジェクトは、互いを生かし続けます。
片方の向きを `weak` にすれば、循環は切れます。

```ruby
class Node {
  pub prop kids : List<Node>, default: []
  pub weak prop parent : Node?, default: nil       # 親への逆参照
}
```

`weak` な参照は、参照先を生かし続けません。
参照先が消えたあとに読んでも安全で、消えたかどうかもその場で判別できます。
指しているオブジェクトがまだ在るかどうかは、ハンドルからいつでも分かるからです。

リストプロパティへの追加は、そのプロパティがどこにあっても一つの操作で済みます。
このオブジェクトのものでも、store のものでも、別のオブジェクトを辿った先のものでも同じように書けます。

```ruby
top.kids.push(kid)      # オブジェクト経由
Doc.rows.push(row)      # store 経由
```

view が持つオブジェクトは、その view が持っているあいだ生き続けます。
そのため、store に渡してもリストに入れてもよく、そのリストが手放したあとも無事です。

```ruby
view Main {
  let mine = Tally()
  Column {
    Button { text: "stash"; onClick: Bin.take(mine) }
    Button { text: "bump";  onClick: mine.bump() }   # Bin が手放した後も使える
  }
}
```

設計として grow-only なのは、行シート（行ごとのコンポーネント状態）だけです。
一度100万行に達したリストは、そのあとも100万個の行オブジェクトを持ち続けます。
大きなリストを扱うときは、選択状態を行ごとの状態ではなく store のインデックスとして持ってください。

とはいえ、既定として安いのは値のほうです。
`List<なにかのstruct>` 型の store プロパティも、書き込みのたびに通知を出します。
そのため、World オブジェクトを一つも作らないまま、リストにも文書にもプロパティ単位のリアクティビティを持たせられます。

### 柱2：COW 値（データ）

`String / List / Map / Bytes` は **COW（copy-on-write）値**です。
代入や受け渡しでは参照カウントを共有し、複製が起きるのは書き込む瞬間だけです。

```
   let a = [1, 2, 3]      a ──┐
   let b = a                  ├──▶ [1, 2, 3]   (共有・コピーなし)
                          b ──┘

   b.push(4)              a ─────▶ [1, 2, 3]    (a は無傷)
                          b ─────▶ [1, 2, 3, 4] (ここで初めて複製)
```

値はコピーのつもりで気軽に配ってかまいません。
100万要素のリストを渡してもポインタが一つ増えるだけで、実際に複製されるのは誰かが書き込んだときです。

---

## 6. クラス、store、リアクティビティ

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

`let` への代入はコンパイラが検査します。
`init` 以外の場所で `id` に代入すると、そこには書けないと述べるエラーが出ます。
導出プロパティへの代入もエラーです。
書き込む先がないので、その導出が読んでいるプロパティのほうに代入します。

プロパティはどの値型でも持てます。
`List<T>`、`Map<K, V>`、`T?`、`Bytes`、`struct`、それに別のオブジェクトです。

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

view の中では、map の `keys` と `values` はリピータで回せるリストになります。
`m[k]` は `T?` を返し、中身のないオプショナルは文字列に埋め込んでも何も出ません。

```ruby
for k in Sheet.tally.keys {
  Text { text: "#{k} = #{Sheet.tally[k]}" }
}
for r in Sheet.rows {
  Text { text: "#{r.name} #{r.score} [#{r.note}]" }
}
```

メソッドの中の **`this`** は、そのメソッドを実行しているオブジェクトです。
別のオブジェクトに渡すことも、戻り値にすることもできます。

```ruby
pub fn adopt(k: Node) {
  kids.push(k)
  k.attach(this)        # 子が親を参照できるようになる
}

pub fn me Node { this }
```

### class と struct のどちらを使うか

|  | `class` | `struct` |
|---|---|---|
| 居場所 | World（`Handle` 経由で触る） | どこでもない。値そのもの |
| 代入 | 同じ実体を共有 | コピー |
| 変更通知 | どのフィールドへの書き込みでも自動 | なし |
| 構築 | `init`（1クラスに1つ） | 位置引数 |
| 回収 | されない（§5） | ふつうの値と同じ |

判断は2つの問いで決まります。

1. 変化を誰かが**観測**する必要があるか。
   あるいは2つの持ち主が**同じ実体**を共有し、一方の書き込みがもう一方から見える必要があるか → `class`。
   prop とシグナルとハンドルの同一性は、そのためだけにあります。
2. それ以外 → `struct`。
   安く、コピーで渡り、World には入りません。

実際のアプリのデータは、たいてい2番です。
struct は自由に入れ子にできます。
struct の中の struct、`List<Struct>` のフィールド、再帰する struct（つまり木）、どれも書けます。

```ruby
struct Node {
  var v: Int
  var kids: List<Node>
}
```

class も入れ子にできます。
class を使う理由はここにあります。
class 型のフィールドは**参照**を持ちます。
だから2つの持ち主が同じオブジェクトを指し、どちらから書いてももう一方に見えます。
値では表せないのは、この一点だけです。

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
レシーバも状態も持たず、クラス名を書いて呼び出します。

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

view はプロパティをたどった先まで読めます。
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

バインディングは**単方向**です。
view は状態を読むだけで、書き込みは必ずメソッドを通ります。
データの流れは一目で追えます。

書き込みは自動的にまとめられます。
メソッドが返るまで再構築は起きません。
同じプロパティに3回書いても、通知は1回です。

---

## 7. view とスタイル

### 18 ウィジェット

Column / Row / Grid / Stack / Text / Button / TextField（IME 対応）/ ListView（仮想化オプション付き）/ ScrollView / HScrollView / Image / Svg / DataTable / Modal / BarChart / LineChart / ProgressBar / Spinner。

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

`for` の本体にも `if` の分岐にも、要素は書いた数だけ置けます。
一方をもう一方の中に入れることもできます。
リピータが回すリストは、名前で指定できるものなら何でもかまいません。
行そのものが持つリストも回せるので、表はこの形になります。

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

例外は `virtualized:` を付けたリストです。
必要になった行だけ、1行につき1要素を作る仕組みなので、`for` の本体に書けるのはちょうど1要素です。
複数並べたいときは `Column` で包みます。

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

どのアームにも書かれていないバリアントは、何も出力しません。
view の構築が途中で失敗しないのは、ここでも変わりません。

`Image` と `Svg` は1度だけデコードし、上限のあるキャッシュを共有します。
長いジャケット一覧をスクロールしても、通り過ぎた画像を全部持ち続けることはありません。
上限はデコード済みピクセルで 256 MB です。
`PIXIE_IMAGE_BUDGET_MB` で変更できます。

### Grid

`Column` と `Row` は一方向に積むだけです。
`Grid` は均等なトラックを順に埋め、行が埋まると次の行へ自動で折り返します。

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

- 要素は、入ったセルをそのまま埋めます（`grow:` は要りません）
- `colSpan:` / `rowSpan:` は複数のトラックにまたがらせる指定で、**どの要素にも**書けます（Column でも、チャートでも、コンポーネントの Button でも）。
  要素自身の性質ではなく、親のグリッドでの置き方を表すからです
- トラックが均等になるのは、そう作ってあるからです。
  エンジンのテンプレートが `repeat(n, minmax(0, 1fr))` なので、列ごとに幅を変える指定（`100px 1fr auto`）は書けません。
  幅の違う列が必要なら、これまでどおり `grow:` 付きの `Row` を使います
- `examples/calcgrid` は `examples/calc` のキーパッドを Grid ひとつに書き直したものです。
  見た目は同じまま、`Row` 5 つと取り分の計算が消えます

### ハンドラ

`onClick:`（や `onTextChanged:` / `onSubmitted:`）の本体には、メソッドの本体と同じ文が書けます。
制御フローも、ローカルも、その場で作るオブジェクトもです。

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

呼び出しが1つだけならブロックは要りません。
`onClick: Board.reset()` と書けます。
裸の `return` を書くと、ハンドラを途中で抜けます。
ループの中からでも抜けられます。

```ruby
onClick: {
  if Board.locked { return }
  Board.commit()
}
```

ハンドラにできて、周りの view 本体にできないことが1つあります。
**メソッドの呼び出し**です。
view の構築は状態を読むだけであり、だからこそ再構築が安全になります。
view の本体はプロパティを読み、何かを変えるのはハンドラの側です。

### カスタムコンポーネント — 再利用できるステートフルビュー

`Main` 以外の `view` はすべて**コンポーネント**です。
パラメータを取り、要素として使え、インスタンスごとの状態を持ちます。
解決はすべてコンパイル時に済みます。
使用箇所にインライン展開されるので、エンジンの語彙は増えません。
両実行ティアは同じ形に展開します。

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

- 使用側に書いたプロパティが、宣言したパラメータに束縛されます（パラメータにはデフォルト値も書けます）
- コンポーネントの中の `state` / `let` は、インスタンスごとにホイストされます。
  ステートフルな使用箇所が増減するとリビルドになり、本体の編集は他のビュー編集と同じくホットリロードで済みます
- `Slot { }` は1コンポーネントに1つです。
  再帰はコンパイルエラーになります
- **行ごとの状態**：`for` リピータの中のステートフルコンポーネントは、**行ごとに独立した状態**を持ちます。
  キーは位置なので、リストが縮んでまた伸びると、元の行の状態がそのまま戻ります（作り直しにはなりません）。
  リピータの深さは問わず、`virtualized:` リストの中でも持てます。
  行ごとの `let` オブジェクトだけが未対応です
- **クロスモジュール**：`pub view` はモジュールを越えます。
  修飾（`ui.Card { }`）、エイリアス（`use ui as U` → `U.Card`）、選択（`use ui.{Card as MyCard}`）のどれでも書けます。
  pub コンポーネントの本体は、自分のモジュールのビュー（private な兄弟も含む）で解決されます

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

箱を描く要素は、角の丸みと枠の太さ、枠の色も取れます。

```ruby
style Card {
  padding: 10.0
  background: "panel"
  borderRadius: 10.0
  borderWidth: 1.0
  borderColor: "accent"     # トークンなので、パレットの切り替えに追従します
}
```

- スタイルはコンパイル時に**完全インライン**されます（実行時のコストはゼロです）
- スタイル名は、それを**書いたモジュール**の側で解決されます。
  そのため、公開したコンポーネントは `pub` を付けていないスタイルも含めて、自分のスタイルのまま他のモジュールで使えます。
  `pub` を付けるかどうかは、他のモジュールがその名前を**書けるか**どうかの話です。
  スタイルがそこまで届くかどうかとは、別の問いです
- 色トークン（`"accent"` や `"panel"` など）はダークテーマとライトテーマに追従します。
  `PIXIE_THEME=light` を付けて起動し、実行中は Cmd+T で切り替えます
- `theme:` は、パレットの効く範囲を部分木に限定します。
  暗いウィンドウの中に明るいパネルを1枚置けます。
  値は式なので、アプリ自身のテーマをふつうの状態として持てます

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

- トークンは要素ツリーの上で、リビルドごとに1度だけ解決されます。
  そのため `PIXIE_SCRIPT="theme:light"` はライトのツリーを出力し、`animate:` の付いた要素はパレットが切り替わるときにクロスフェードします
- スタイルを編集したときも、view 本体の編集と同じく**約1msでホットリロード**されます。
  別のモジュールの `pub style` でも同じです

### アニメーション

アニメーションは、値が動く要素そのものに書きます。
値を動かす更新の側を囲むのではありません。

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

- アニメーションを有効にするのは `animate:` です。
  `easing:` / `enter:` / `exit:` を `animate:` なしで書くと、黙って無視されるのではなく名前付きエラーになります
- 4つとも式を取ります。
  曲線もフェードの有無も状態として持てるので、アプリ側で切り替えを用意できます（`easing: App.curve`、`exit: App.fades`）
- `enter:` は、要素が初めて現れるときにフェードインさせます。
  `exit:` は、view がその要素を出力しなくなったあとも描画を保ち、そのうえでフェードアウトさせます。
  この保持があるので、`if` ブロックが消えるときも間が飛びません
- 数値はそのまま補間されます。
  色は両端が16進リテラルのときだけ補間され、テーマトークンはエンジン側で解決されるため補間されずに切り替わります
- 補間は描画側ではなく要素ツリーの上で走るので、ヘッドレス実行からも見えます。
  `PIXIE_SCRIPT` の `advance:<ms>` は、時計をその分だけ進めた時点で止めます

```sh
PIXIE_SCRIPT="click:show,advance:100" ./app   # フェード途中の1フレームを出力
```

- 時間を扱わないスクリプトでは、dump の前にすべての補間が終わります。
  アニメーションがスクリプトの意味を変えることはありません
- 視差効果を減らす設定が有効なときは、すべての duration が 0 になります

### アクセシビリティ

アクセシビリティツリーの大半は導出できるので、pixie は導出します。
Button はラベルを名前とする button になり、TextField はプレースホルダを名前、入力内容を値とする textInput になります。
ProgressBar は自分の数値を報告します。
レイアウト用のコンテナ自身は何も報告せず、子をそのまま親へ渡します。
「グループ、グループ、グループ」と読み上げられるより、黙っているほうがましだからです。

導出できないものは、どの要素にも書ける2つのプロパティで補います。

```ruby
Text { text: Doc.title; fontSize: 22.0; role: "heading" }

Row {
  role: "group"
  label: "toolbar"
  Svg { source: "save.svg"; label: "Save" }   # 代替テキスト
  Button { text: "save"; onClick: Doc.save() }
}
```

- `role:` に書ける語彙は閉じています（`button` / `label` / `heading` / `textInput` / `image` / `list` / `listItem` / `table` / `dialog` / `progress` / `group`）。
  リテラルで書けばビルド時に検査されます。
  式も書けるので、行のデータ次第で見出しにも項目にもできます。
  実行時に語彙にない名前が来たときは、その要素がもともと導く役割に戻ります
- `label:` は任意の文字列式を取るので、代替テキストに補間も書けます
- アクセシビリティツリーは要素ツリーの上で計算されるので、スクリプトから出力できます

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

失敗しうることは型に現れます（`!Int`）。
エラーを黙って握りつぶす書き方はできません（`case` には ok と err の両方のアームが要ります）。
`T?`（§4）は、失敗ではなく不在を表すための別の道具です。

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

- `await` できるのはバインディングの呼び出しだけです。
  処理はワーカーで走り、結果はメインスレッドに戻ってから pixie の値に変換されます。
  非同期にできるのは今のところここまでです。
  `async fn` は値を返せませんし、別の `async fn` を `await` することもできません
- ランタイムは gpui のプール1つだけです。
  tokio のような二つ目の非同期ランタイムは持ち込みません
- HTTP クライアントは組み込みです。
  `Http.get / getBytes(→Bytes) / post / getWith(url, headers)` があり、ヘッダは `Map<String, String>` で渡します
- ウィンドウを開く実行とヘッドレス実行とで、**実行セマンティクスは同一**です。
  ヘッドレスの実行は、状態が静まるまで決定的に待ちます

---

## 10. Rust バインディング

**crates.io が標準ライブラリ**です。
Rust crate のどの項目を呼べるかを `.rpi` ファイルに書いておくと、呼び出し点のアダプタが型を変換します。

```ruby
# fs.rpi(手書きするならこれだけ)
class Fs {
  static fn writeString(path: String, contents: String) !Void @rust("std::fs::write")
  static fn read(path: String) !Bytes @rust("std::fs::read")
}
```

ふだんは手で書きません。
**rpi-gen** が rustdoc JSON から `.rpi` を導出します。

```
 Rust crate ──▶ rustdoc JSON ──▶ rpi-gen ──▶ .rpi
                                   │
                                   └─ 束縛できない関数は
                                      「理由付きでスキップ」報告
```

アダプタが行う型対応の抜粋です。
引数として渡すときにも、戻り値として受け取るときにも当てはまります。

| Rust 側 | pixie 側 |
|---|---|
| `&str` / `String` / `PathBuf` | `String` |
| `i64`（戻り値なら他の幅も広がります） | `Int` |
| `Vec<T>` | `List<T>` |
| `Vec<u8>` / `&[u8]` | `Bytes` |
| `Option<T>` | `T?` |
| `Result<T, E>` | `!T`（戻り値のみ） |
| kernel の `Map<K, V>` | `Map<K, V>` |
| C ライクな `enum` | rpi-gen が出力する `enum`（下記） |
| `struct`（タプル struct も含む） | rpi-gen が出力する `struct`（下記） |

**戻り値のほうが引数より緩い**点に注意してください。
戻り値なら整数はどの幅でも受け取れますし、`PathBuf` も受け取れます。
一方、引数は pixie 側の型に対応する Rust の型（`i64` や `String`）で受け取ります。
struct のフィールドには書き戻す先の Rust の型を書けるので、フィールドについてはどちらの向きにも渡ります。

### enum は `.rpi` に対応を書けば渡ります

pixie の `enum` と Rust の `enum` は別の型です。
どちらがどれに対応するかは推測に任せず、`.rpi` に書きます。
**この対応は rpi-gen が出力します。**
バインディングを作る対象のモジュールにある公開の C ライク enum は、すべて出力されます。

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

`@rust` を書かなかったバリアントは、その名前がそのまま Rust 側の名前になります。
生成された形で属性が enum 全体の1つだけで済んでいるのは、このためです。
pixie 側で綴りを変えたいときだけ、`dir @rust("Dir")` と書きます。

返ってくるのはふつうの pixie の値なので、`case` でそのまま照合できます。

```ruby
case Kernel.pathKind("/tmp") {
  when Dir { note = "ディレクトリ" }
  when _ { note = "それ以外" }
}
```

限界は2つあり、どちらも黙って壊れるのではなく、理由の分かるエラーになります。
1つは、**ペイロードを持つバリアント**が対応づけられないことです。
変換はバリアントを1対1で照合するので、ペイロードまで対応づけるには、その中のフィールドを一つずつ対応づけることになります。
そのフィールドのほうを渡してください。
もう1つは、属性を書かずに名前だけで対応づける案を採らなかったことです。
Rust 側でバリアント名が変わったときに、何も言わないまま壊れてしまいます。
バインディングは、黙って壊れるといちばん困る場所です。

### struct も同じ形で渡ります

`struct` には対応する Rust の struct を書き、フィールドごとに対応する Rust のフィールドを書きます。
名前だけでは足りないときは、Rust 側の型も添えます。

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
`len` に属性が要るのは、Rust 側が `u64` だからです。
その理由は後で説明します。
ここまでの宣言も rpi-gen が出力します。
フィールド名はキャメルケースに直され、属性は食い違うところにだけ付きます。

返ってくるのはふつうの pixie の値です。
pixie 側で組み立てた値も、そのまま Rust 側へ渡ります。

```ruby
let s = Kernel.fileStat("notes.txt")
size = s.len
line = Kernel.statLine(FileStat(1024, true))
```

フィールドが渡れるかどうかは、値そのものと同じ規則で決まります。
だから struct は struct や enum、リスト、オプショナルを持てますし、`List<FileStat>` や `FileStat?` は要素ごとに渡ります。

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

`Entry` のフィールドには、どれも型を書いていません。
そのまま書き戻すだけで Rust 側が求める型になるからです。

**`len` に型が要る理由。**
数値は pixie 側へ来るときに広がります（`Int` がどの整数幅も吸収します）。
一方、戻すときは幅をぴったり合わせる必要があり、その幅が分かるのは `.rpi` だけです。
属性は Rust のフィールド宣言と同じ形、つまり名前だけか、名前と型かで書きます。
文字列を `PathBuf` に書き戻すときも書き方は同じです。
幅のキャストは巻き戻るので、負の `Int` を `u64` に書き戻すと巨大な値になります。
幅を決めているのは `.rpi` の側で、変換はその指定に従います。

**タプル struct** は位置で対応します。
pixie 側ではフィールドに名前を付け、Rust 側では `.0` で参照します。
この対応も rpi-gen が出力します。

```ruby
struct Perms @rust("pixie_kernel::Perms") {
  var value : Int @rust("0: u32")
}
```

渡れないフィールドが1つでもあると、struct 全体が渡れなくなります。
このときエラーには、どのフィールドが原因かが出ます。
rpi-gen も同じ struct を、理由を付けてスキップします。
理由になるのは、非公開のフィールド（pixie 側から値を入れられません）、対応する型のないフィールド、それに要素のほうが独自の Rust 型を必要とするフィールドです。
属性に書ける型は1つなので、`Vec<u32>` のフィールドには要素の型を書く場所がありません。

---

## 11. モジュールとパッケージ

### モジュール

ファイルがそのままモジュールで、モジュールのパスはディレクトリの構成と一致します。

```ruby
use model                  # 兄弟ファイル model.pix(pub 項目が見える)
use ui.card                # ui/card.pix。card.cardTitle(..) と修飾も可
use model as m             # エイリアス: m.decorate(..)
use model.{decorate}       # 選択インポート
use model.{decorate as d}  # リネーム付き
pub use ui.card.{cardTitle} # 再エクスポート(パッケージの顔を作る)
```

別のモジュールに同じ名前の項目があっても共存できます（内部ではモジュールごとに名前をマングリングします）。
どちらを指すか決まらない裸の参照はエラーになり、候補になった両方のモジュールがエラーに出ます。
修飾して書くか、選択インポートで指定すれば解決します。

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

- pixie に **semver のソルバはありません**。
  Rust の依存は cargo が解決し、pixie パッケージは lock に固定した rev に従います
- 依存パッケージにある `pub style` も、スタイルの展開に含まれます

ここまでの管理には CLI のコマンドがあり、手で編集する必要はありません。

```sh
pixie new my_app                    # pixie.toml + src/main.pix の足場生成
pixie add kit --git https://…       # pixie 依存: fetch + pixie.lock へピン
pixie add kit 1                     # …レジストリインデックス経由
pixie add serde_json 1 --crate      # Rust クレート: cargo 依存 + バインディング導出
pixie update [kit]                  # ピンを解いて再解決、old → new rev を報告
pixie remove kit                    # エントリ + lock ピン + キャッシュを掃除
```

`add` はその場で同期し、fetch や導出に失敗したらマニフェストを**ロールバック**します。
URL を打ち間違えても、プロジェクトが壊れたまま残ることはありません。

---

## 12. 二層実行とホットリロード

同じ `.pix` を実行する仕組みが2つあります。

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

- `pixie watch` は保存のたびに、指紋（fingerprint）で変更の種類を見分けます。
  view の本体やスタイルの編集ならティア2で**約1ms**、それ以外なら再ビルド（約0.5秒）です。
  インポート先のファイルも見ているので、別のモジュールにある `pub style` や公開コンポーネントの本体を編集しても、編集中のファイルと同じようにその場で反映されます
- `pixie build --release` は**ティア2を丸ごと取り除きます**。
  リロードの機構もインタプリタへの依存も生成物から消え、counter デモでは 60MB から 13MB に減ります。
  挙動はバイト単位で同一です

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

できないことの一覧です。
どれも、何がなぜできないのかを**エラーが説明します**。
黙って壊れるものはありません。

- **コンポーネント**：行ごとの `let` オブジェクトと、他のモジュールのコンポーネント本体に書いた修飾参照（`ui.Card`）です。
  コンポーネント自体はモジュールを越えられますし、行ごとの状態はリピータの深さを問わず、`virtualized:` リストの中でも持てます（§7、いずれもゲート済み）
- `virtualized:` リストの行の中でのアニメーション。
  補間を行うパスが、遅延して作られる行をあえて展開しないためです。
  チャートのデータを差し替えるときの補間と、フェード以外のトランジションも同じくできません
- class 型の struct フィールド。
  これは穴ではなく**規則**です。
  struct は代入でコピーされるので、コピーされた参照は同じオブジェクトを指す2つ目の参照になります。
  フィールドを持つ側を `class` にするか、id を持たせて引き直してください
- 独自のパレットの定義（`theme:` が受け取れるのは組み込みの2つです）と、トークン単位の上書き
- 役割、名前、値から先のアクセシビリティ。
  ラベルのない画像へのビルド時の警告、フォーカス順序を明示して指定すること（Tab リングは文書順のままです）、AccessKit のアクションやライブリージョンは、まだありません
- Grid で均等なトラック以外のサイズ指定。
  列ごとに幅を変えるトラック（`100px 1fr auto`）と、明示的な配置（`colStart:` / `rowStart:`）はまだできません。
  エンジンが公開しているのは、均等なトラックと span までです
- init のオーバーロード（init は1クラスに1つです）
- 境界付きのクラスや struct の型パラメータ（`class Sorted<T: Comparable>`）。
  自由関数の型パラメータには境界を書けますが、クラスと struct にはまだ書けません
- ジェネリッククラスへの trait impl、ジェネリックな impl、ジェネリックな store
- ホットリロード中のウィンドウで、ハンドラからジェネリックメソッドを直接呼ぶこと（具象のラッパーメソッドを挟めば呼べます）
- ビットフラグ（`flags Perms of Perm`）。
  型検査はこの形を認識しますが、実際に動かすことはまだできません
- 入れ子のパターン（`when err(bad(v))`）と、リテラルとの照合（`when 42`）。
  `case` のアームに書けるのは、バリアント名と `some` / `nil`、それに `_` です
- Rust 側の関数がペイロード付きの `enum` やジェネリックな struct を受け取る、または返すバインディング。
  C ライクな `enum` と `struct` は渡りますし、タプル struct も渡ります（宣言は rpi-gen が出力します）。
  それ以外も両方向に渡ります。
  数値、真偽値、文字列、バイト列、map、リスト、オプショナル、それに戻り値の `!T` です
- HTTP **サーバー**（構想では宣言的な `service` ブロックです。意図して後回しにしています）
- Linux と Windows（エンジンの動作を確かめているのは macOS です）
