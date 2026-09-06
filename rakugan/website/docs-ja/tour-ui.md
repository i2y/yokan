<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# 見た目とウィンドウ

すべての要素が取るキーワード、配色、そしてウィンドウ自身が持ってくるもの。

## すべての要素が取るキーワード

15 の性質が、どの要素にも同じ名前と同じ意味で乗ります。
`width`、`height`、`min_width`、`max_width`、`disabled`、`theme`、`animate`、`easing`、`enter`、`exit`、`col_span`、`row_span`、`role`、`a11y_label`、`tooltip` です。

```perl
    button("save", disabled => $busy, tooltip => "write the file", width => 120);
    text("total", role => "heading", a11y_label => "the running total");
```

その名前を自分の意味で持っている要素は、そちらを保ちます。
`text` の `width` は文字のもので、外側の箱は手を出しません。


## テーマとアニメーション

配色も動きもキーワードです。

```perl
    column(@cells, theme => "dark");
    text("saved", animate => 150, easing => "out", enter => true);
```

`theme` は画面の一部の下で配色を差し替えます。
`animate` は変化にかける時間（ミリ秒）、`easing` はその形（`linear`、`in`、`out`、`inOut`）で、現れるものと消えるものには `enter` と `exit` があります。
色は 16 進の文字列か、配色自身の名前（`accent`、`muted`、`danger`）です。
名前で書けば、一度書いた画面が明暗どちらの配色にも従います。


## ウィンドウまわり

ウィンドウ自身から来るものが四つあります。
どれもアプリを作ったあと、`run` より前に宣言します。

<!-- script: click:+1,key:cmd+s,dump,menu:Clear,dump -->
```perl
use Rakugan;

class Keys {
    use Rakugan;
    field $count = 0;
    field $saved = 0;
    field $last  = "-";

    method save  { $saved = $count }
    method clear { $count = 0; $saved = 0 }

    method typed :Sig(Str) ($key) {
        $last = $key;
    }

    method view {
        return column(
            text("count: $count  saved: $saved"),
            text("last key: $last"),
            button("+1", on_click => sub { $count += 1 }),
            spacing => 8,
            padding => 12,
        );
    }
}

my $app = Keys->new;

menu_item("Count", "Clear", sub { $app->clear });
shortcut("cmd+s", sub { $app->save });
on_key(sub ($chord) { $app->typed($chord) });

run($app, title => "keys");
```

四つ目は `on_file_drop(sub ($path) { ... })` です。
これらはアプリが動いているあいだずっと生きています。

クリップボードは二つの呼び出しです。

```perl
    clipboard_set_text($text);
    $text = clipboard_get_text();
```

ファイルダイアログは人を待つので、ウィンドウのスレッドの外に出します。
そのためにあるのが [`task`](tour-lib.md#タイマーとウィンドウの外でする仕事) です。

```perl
    task(sub { fs_open_dialog("Choose a file") },
         on_done => sub ($path) { $self->took($path) });
    task(sub { fs_save_dialog("notes.txt") },
         on_done => sub ($path) { $self->write_to($path) });
```

ウィンドウなしのスクリプトは、ダイアログに `file:<path>` の手順で答えます。
だからダイアログも、ほかと同じように確かめられる操作になります。

音は、鳴らしたら忘れるファイルです。

```perl
    audio_play("demo/assets/sound/blip.wav");        # 録音されたままの大きさで
    audio_play("demo/assets/sound/blast.wav", 0.4);  # 0.0 から 1.0 の大きさで
    audio_stop();
```

呼び出しはすぐ返り、音の終わりを待つものはありません。
スクリプトでの実行は無音です。
ゲートがスピーカーのあるマシンを要求するわけにはいかないからです。
音の出せないマシンや、読めないファイルは、アプリを失敗させずに何も鳴らしません。
エンジンが解けるのは WAV です。

