# 値と型

[ツアー](tour.md)の続きです。
Value クラス、インターフェース、メモリ管理、直和型、Optional と Enum を見ます。

## Value クラスとインターフェース

データそのものは**Value クラス**で持ちます。
`@value` を付けたクラスが、ネイティブの構造体にコンパイルされます（`@dataclass(frozen=True)` と同じ意味なので、そちらの綴りでも書けます）。
Value クラスは不変なので、書き換えるときは `replace` で新しい値を作ります。
フィールドには、先に宣言した別の Value クラスも置けます（入れ子の値）。

```python
from dataclasses import replace


@value
class Point:
    x: int
    y: int = 0

sel: State[Point] = State(Point(3, 4))
sel.set(replace(sel(), x=10))
text(f"x={sel().x}")
```

Value クラスにはメソッドも書けます。
演算子の特殊メソッド（`__add__`、`__sub__`、`__mul__`）を定義すると、`+` `-` `*` がそのメソッドを呼びます（開発中は Python がそのまま呼び、リリース版では同じ計算がコンパイルされて走ります）。
メソッドの本文は `return 式` の一文だけです（不変の値には代入するものがないためです）。

```python
@value
class V2:
    x: int
    y: int

    def __add__(self, o: "V2") -> "V2":
        return V2(self.x + o.x, self.y + o.y)

    def __mul__(self, k: int) -> "V2":
        return V2(self.x * k, self.y * k)

    def dot(self, o: "V2") -> int:
        return self.x * o.x + self.y * o.y

c.set(a() + b() * 2)      # 演算子は特殊メソッドへ
d.set(a().dot(b()))       # 普通のメソッドはハンドラから
```

インターフェースは `typing.Protocol` です。
Protocol を基底に挙げたモデルが、その実装になります。
Protocol 型の引数を取るヘルパは、どの実装を渡しても動きます（実装ごとに特殊化してコンパイルされます）。

```python
class Shape(Protocol):
    def area(self) -> float: ...


@model
class Circle(Shape):
    r: float = 1.0
    def area(self) -> float:
        return self.r * self.r * 3.0


def area_of(s: Shape) -> float:
    return s.area()
```

## メモリ管理

手で解放するものはありません。
覚えることは二つだけです。

- **値**（Value クラス、リスト、辞書、文字列）は、コピーとして渡されます。
  渡した先で書き換わっても、元の側は変わりません。
  リリース版は書き換わる瞬間まで実体を共有する（コピーオンライト）ので、大きなリストを渡しても複製のコストはかかりません。
- **モデル**（と、それを持つストア）は参照です。
  リリース版は参照カウントで管理します。
  最後の参照が外れると、その代入の時点で解放します。
  ヒープを走査する GC はなく、停止もありません。

この二つから、日々の作法が決まります。

- データは Value クラスとリストで持ち、ストアのフィールドに置きます。
  モデルにするのは「共有されて、書き換わって、画面が追随する」ものだけです。
- ハンドラの中で作って外に渡さなかったモデルは、ハンドラを抜けた時点で解放されます。
  ループの中で作る一時オブジェクトも同じです。
- 所有のつながりを断てば（`self.root = None`）、その下がまとめて解放されます。
  生き残った側から `Weak` を読むと None が返ります。

循環だけが例外です。
互いに所有し合うオブジェクトは、外から所有のつながりを断っても、互いに相手を手放しません。
リリース版では解放されず、そのまま残ります（リークであって、クラッシュではありません）。
逆向きの参照を `Weak` にして、循環を作らないのが作法です。
なお開発中の CPython には循環回収があるので、循環を作ってしまったときのメモリの振る舞いだけは、二つの実行で同じになりません。
ゲートが比べるのは画面で、メモリは検証の対象外だからです。
代わりに、`check` が次の二つを警告します。
フィールドの型が二つ以上のモデルにまたがって輪になっているときと、ハンドラに `a.kid = b`、`b.parent = a` と往復を書いたときです。

生きているオブジェクトの数は、ヘッドレス実行の `mem` ステップでいつでも数えられます。

## 直和型と match

Value クラスを `type` エイリアスで束ねると、`match` で分岐できる選択肢の型（直和型）になります。
`match` はハンドラでもビューでも使え、`case Degraded(services):` のような分解もそのまま書けます。

```python
@value
class Healthy: pass


@value
class Degraded: services: int


@value
class Outage: service: str

type Health = Healthy | Degraded | Outage

health: State[Health] = State(Healthy())

# ビューの中で
match health():
    case Healthy():
        text("ALL SYSTEMS NOMINAL")
    case Degraded(services):
        text(f"DEGRADED — {services} service(s)")
    case Outage(service):
        text(f"OUTAGE — {service} is down")
```

