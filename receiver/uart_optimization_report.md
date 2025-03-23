# CXD5602PWBIMU UART通信最適化レポート

> 注: 本レポートは Claude Code による分析結果です。内容は実装予定の TODO であり、実際の実装前に再検討が必要な場合があります。

## 1. 現状分析

### 現在の通信フォーマット
- **形式**: カンマ区切りの16進数文字列（テキスト形式）
- **データ構造**: `timestamp,temp,gx,gy,gz,ax,ay,az`
- **例**: `00000123,41200000,3F800000,3F800000,3F800000,3F800000,3F800000,3F800000`
- **各値の意味**:
  - timestamp: u32のセンサータイムスタンプ（16進数表記）
  - temp: f32の温度値（IEEE-754ビットパターンを16進数表記）
  - gx, gy, gz: f32のジャイロセンサー値（同上）
  - ax, ay, az: f32の加速度センサー値（同上）
- **行区切り**: CR (`\r`) または LF (`\n`)

### 帯域使用率分析

- **ボーレート**: 921600 bps
- **実効データレート**: 921600 ÷ 10 = 92160 バイト/秒（10ビット = 8データビット + 1スタートビット + 1ストップビット）
- **1データあたりのバイト数**:
  - 8つの値 × 8桁の16進数 = 64文字
  - 7つのカンマ = 7文字
  - 改行（CR+LF） = 2文字
  - 合計: 73バイト/データ

- **現在の理論上の最大サンプリングレート**:
  92160 バイト/秒 ÷ 73 バイト/データ ≈ 1262 Hz

- **実際の上限**:
  通信オーバーヘッド、処理遅延などを考慮すると約1000Hz程度

## 2. 問題点

1. **高サンプリングレートでのデータ取りこぼし**:
   - 目標サンプリングレート2000Hzに対して、現在のフォーマットでは理論上も対応できない
   - テキスト形式による冗長性が高い（16進数表記のオーバーヘッド）
   - データパース処理のCPU負荷が高い

2. **同期・エラー検出メカニズムの欠如**:
   - データの破損や欠落を検出する仕組みがない
   - 一部データが失われた場合の再同期が困難

## 3. 最適化方針

### バイナリフォーマットへの変更

- **新フォーマット構造**:
  ```
  [Frame ID(2B)][timestamp(4B)][temp(4B)][gx(4B)][gy(4B)][gz(4B)][ax(4B)][ay(4B)][az(4B)][CRC(1B)]
  ```

- **各フィールドの詳細**:
  - Frame ID: 固定値 0xAA55（フレーム開始識別子）
  - timestamp: u32値をそのままバイナリ転送（4バイト）
  - 各センサー値: f32値をそのままバイナリ転送（4バイト×7）
  - CRC: 8ビットチェックサム（データ完全性の検証用）

- **1データあたりのバイト数**: 35バイト
  - 2バイト (フレームID) + 32バイト (データ) + 1バイト (CRC)

- **新フォーマットでの理論最大サンプリングレート**:
  92160 バイト/秒 ÷ 35 バイト/データ ≈ 2633 Hz

- **実用的な最大サンプリングレート**:
  安全マージンを考慮しても2000Hz以上の対応が可能

### 実装案

