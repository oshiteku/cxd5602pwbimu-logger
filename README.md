# cxd5602pwbimu-logger

## 概要

Sony Spresense の IMU サンプルコードを Arduino IDE で動作するように改変したものです。

- SONY SPRESENSE マルチIMU Add-onボードのデータを USB UART に書き出し
- Arduino IDE で簡単に使用可能

## 使い方

1. [Spresense Arduino スタートガイド](https://developer.sony.com/spresense/development-guides/arduino_set_up_ja.html) に従い Arduino IDE をセットアップ
2. `cxd5602pwbimu-logger.ino` を Arduino IDE で開き、Spresense に書き込み

## 元となったコード

- オリジナルのサンプル: [cxd5602pwbimu_logger_main.c](https://github.com/sonydevworld/spresense/blob/b308223fe058bb0a91887df51f6e2aa76f13e22d/examples/cxd5602pwbimu_logger/cxd5602pwbimu_logger_main.c) (Sony Spresense 公式リポジトリ)
- 参考資料: SaChiKaKK 氏の [Qiita 記事](https://qiita.com/SaChiKaKK/items/50c550782b13fe43061e)

## ライセンス

オリジナルのコードのライセンスに準拠します。