case の抜けはコンパイル時に指摘されます。
バリアントのフィールドにデフォルト値は書けません。
一つのバリアントが属せる直和型は、一つだけです。
case にはガードと `|` の並記が書けます。
ガードが成り立たないときは、Python と同じく下の case に落ちます。

```python
match health():
    case Degraded(services) if services > 3:
        text("badly degraded")
    case Healthy() | Degraded(_):
        text("fine enough")
    case _:
        text("down")
```

## Optional と Enum

Optional は状態にもフィールドにも書けます（`last: int | None = None`）。
絞り込みは walrus の節で見たとおりです。

Enum は `class Mood(Enum)` と普通に書けば、そのままコンパイルされます（`import enum` と `class Mood(enum.Enum)` の形も同じです）。
`.name` と `.value` は Python と同じ値を返します（`auto()` は 1 から数えます）。
`for m in Mood:` は宣言順にメンバーを回ります。
`match` の case に書けるのは `Mood.MEMBER` か `_` で、抜けはここでも指摘されます。
テキストに埋め込むと、Python と同じ `Mood.HAPPY` の形で描画されます。
`repr(m)` は、メンバーについて Python が書くもの（`<Mood.HAPPY: 1>`）を返します。
int、float、bool、bytes の `repr` も、Python が書くものと同じです。
str の `repr` はまだ方言にありません。
Python は Unicode の表で表示できないとされる文字をエスケープしますが、コンパイル側はその表を持っていないからです。

`match` は int、float、str、bool の値も取ります。
`|` で候補を並べたり、`if` でガードを付けたりもできます。

```python
match code():
    case 0 | 1:
        note.set("early")
    case n if n > 100:
        note.set("far")
    case _:
        note.set("middle")
```

## 型の名前

型を付けたモジュールは、使う前に型に名前を付けます。
方言はその名前を、型チェッカーと同じように読みます。

```python
import enum
from typing import NamedTuple, NewType, TypeAlias

Gram = NewType("Gram", int)          # 単位。コンパイル側には int


class Size(enum.Enum):
    SMALL = "small"
    LARGE = "large"
    HUGE = "huge"


class Parcel(NamedTuple):
    weight: Gram
    size: Size


Parcels = list[Parcel]               # 型の別名
Fees: TypeAlias = dict[str, int]     # 同じことを明示した形
type Labels = list[str]              # 新しい書き方
_len = len                           # 組み込み関数に付けた、モジュール自身の名前

OVERSIZE = frozenset({Size.LARGE, Size.HUGE})


def oversize_weight(parcels: Parcels) -> Gram:
    total = Gram(0)
    for p in parcels:
        if p.size in OVERSIZE:
            total = Gram(total + p.weight)
    return total
```

NewType は、型チェッカーが元の型と区別するための名前です。
実行時には、渡された値をそのまま返すだけです（`Gram(5)` は 5 を返します）。
そのため、注釈に書いた NewType は元の型として読み、呼び出しの引数はその元の型であることを確かめます（`Gram(1.5)` は受け付けません。Python は float をそのまま返すからです）。
`isinstance(x, Gram)` は、Python が TypeError を出すのと同じ理由で受け付けません。
NewType にはクラスがないからです。
別名は、三つの書き方のどれでも、その型そのものです。
Value クラスを `|` でつないだものは直和型で、`type Shape = Circle | Rect` の書き方のままです。
組み込み関数、関数、クラスに付けた別名は、呼ぶ場所でその本体と同じです。
ただし、元の名前を関数が自分で定義している場合は受け付けず、関数の側の名前を変えるように求めます。
そこでも Python の別名は組み込みのほうを読み続けるからです。
リテラルの `frozenset` と、その場で書いた集合（`x in {A, B}`）は、`in` と `not in` に答えます。
集合が両方の実行で同じに答えるのは、この問いだけです（どの順で回るかによらないからです）。

コンパイル側が自分のために使っている名前を、クラスやローカルに付けることもあります。
`Element` や `List` という名前のクラス、`state`、`case`、`_` という名前のローカルです。
デコレータのないクラス（NamedTuple、Enum、例外）と、関数の引数とローカルは、コンパイル側だけで別の名前になります。
その名前は誰にも見えません。
開発中の CPython の実行、Enum のメンバーの `str()`、メッセージは、書いたとおりの名前のままです。
ストア、モデル、Value クラスは名前で見つけられるので（開発中の実行はストアのメソッドを名前で探し、Value クラスは Rust の構造体と名前で対応します）、名前を変えられません。
これらには別の名前を付けるように求めます。
