# UART ロガーツール要件

## 基本機能

### データ収集
- Arduinoなどからのシリアル接続（UART）を通じてデータを読み取る
- 想定するフォーマット: アスキーHEX表現されたセンサーデータ
  - 例: `00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000`
  - 構成: タイムスタンプ(uint32_t), 温度(float), ジャイロx/y/z(float), 加速度x/y/z(float)
  - 各値はFloatをビット表現に変換し、uint32_tとしてhex出力されている

### データ処理
- 受信したHEXデータを適切な型（u32/float）に変換
- システムタイムスタンプを追加記録
- データバッファリングによる効率的な処理

### データ永続化
- Parquetフォーマットでデータを保存
- 圧縮オプション（SNAPPY, GZIP, LZ4, ZSTD, 無圧縮）
- 定期的なファイル分割機能（指定時間ごとに新ファイル作成）

### 制御機能
- Ctrl-Cで安全に終了（バッファ内データを確実に保存）
- エラー処理（パースエラー、ノイズ混入時の堅牢な動作）

## 拡張機能

### 非同期処理
- データ受信スレッドとファイル書き込みスレッドの分離
- チャネルを用いたスレッド間通信
- バッファサイズの調整機能

### 操作性
- コマンドラインインターフェース
- 詳細なログ出力
- 設定の柔軟性（ポート、ボーレート、出力先など）

## 入出力仕様

### 入力データ形式
```
%08x,%08x,%08x,%08x,%08x,%08x,%08x,%08x\n
```
- 1フィールド目: タイムスタンプ (uint32_t)
- 2フィールド目: 温度センサー値 (floatをuint32_tにビットキャスト)
- 3-5フィールド目: ジャイロセンサー値 X/Y/Z (floatをuint32_tにビットキャスト)
- 6-8フィールド目: 加速度センサー値 X/Y/Z (floatをuint32_tにビットキャスト)

### 出力データ形式
Parquetファイル内のスキーマ:
```
message schema {
    required INT64 timestamp;
    required FLOAT temp;
    required FLOAT gx;
    required FLOAT gy;
    required FLOAT gz;
    required FLOAT ax;
    required FLOAT ay;
    required FLOAT az;
    required INT64 system_timestamp;
}
```

## 使用方法

### コマンドライン引数
```
cargo run -- -p /dev/ttyUSB0 -b 115200 -o ./logs -s 60 -p sensor_log -c snappy -u 100
```

- `-p, --port`: シリアルポート指定 (例: `/dev/ttyUSB0`, `COM3`)
- `-b, --baud_rate`: ボーレート (デフォルト: 115200)
- `-o, --output_dir`: 出力ディレクトリ (デフォルト: `./logs`)
- `-s, --split_minutes`: ファイル分割時間（分単位、0=分割なし）
- `-p, --prefix`: 出力ファイル名のプレフィックス
- `-c, --compression`: 圧縮アルゴリズム (none, snappy, gzip, lz4, zstd)
- `-u, --buffer_size`: バッファサイズ（何件のデータをまとめて書き込むか）

## 依存ライブラリ
- anyhow: エラーハンドリング
- thiserror: カスタムエラー定義
- serialport: シリアルポート通信
- parquet: Parquetファイル操作
- chrono: 日時処理
- clap: コマンドライン引数解析
- ctrlc: シグナルハンドリング
