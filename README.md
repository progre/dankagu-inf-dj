# dankagu-inf-dj

IDIX INFINITAS 互換コントローラーのスクラッチを
カーソルキー上下の入力に変換する東方ダンマクカグラ向けのツールです。

## 使い方

コントローラーを接続した状態で展開した実行ファイルを起動してください。

コマンドラインで `--help` を指定すると、ヘルプが表示されます。

```absh
Usage: dankagu-inf-dj.exe [OPTIONS]

Options:
  -t, --threshold <THRESHOLD>         [default: 1000]
  -s, --stroke <STROKE_MILLISECONDS>
  -r, --refresh-rate <REFRESH_RATE>   [default: 60]
  -h, --help                          Print help
```

ゲーム内でフレームレート上限の設定に合わせて `--refresh-rate` を設定すると快適に操作できます。

| ゲーム内設定 | 推奨起動オプション             |
| :--------: | :-------------------------: |
| VSync      | `-r` (画面のリフレッシュレート) |
| 60fps上限   | `-r 60`                     |
| 120fps上限  | `-r 120`                    |
| 無制限の    | `-r 0`                      |