```rust
// 新しいバイナリ形式のパース関数
pub fn parse_binary_sensor_data(buffer: &[u8], start_index: usize) -> Result<(SensorData, usize)> {
    // 必要な最小バイト数をチェック (ヘッダ2B + データ32B + CRC 1B = 35B)
    if buffer.len() < start_index + 35 {
        return Err(ReceiverError::ParseError("Incomplete data frame".to_string()).into());
    }
    
    // フレームヘッダをチェック
    if buffer[start_index] != 0xAA || buffer[start_index+1] != 0x55 {
        return Err(ReceiverError::ParseError("Invalid frame header".to_string()).into());
    }
    
    // バイナリからデータを抽出（リトルエンディアン）
    let timestamp = u32::from_le_bytes([buffer[start_index+2], buffer[start_index+3], buffer[start_index+4], buffer[start_index+5]]);
    let temp = f32::from_bits(u32::from_le_bytes([buffer[start_index+6], buffer[start_index+7], buffer[start_index+8], buffer[start_index+9]]));
    let gx = f32::from_bits(u32::from_le_bytes([buffer[start_index+10], buffer[start_index+11], buffer[start_index+12], buffer[start_index+13]]));
    let gy = f32::from_bits(u32::from_le_bytes([buffer[start_index+14], buffer[start_index+15], buffer[start_index+16], buffer[start_index+17]]));
    let gz = f32::from_bits(u32::from_le_bytes([buffer[start_index+18], buffer[start_index+19], buffer[start_index+20], buffer[start_index+21]]));
    let ax = f32::from_bits(u32::from_le_bytes([buffer[start_index+22], buffer[start_index+23], buffer[start_index+24], buffer[start_index+25]]));
    let ay = f32::from_bits(u32::from_le_bytes([buffer[start_index+26], buffer[start_index+27], buffer[start_index+28], buffer[start_index+29]]));
    let az = f32::from_bits(u32::from_le_bytes([buffer[start_index+30], buffer[start_index+31], buffer[start_index+32], buffer[start_index+33]]));
    
    // CRCの検証
    let calculated_crc = calculate_crc8(&buffer[start_index+2..start_index+34]);
    if calculated_crc != buffer[start_index+34] {
        return Err(ReceiverError::ParseError("CRC check failed".to_string()).into());
    }
    
    let system_ts = Utc::now().timestamp_millis();
    
    Ok((SensorData {
        timestamp,
        temp,
        gx,
        gy,
        gz,
        ax,
        ay,
        az,
        system_timestamp: system_ts,
    }, start_index + 35)) // 次のフレーム開始位置を返す
}

// バッファからすべてのセンサーデータを解析
pub fn read_binary_sensor_data(port: &mut Box<dyn SerialPort>) -> Result<Vec<SensorData>> {
    let mut buffer = [0u8; 4096];
    let mut result = Vec::new();
    
    // 入力バッファからデータを読み取る
    let n = match port.read(&mut buffer) {
        Ok(n) => n,
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    
    if n == 0 {
        return Ok(Vec::new());
    }
    
    // 受信バッファ内の完全なフレームをすべて処理
    let mut index = 0;
    while index < n {
        // フレーム開始マーカーを探す
        if index + 1 < n && buffer[index] == 0xAA && buffer[index+1] == 0x55 {
            match parse_binary_sensor_data(&buffer, index) {
                Ok((data, next_index)) => {
                    result.push(data);
                    index = next_index;
                }
                Err(_) => {
                    // 解析エラーの場合は次のバイトから再開
                    index += 1;
                }
            }
        } else {
            // フレーム開始マーカーが見つからない場合は次のバイトへ
            index += 1;
        }
    }
    
    Ok(result)
}
```

## 4. 移行計画

### ステップ1: 送信側（Arduino/センサー側）の変更
- バイナリフォーマットを実装
- フレーム識別子とCRCの追加
- 既存のテキスト形式と新バイナリ形式の両方をサポート（コマンドで切替可能に）

### ステップ2: 受信側（このアプリケーション）の変更
- バイナリ形式のパース機能を実装
- 自動フォーマット検出機能の追加
- バッファリング戦略の最適化
- パフォーマンステストの実施

### ステップ3: 評価とチューニング
- 最大2000Hz以上でのデータ転送テスト
- CPU使用率の評価
- 必要に応じたさらなる最適化

## 5. 将来の拡張性

- **複数センサーのサポート**:
  フレームにセンサーIDを追加して、複数センサーからのデータを区別可能に

- **可変長データフォーマット**:
  オプションフィールドやメタデータを含む拡張可能なフォーマットに対応

- **エラー訂正コード**:
  高ノイズ環境向けに前方誤り訂正（FEC）の実装

---

## 6. TODO リスト

1. **送信側（マイコン）の修正**
   - [ ] バイナリフォーマットの実装
   - [ ] フレーム識別子とCRC計算の追加
   - [ ] フォーマット切替コマンドの実装

2. **受信側（Rust）の修正**
   - [ ] バイナリデータ解析ロジックの実装
   - [ ] フォーマット自動検出機能の追加
   - [ ] バッファリング戦略の最適化
   - [ ] エラー処理と再同期メカニズムの強化

3. **テストと検証**
   - [ ] 単体テストケースの追加
   - [ ] 2000Hz条件下での長時間安定性テスト
   - [ ] エラー注入テスト（データ破損シミュレーション）
   - [ ] CPU使用率とメモリ使用量の測定

4. **ドキュメント**
   - [ ] 新フォーマット仕様書の作成
   - [ ] コード内ドキュメントの更新
   - [ ] 移行ガイドの作成

*解析と報告: Claude Code (2025-03-23)*