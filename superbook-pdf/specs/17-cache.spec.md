# 17-cache.spec.md - 処理結果キャッシュモジュール仕様

## 概要

PDF処理結果のキャッシュ機能。C#版の「OKファイル」機構を参考に、ハッシュベースのスマートキャッシュを実装する。

## 目的

- 同一PDFの再処理をスキップ（高速化）
- 処理オプションが変わった場合は再処理
- 処理結果メタデータの保存

## 設計

### CacheDigest (構造体)

処理をユニークに識別するダイジェスト。

```rust
pub struct CacheDigest {
    /// ソースファイルの最終更新日時 (Unix timestamp)
    pub source_modified: u64,
    /// ソースファイルのサイズ (bytes)
    pub source_size: u64,
    /// 処理オプションのハッシュ
    pub options_hash: String,
}
```

### ProcessingCache (構造体)

キャッシュファイル（`.superbook-cache`）の内容。

```rust
pub struct ProcessingCache {
    /// キャッシュバージョン
    pub version: u32,
    /// 処理日時 (Unix timestamp)
    pub processed_at: u64,
    /// ダイジェスト
    pub digest: CacheDigest,
    /// 処理結果メタデータ
    pub result: ProcessingResult,
}
```

### ProcessingResult (構造体)

処理結果のメタデータ。

```rust
pub struct ProcessingResult {
    /// 処理したページ数
    pub page_count: usize,
    /// 検出されたページ番号シフト
    pub page_number_shift: Option<i32>,
    /// 縦書き検出結果
    pub is_vertical: bool,
    /// 処理時間 (秒)
    pub elapsed_seconds: f64,
    /// 出力ファイルサイズ
    pub output_size: u64,
}
```

## API

### キャッシュ操作

| 関数 | 説明 |
|------|------|
| `CacheDigest::new(source_path, options)` | ダイジェスト生成 |
| `ProcessingCache::load(output_path)` | キャッシュ読み込み |
| `ProcessingCache::save(output_path)` | キャッシュ保存 |
| `ProcessingCache::is_valid(digest)` | キャッシュ有効性確認 |

### キャッシュファイル

```
output.pdf           # 出力PDF
output.pdf.superbook-cache  # キャッシュファイル (JSON)
```

### キャッシュファイル形式

```json
{
  "version": 1,
  "processed_at": 1706123456,
  "digest": {
    "source_modified": 1705987654,
    "source_size": 12345678,
    "options_hash": "sha256:abc123..."
  },
  "result": {
    "page_count": 100,
    "page_number_shift": 2,
    "is_vertical": true,
    "elapsed_seconds": 45.3,
    "output_size": 54321098
  }
}
```

## CLI統合

```bash
# キャッシュを使用（デフォルト）
superbook-pdf convert input.pdf output/

# キャッシュを無視して再処理
superbook-pdf convert input.pdf output/ --force

# キャッシュ情報を表示
superbook-pdf cache-info output/file.pdf
```

## テストケース

| TC ID | テスト内容 |
|-------|----------|
| CACHE-001 | CacheDigest生成 |
| CACHE-002 | CacheDigest比較 (同一) |
| CACHE-003 | CacheDigest比較 (ファイル変更) |
| CACHE-004 | CacheDigest比較 (オプション変更) |
| CACHE-005 | ProcessingCache保存 |
| CACHE-006 | ProcessingCache読み込み |
| CACHE-007 | キャッシュファイルが存在しない場合 |
| CACHE-008 | キャッシュバージョン不一致 |
| CACHE-009 | 破損したキャッシュファイル |
| CACHE-010 | --force フラグでキャッシュ無視 |

## 実装ステータス

| 機能 | 状態 | 備考 |
|------|------|------|
| CacheDigest構造体 | ✅ | sha256ハッシュ |
| ProcessingCache構造体 | ✅ | JSONシリアライズ対応 |
| キャッシュ保存/読み込み | ✅ | .superbook-cacheファイル |
| CLI統合 | ✅ | --force オプション |
| テスト | ✅ | 23テスト実装 |
