<!-- Written by website/tools/tour_pages.pl from the tour. Edit the tour. -->
# 見た目とウィンドウ

すべての要素が取るキーワード、配色、そしてウィンドウ自身が持ってくるもの。

## すべての要素が取るキーワード

どの要素も、次の 15 の性質を同じ名前と同じ意味で取ります。
`width`、`height`、`min_width`、`max_width`、`disabled`、`theme`、`animate`、`easing`、`enter`、`exit`、`col_span`、`row_span`、`role`、`a11y_label`、`tooltip` です。

```perl
    button("save", disabled => $busy, tooltip => "write the file", width => 120);
    text("total", role => "heading", a11y_label => "the running total");
```

同じ名前をもともと自分の意味で持っている要素では、要素側の意味がそのまま通ります。
`text` の `width` は文字の幅を指し、外側のコンテナが上書きすることはありません。


## テーマとアニメーション

配色も動きも、キーワードで指定します。

```perl
    column(@cells, theme => "dark");
    text("saved", animate => 150, easing => "out", enter => true);
```

`theme` は、書いた要素から下だけ配色を差し替えます。
`animate` には変化にかける時間をミリ秒で渡し、`easing` にはその曲がり方（`linear`、`in`、`out`、`inOut`）を渡します。
現れるときと消えるときの動きは、`enter` と `exit` で指定します。
色は 16 進の文字列か、配色自身の名前（`accent`、`muted`、`danger`）です。
名前で書けば、一度書いた画面がそのまま明暗どちらの配色にも従います。


## ウィンドウまわり

ウィンドウ自身から来るものが、次の四つです。
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
これらは、アプリが動いているあいだずっと有効です。

クリップボードは、二つの呼び出しで読み書きします。

```perl
    clipboard_set_text($text);
    $text = clipboard_get_text();
```

ファイルダイアログは人の操作を待つので、ウィンドウのスレッドの外に出します。
そのためにあるのが [`task`](tour-lib.md#タイマーとウィンドウの外でする処理) です。

```perl
    task(sub { fs_open_dialog("Choose a file") },
         on_done => sub ($path) { $self->took($path) });
    task(sub { fs_save_dialog("notes.txt") },
         on_done => sub ($path) { $self->write_to($path) });
```

ウィンドウなしのスクリプトは、ダイアログに `file:<path>` の手順で答えます。
だからダイアログも、ほかの操作と同じように確かめられます。

音は、ファイルを鳴らして、あとは放っておく形です。

```perl
    audio_play("demo/assets/sound/blip.wav");        # 録音されたままの大きさで
    audio_play("demo/assets/sound/blast.wav", 0.4);  # 0.0 から 1.0 の大きさで
    audio_stop();
```

呼び出しはすぐ返り、鳴り終わるのを待ちません。
スクリプトで走らせるときは無音になります。
ゲートがスピーカーのあるマシンを要求するわけにはいかないからです。
音の出ないマシンや、読めないファイルでは、アプリを止めずに何も鳴りません。
エンジンが読めるのは WAV です。

