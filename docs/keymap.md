# キーバインドのカスタマイズ（フォーク独自機能）

`~/.config/karukan-im/keymap.toml` にキーと動作の対応を書くと、組み込みのキーバインドより優先して使われます。

```toml
[bindings]
"Ctrl+U" = "hiragana"
"Ctrl+I" = "katakana"
"Ctrl+P" = "alphanumeric_full"
"Ctrl+O" = "alphanumeric_half"
```

読み込むのは起動時だけなので、変更したら `fcitx5 -r` で再起動してください。

## 動作するタイミング

- 入力中・変換中だけ有効です。何も入力していないときは、そのキーはアプリケーションにそのまま渡ります（エディタの Ctrl+P などはそのまま使えます）。
- 組み込みと同じキーを指定すると、組み込みの動作を上書きします（上の例では Ctrl+I の🤖AI候補ビューと、変換中の Ctrl+P の前候補）。

## 動作一覧

| 動作 | 内容 |
|------|------|
| `hiragana` | 入力全体をひらがなにする |
| `katakana` | 入力全体を全角カタカナにする |
| `alphanumeric_full` | 打鍵したキーそのものを全角英数にする（`ｋａｎｊｉ`） |
| `alphanumeric_half` | 打鍵したキーそのものを半角英数にする（`kanji`） |
| `ai_candidates` | 🤖AI候補のビューを開く（組み込みの Ctrl+I） |
| `katakana_mode` | カタカナ入力モードに切り替える（組み込みの Ctrl+K） |

`hiragana` から `alphanumeric_half` は変換ウィンドウを開き、選んだ形を先頭に、残りの3つの形を候補として並べます。Enter で確定、Space / ↓ で他の形、Escape で入力に戻ります。続けて別のキーを押せば、そのまま別の形に切り替わります。

英数の2つは、ローマ字テーブルを通す前の打鍵を使います。独自のローマ字テーブル（`romaji.tsv`）で `kd` → けん としていても、結果は `kd` です。入力の途中で文字を削除したりカーソルを動かして編集したりして打鍵と対応が取れなくなった場合は、読みをローマ字に直したもの（けん → `ken`）になります。英数の結果は学習しません。

## キーの書き方

`修飾キー+キー` の形で書きます。

- 修飾キー: `Ctrl`, `Alt`, `Shift`, `Super`
- キー: 英数字・記号1文字（大文字小文字は区別しない。Shift は `Shift+` で明示する）、`F1`〜`F12`, `Space`, `Tab`, `Enter`, `Escape`, `BackSpace`, `Delete`, `Home`, `End`, `Left`, `Right`, `Up`, `Down`, `Henkan`, `Muhenkan`

解釈できない行はログに警告を出して無視します。
