# 値と型

[ツアー](tour.md)の続きです。Value クラス、インターフェース、メモリ管理、直和型、Optional と Enum を見ます。

## Value クラスとインターフェース

データそのものは**Value クラス**で持ちます。
`@value` を付けたクラスがネイティブの構造体にコンパイルされます（`@dataclass(frozen=True)` と同じもので、その綴りでも書けます）。
Value クラスは不変なので、書き換えは `replace` で新しい値を作る形です。
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
演算子の特殊メソッド（`__add__`、`__sub__`、`__mul__`）を定義すると、`+` `-` `*` がその意味になります（開発中は Python がそのまま呼び、リリース版では同じ計算がコンパイルされて呼ばれます）。
本文は `return 式` の一文です（不変の値には代入するものがないため）。

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
Protocol を基底に挙げたモデルがその実装になり、Protocol 型の引数を取るヘルパはどの実装を渡しても動きます（実装ごとに特殊化してコンパイルされます）。

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
覚える形は二つだけです。

- **値**（Value クラス、リスト、辞書、文字列）はコピーの意味を持ちます。
  渡した先で書き換わっても、元の側は変わりません。
  リリース版は書き換わる瞬間まで実体を共有する（コピーオンライト）ので、大きなリストを渡しても複製のコストはかかりません。
- **モデル**（と、それを持つストア）は参照です。
  リリース版は参照カウントで管理し、最後の所有が外れた代入のその場で解放します。
  ヒープを走査する GC はなく、停止もありません。

この二つから、日々の作法がそのまま出ます。

- データは Value クラスとリストで持ち、ストアのフィールドに置く。モデルは「共有されて、書き換わって、画面が追随する」ものだけにする。
- ハンドラの中で作って外に渡さなかったモデルは、ハンドラを抜けた時点で解放されます。ループの中で作る一時オブジェクトも同じです。
- 所有の鎖を断てば（`self.root = None`）、その下がまとめて解放されます。生き残った側から `Weak` を読むと None が返ります。

循環だけが例外です。
互いに所有し合うオブジェクトは、鎖を断っても誰も手放せず、リリース版では解放されません（リークであって、クラッシュではありません）。
逆向きの参照を `Weak` にして、循環を作らないのが作法です。
なお開発中の CPython には循環回収があるので、循環を作ってしまったときのメモリの振る舞いだけは二つの実行で同じになりません。
ゲートが比べるのは画面で、メモリは検証の対象外だからです。

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

# ビューの中で:
match health():
    case Healthy():
        text("ALL SYSTEMS NOMINAL")
    case Degraded(services):
        text(f"DEGRADED — {services} service(s)")
    case Outage(service):
        text(f"OUTAGE — {service} is down")
```

case の抜けはコンパイル時に指摘されます。
バリアントのフィールドにデフォルトは書けず、一つのバリアントは一つの直和型にだけ属します。
腕にはガードと `|` の並記が書け、ガードが外れたときは Python と同じく下の腕に落ちます。

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

Enum は普通の `class Mood(Enum)` がそのままコンパイルされます。
`.name` と `.value` は Python と同じ値を返し（`auto()` は 1 から数えます）、`for m in Mood:` は宣言順にメンバーを回ります。
`match` の case は `Mood.MEMBER` か `_` で、抜けは指摘されます。
テキストに入れると Python と同じ `Mood.HAPPY` の形で描画されます。

