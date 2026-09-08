<!-- Written by website/tools/refusalspage from test/refuse/. Edit the fixtures. -->
# Gomamochi が断る書き方

Gomamochi に翻訳器はないので、覚える二つ目の言語もありません。
アプリは Go です。
`gomamochi check` が断るのは、解釈実行がコンパイルした実行と同じには動かせない Go と、ビューが守る少数の規則です。
アプリを読み、受け取れない書き方があれば、ファイルと行と桁、その行そのもの、そして代わりの書き方を示します。
`go` がパスにあれば、コンパイラと同じように型も検査します。
そこで見つかった誤りは、Go 自身の言葉のまま出ます。
`check` はビルドの前にもゲートの前にも走ります。
何もビルドせず、ウィンドウも開かず、言うことがなければ何も出力しません。

下の 21 個には、それを起こすファイルと、出力されるべき文面が、`test/refuse/` にそのまま置いてあります。
`tools/gate_all.sh` がそれを回すので、断りの文面が黙って変わることはありません。
このページも、その同じファイルから引いています。

## アプリの形

アプリはプログラムです。
`package main` で、`main` がアプリを `Run` に渡します。

```console
test/refuse/package_not_main.go:1:9: Gomamochi cannot take this — an app is a `package main` whose `main` hands the app to `Run`
    package app
            ^
```

`main` がなければ、動かすものも `Run` に渡すものもありません。

```console
test/refuse/no_main.go:1:1: Gomamochi cannot take this — there is no `main`. Write `func main() { Run(&App{}, Title("…")) }`
    package main
    ^
```

解釈実行の型がパッケージの `App` インタフェースの代わりを務めるにはラッパーが要りますが、呼び出しの結果をそのまま渡すと、インタプリタはラッパーなしで渡してしまいます。
いったん変数に受けてから渡せばラッパーが付きます。

```console
test/refuse/run_from_call.go:11:19: Gomamochi cannot take this — the app is handed to `Run` straight from a call, and the interpreted run cannot take it that way. Give it a name first: `app := newApp()`, then `Run(app, …)`
    func main() { Run(newApp(), Title("x")) }
                      ^
```

## 解釈実行がコンパイルした実行と同じには動かせない書き方

gc は `min` と `max` を知っていますが、Go 1.22 相当のインタプリタは知りません。
そのままでは、片方の実行でしか動かないアプリになります。

```console
test/refuse/min_max.go:9:32: Gomamochi cannot take this — `min` is Go 1.21's, and the interpreted run does not know it. Write the comparison out (`if a < b { … }`), or a small function of your own
    func (a *Demo) clamp() { a.n = min(a.n, 10) }
                                   ^
```

数に対する `range` は Go 1.22 の書き方で、インタプリタは別の答えを返すのではなくそこで止まります。
だから走らせる前に断ります。

```console
test/refuse/range_number.go:10:17: Gomamochi cannot take this — `range` over a number is Go 1.22's, and the interpreted run stops on it. Write `for i := 0; i < n; i++`
    	for i := range 3 {
    	               ^
```

同じ書き方を、リテラルではなく変数で書いたものです。
型を検査しなければ見つからないので、`go` がパスにあるときに断ります。

```console
test/refuse/range_count.go:9:17: Gomamochi cannot take this — `range` over a number is Go 1.22's, and the interpreted run stops on it. Write `for i := 0; i < n; i++`
    	for i := range a.n {
    	               ^
```

関数に対する `range` は Go 1.23 の書き方で、インタプリタはこれにも止まります。

```console
test/refuse/range_function.go:13:17: Gomamochi cannot take this — `range` over a function is Go 1.23's, and the interpreted run stops on it. Call the function and range over what it answers
    	for i := range pair {
    	               ^
```

`%T` が出力するのは、アプリの型をインタプリタが呼ぶ名前で、Go の名前ではありません。
二つの実行が違う文字列を出力することになります。

```console
test/refuse/percent_t.go:11:57: Gomamochi cannot take this — `%T` names the app's own types differently in the interpreted run. Print the value (`%v`), or give the type a `String` method
    func (a *Demo) View() Element { return Text(fmt.Sprintf("%T", a)) }
                                                            ^
```

`reflect` からも同じ違いが見えます。
インタプリタは、アプリ自身の型をコンパイルした実行とは別の名前で呼びます。

```console
test/refuse/import_reflect.go:4:2: Gomamochi cannot take this — `reflect`: the interpreted run names the app's own types differently from the compiled one, so what it answers would differ. Write the check as a type switch or a method
    	"reflect"
    	^
```

解釈実行は `unsafe` を扱えません。

```console
test/refuse/import_unsafe.go:4:2: Gomamochi cannot take this — `unsafe`: the interpreted run does not take it. Write the same thing in plain Go
    	"unsafe"
    	^
```

解釈実行は C を呼べません。
コンパイルした実行も cgo なしでビルドします。

```console
test/refuse/import_cgo.go:3:8: Gomamochi cannot take this — cgo: the interpreted run cannot call C, and the compiled run is built without it. Reach the engine through the package, and anything else through Go
    import "C"
           ^
```

解釈実行はファイルをソースとして読むので、埋め込むものがありません。

```console
test/refuse/import_embed.go:4:2: Gomamochi cannot take this — `//go:embed`: the interpreted run cannot embed a file. Read it with `os.ReadFile` in a handler, or name it as an element's source
    	_ "embed"
    	^
```

解釈実行は、標準ライブラリの外のモジュールをまだ読めません。
アプリが import できるのは標準ライブラリとこのパッケージです。

```console
test/refuse/import_module.go:4:2: Gomamochi cannot take this — `github.com/example/charts` is a module outside the standard library, and the interpreted run cannot read it yet. The standard library and this package are what an app imports
    	"github.com/example/charts"
    	^
```

解釈実行では、クロージャが捕まえたループ変数は反復ごとのコピーになります。
本体でその変数に代入しても、クロージャが見るのはコピーのほうです。

```console
test/refuse/loop_variable_written.go:11:4: Gomamochi cannot take this — the loop's own variable is written inside its body and a closure captures it: the two runs would disagree about which iteration the closure sees. Copy it first (`j := i`) and let the closure use the copy
    			i = 2
    			^
```

## ビュー

同じ画面を二度組み立てたら同じ画面になる必要があるので、組み立てるときは読むだけです。

```console
test/refuse/view_write.go:8:2: Gomamochi cannot take this — a view only reads. Move the write into a handler — the closure on a button, or a method the app calls from one
    	a.n += 1
    	^
```

ビューは何かが変わるたびに組み立て直されるので、そのたびに処理が走り出してしまいます。
ハンドラから `Task` で一度だけ始めます。

```console
test/refuse/view_goroutine.go:8:2: Gomamochi cannot take this — a view starts a goroutine, and a view is built again from the same state whenever anything changes. Start the work from a handler with `Task`
    	go func() { a.n++ }()
    	^
```

時計は組み立てるたびに違う値を返し、二つの実行が違う画面を描くことになります。
タイマーで読んで、その値をアプリに持たせます。

```console
test/refuse/view_clock.go:11:45: Gomamochi cannot take this — `time.Now` reads the clock, and a view may only read the app: it is built again from the same state whenever anything changes. Read the clock in a timer and keep the answer on the app
    func (a *Demo) View() Element { return Text(time.Now().Format("15:04")) }
                                                ^
```

環境変数、ファイル、ストリーム、ネットワーク、乱数はハンドラで読み、読んだ値をアプリに持たせます。

```console
test/refuse/view_environment.go:11:45: Gomamochi cannot take this — `os.Getenv` reads the environment or a file, and a view may only read the app: it is built again from the same state whenever anything changes. Read it in a handler and keep the answer on the app
    func (a *Demo) View() Element { return Text(os.Getenv("HOME")) }
                                                ^
```

キーボードはデバイスで、読むのはタイマーからです。
ビューからは読みません。

```console
test/refuse/view_keyboard.go:8:5: Gomamochi cannot take this — `KeyDown` reads the keyboard, and a view is built again from the same state whenever anything changes. Call it from a handler or a timer, and keep what it answers on the app
    	if KeyDown("space") {
    	   ^
```

マップを `range` で回る順序は走らせるたびに変わり、二つの実行が違う画面を描くことになります。
キーを集めて並べ替えるだけなら、ハンドラの中で回せます。

```console
test/refuse/range_map.go:9:20: Gomamochi cannot take this — `range` over a map walks it in a different order every run, and a view must draw the same screen from the same state. Keep a sorted list of the keys on the app, made in a handler, and range over that
    	for name := range a.prices {
    	                  ^
```

## Go 自身の判定

Gomamochi の側に言うことがなければ、Go 自身の型検査が、Go 自身の言葉で誤りを伝えます。

```console
test/refuse/undefined_name.go:7:45: undefined: label
```
