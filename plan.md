## Plan: 旧処理削除付きGPU最大性能化

後方互換を捨て、旧処理・旧設定キー・旧実行分岐を削除しながら、NVIDIA本番向けに並列処理とI/Oを最大性能寄りへ再設計する。目的は高速化を最優先に、同時に拡張性・メンテナンス性・可読性を上げること。

**Steps**
1. Phase 1: 旧設定系の撤去と新設定の単純化
2. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs) から旧並列設定キーと旧解決ロジックを削除し、新キーへ一本化する。対象は CPU/GPUで曖昧な max_parallel_pages_* 系の整理と、GPU本番前提の明示的な page_parallel と job_parallel への再定義。
3. [superbook-pdf/pipeline.toml](superbook-pdf/pipeline.toml) と [data/pipeline.toml](data/pipeline.toml) の旧キーを削除し、新キーのみを残す。
4. [superbook-pdf/src/main.rs](superbook-pdf/src/main.rs) と [superbook-pdf/src/api_server/server.rs](superbook-pdf/src/api_server/server.rs) の設定読込経路から旧キー参照コードを削除する。*depends on 2*

5. Phase 2: 旧実行分岐の削除と並列制御の再設計
6. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs) の CPU優先分岐を削除し、GPU前提の並列制御に統一する。
7. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs) の WorkerPool を job_parallel に一致させ、ジョブ並列とページ並列が過剰に掛け算されないよう制御する。
8. [superbook-pdf/src/runner.rs](superbook-pdf/src/runner.rs) の run_all を有界実行へ変更し、全ページ一括spawnの旧方式を削除する。*parallel with 6*
9. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs) の進捗通知で、イベントごとに tokio::spawn する旧方式を削除し、集約送信へ置換する。*parallel with 8*

10. Phase 3: Upscale経路の旧パラメータ固定を削除
11. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs) に upscale tile/fp32 を必須設定として追加し、固定値依存を排除する。
12. [superbook-pdf/src/stages/upscale/mod.rs](superbook-pdf/src/stages/upscale/mod.rs) と [superbook-pdf/src/stages/upscale/blocks.rs](superbook-pdf/src/stages/upscale/blocks.rs) で payload を新設定準拠に更新する。
13. [superbook-pdf/src/pipeline_builder.rs](superbook-pdf/src/pipeline_builder.rs) で UpscaleStage 初期化を新設定へ合わせる。
14. [ai_services/realesrgan/app.py](ai_services/realesrgan/app.py) の旧デフォルト tile=400 を削除し、方針値へ変更する。

15. Phase 4: 旧I/O経路の削除
16. [superbook-pdf/src/stages/load/blocks.rs](superbook-pdf/src/stages/load/blocks.rs) の PNG一時ファイル前提経路を撤去し、WebP中心の新経路へ統一する。
17. [superbook-pdf/src/stages/save/blocks.rs](superbook-pdf/src/stages/save/blocks.rs) で不要再エンコードの旧処理を削除し、最小回数の保存フローへ整理する。

18. Phase 5: 不要コード除去と回帰防止
19. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs) と [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs) の旧仕様向けテストを削除し、新仕様に置換する。
20. [superbook-pdf/tests](superbook-pdf/tests) に、GPU並列上限・同時ジョブ数制御・大規模PDF時のメモリ上限を検証するテストを追加する。
21. [superbook-pdf/src/api_server/metrics.rs](superbook-pdf/src/api_server/metrics.rs) と [superbook-pdf/src/api_server/routes.rs](superbook-pdf/src/api_server/routes.rs) で新ボトルネック可視化指標を追加する。

**Relevant files**
- [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs) — 旧設定キー削除、新設定モデル一本化
- [superbook-pdf/src/main.rs](superbook-pdf/src/main.rs) — CLI設定解決の旧分岐削除
- [superbook-pdf/src/api_server/server.rs](superbook-pdf/src/api_server/server.rs) — Web起動設定の旧分岐削除
- [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs) — GPU統一並列制御、進捗送信集約、旧分岐削除
- [superbook-pdf/src/runner.rs](superbook-pdf/src/runner.rs) — run_all旧spawn方式削除、有界実行化
- [superbook-pdf/src/pipeline_builder.rs](superbook-pdf/src/pipeline_builder.rs) — Stage初期化の新設定反映
- [superbook-pdf/src/stages/upscale/mod.rs](superbook-pdf/src/stages/upscale/mod.rs) — Stage引数更新
- [superbook-pdf/src/stages/upscale/blocks.rs](superbook-pdf/src/stages/upscale/blocks.rs) — RealESRGAN payload更新
- [ai_services/realesrgan/app.py](ai_services/realesrgan/app.py) — APIデフォルト更新
- [superbook-pdf/src/stages/load/blocks.rs](superbook-pdf/src/stages/load/blocks.rs) — 旧PNG経路撤去
- [superbook-pdf/src/stages/save/blocks.rs](superbook-pdf/src/stages/save/blocks.rs) — 旧再エンコード経路撤去
- [superbook-pdf/src/api_server/metrics.rs](superbook-pdf/src/api_server/metrics.rs) — 新メトリクス
- [superbook-pdf/src/api_server/routes.rs](superbook-pdf/src/api_server/routes.rs) — 指標公開
- [superbook-pdf/pipeline.toml](superbook-pdf/pipeline.toml) — 新キーのみ保持
- [data/pipeline.toml](data/pipeline.toml) — 新キーのみ保持

**Verification**
1. cargo test --features web を実行し、新仕様テストが通ることを確認する。
2. 旧キー入力時に明示エラーになることを確認する（互換読込が残っていないことの検証）。
3. docker compose 起動後に 10/100/300ページで処理時間を測定し、旧実装比の改善を確認する。
4. /api/metrics と /api/stats でジョブ待機・GPUステージ時間が観測可能であることを確認する。
5. AIサービス障害時に retry と失敗伝播が期待通りであることを確認する。

**Decisions**
- 後方互換は実施しない。旧処理は積極的に削除する。
- 対象は NVIDIA GPU本番のみ。
- 変更はコード先行で実施し、ドキュメントは後続更新。
- リスク許容は高。安全側デフォルトより最大性能を優先。
- 除外: UI改修、認証/監査ログ拡張、ROCm個別最適化。

**Further Considerations**
1. 旧設定キーを受け取った場合は黙殺ではなく起動エラーにする（誤設定を早期検知）。
2. 進捗更新頻度は性能優先で削減するが、最小限のUX維持のため上限間隔を決める。
3. I/O最適化で実装難度が高い場合は、まず削除容易な不要ファイル生成経路から先に落とす。

**Execution Breakdown (PR Units)**
1. PR-A: 設定モデル刷新と旧キー撤去
2. 対象: [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs), [superbook-pdf/pipeline.toml](superbook-pdf/pipeline.toml), [data/pipeline.toml](data/pipeline.toml)
3. 完了条件: 旧キー読み込みコードが消えている、旧キー入力で明示エラー、単体テスト更新完了
4. リスク: 設定読込失敗で起動不能
5. ロールバック方針: 直前コミットへ戻すのではなく、最小修正で新キー必須メッセージを改善

6. PR-B: Web/CLI 並列制御統一と旧分岐削除
7. 対象: [superbook-pdf/src/main.rs](superbook-pdf/src/main.rs), [superbook-pdf/src/api_server/server.rs](superbook-pdf/src/api_server/server.rs), [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs)
8. 完了条件: worker_count と job_parallel/page_parallel が同一原則で決定、CPU優先分岐コード撤去
9. リスク: 過並列または過小並列
10. 計測: 10p/100pでGPU使用率と総時間を比較

11. PR-C: Runner 実行方式変更
12. 対象: [superbook-pdf/src/runner.rs](superbook-pdf/src/runner.rs)
13. 完了条件: 全ページ一括spawnが消え、有界実行へ移行、巨大PDFでメモリ増加が抑制
14. リスク: 進捗順序の非決定化
15. テスト: ページ順整列と失敗伝播の回帰テスト追加

16. PR-D: Upscale パラメータ固定撤去
17. 対象: [superbook-pdf/src/stages/upscale/mod.rs](superbook-pdf/src/stages/upscale/mod.rs), [superbook-pdf/src/stages/upscale/blocks.rs](superbook-pdf/src/stages/upscale/blocks.rs), [superbook-pdf/src/pipeline_builder.rs](superbook-pdf/src/pipeline_builder.rs), [ai_services/realesrgan/app.py](ai_services/realesrgan/app.py)
18. 完了条件: tile/fp32 が設定から伝播、サービス側旧デフォルト前提撤去
19. リスク: OOM増加または速度低下
20. 計測: tile 256/320/400 で処理時間と失敗率を比較

21. PR-E: I/O経路統一と旧PNG経路削除
22. 対象: [superbook-pdf/src/stages/load/blocks.rs](superbook-pdf/src/stages/load/blocks.rs), [superbook-pdf/src/stages/save/blocks.rs](superbook-pdf/src/stages/save/blocks.rs)
23. 完了条件: PNG一時前提処理が消え、WebP中心フローへ統一
24. リスク: 画像劣化または互換フォーマット失敗

**Function-level Edit Checklist**
1. PR-A (設定刷新)
2. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): PipelineTomlConfig と ConcurrencyTomlConfig を再定義し、resolved_max_parallel_pages を削除して新解決関数へ置換
3. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): uses_gpu_acceleration は廃止またはGPU固定前提に簡略化
4. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): load_from_path / from_toml で旧キー検出時にエラーを返す
5. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): test_config_toml_parse_* 系を新キー前提で更新、旧キー拒否テストを追加

6. PR-B (Web/CLI並列統一)
7. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs): resolve_max_parallel_pages を削除し、新設定直接参照へ変更
8. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs): WorkerPool::new と worker_count() を job_parallel ベースへ変更
9. [superbook-pdf/src/main.rs](superbook-pdf/src/main.rs): run_pipeline / run_preview / run_serve の max_parallel 解決ロジックを新設定に統一
10. [superbook-pdf/src/api_server/server.rs](superbook-pdf/src/api_server/server.rs): ServerConfig::default の workers 算出を新設定基準に合わせる

11. PR-C (Runner有界化)
12. [superbook-pdf/src/runner.rs](superbook-pdf/src/runner.rs): run_all で全ページ分 handle 配列を作る処理を削除
13. [superbook-pdf/src/runner.rs](superbook-pdf/src/runner.rs): run_page_with_retry / run_page は再利用しつつ、JoinSet等で常時 page_parallel 個まで実行
14. [superbook-pdf/src/runner.rs](superbook-pdf/src/runner.rs): 結果整列ロジックは維持、panic時 page_id=0 フォールバックは廃止検討

15. PR-D (Upscale固定値撤去)
16. [superbook-pdf/src/stages/upscale/mod.rs](superbook-pdf/src/stages/upscale/mod.rs): UpscaleStage フィールドへ tile/fp32 を追加
17. [superbook-pdf/src/stages/upscale/blocks.rs](superbook-pdf/src/stages/upscale/blocks.rs): call_realesrgan_api シグネチャに tile/fp32 を追加し payload へ反映
18. [superbook-pdf/src/pipeline_builder.rs](superbook-pdf/src/pipeline_builder.rs): UpscaleStage::new 呼び出しに新引数を接続
19. [ai_services/realesrgan/app.py](ai_services/realesrgan/app.py): UpscaleRequest の tile デフォルト更新、未指定時補正ロジックを追加

20. PR-E (I/O経路整理)
21. [superbook-pdf/src/stages/load/blocks.rs](superbook-pdf/src/stages/load/blocks.rs): tmp_png 前提処理を削除し、生成ファイル数最小のフローへ統一
22. [superbook-pdf/src/stages/save/blocks.rs](superbook-pdf/src/stages/save/blocks.rs): output_height 同一時の即return維持、再エンコード分岐の不要経路を削除

23. PR-F (観測と回帰)
24. [superbook-pdf/src/api_server/worker.rs](superbook-pdf/src/api_server/worker.rs): publish_web_progress の tokio::spawn 乱立を削除し集約送信へ
25. [superbook-pdf/src/api_server/metrics.rs](superbook-pdf/src/api_server/metrics.rs): queue_wait_ms / gpu_stage_ms / job_total_ms 指標を追加
26. [superbook-pdf/src/api_server/routes.rs](superbook-pdf/src/api_server/routes.rs): 新指標の露出確認
27. [superbook-pdf/tests](superbook-pdf/tests): 100ページ相当の並列制御・待機時間回帰テスト追加

## PR-A Detailed Execution Script

**Goal**
- 設定モデルの旧仕様を削除し、新仕様だけで起動・実行できる状態にする。

**Edit Order**
1. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): `ConcurrencyTomlConfig` を再定義し、`max_parallel_pages_cpu` / `max_parallel_pages_gpu` を削除。
2. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): 新キー `page_parallel` / `job_parallel`（必要なら `gpu_stage_parallel`）を追加。
3. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): `resolved_max_parallel_pages` を削除し、新解決関数（名前例: `resolved_page_parallel` / `resolved_job_parallel`）へ置換。
4. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): `uses_gpu_acceleration` の条件分岐依存を削除または簡略化。
5. [superbook-pdf/pipeline.toml](superbook-pdf/pipeline.toml): 旧キー削除、新キーのみ記述。
6. [data/pipeline.toml](data/pipeline.toml): 同上。
7. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs): テスト更新（旧キー拒否、既定値、新キー解決）。

**Symbols to Remove (must be zero after PR-A)**
1. `max_parallel_pages_cpu`
2. `max_parallel_pages_gpu`
3. `resolved_max_parallel_pages(`

**Validation Commands (PR-A)**
1. `rg "max_parallel_pages_cpu|max_parallel_pages_gpu|resolved_max_parallel_pages\(" superbook-pdf/src superbook-pdf/pipeline.toml data/pipeline.toml`
2. `cargo test --features web config`
3. `cargo test --features web`

**Acceptance Criteria (PR-A)**
1. 旧キーがコード・設定サンプルに残っていない。
2. 旧キー入力時に起動時エラー（黙殺しない）。
3. 既存 `config` 系テストが新仕様で通る。
4. Web/CLI 両方が新キーのみで設定読込できる。

**PR-A Risk Notes**
1. 旧キー削除で既存運用設定が即時失敗するのは仕様どおり。
2. エラーメッセージは「削除済みキー名」を明示し、移行先キーを案内する。

**Definition of Done per PR**
1. ビルド通過: cargo build --features web
2. テスト通過: cargo test --features web
3. 旧仕様残存ゼロ: 旧キー文字列と旧分岐関数がgrepで0件
4. 性能確認: 10p/100p の2ケースで計測値をPR本文に記録

## PR-A Implementation Kickoff Packet

**Scope Lock**
1. 変更対象は [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs), [superbook-pdf/pipeline.toml](superbook-pdf/pipeline.toml), [data/pipeline.toml](data/pipeline.toml) のみ。
2. それ以外のファイルは触らない（PR-Aでの差分肥大防止）。

**Concrete edit checklist (file by file)**
1. [superbook-pdf/src/config.rs](superbook-pdf/src/config.rs)
2. `ConcurrencyTomlConfig` のフィールドを `page_parallel` / `job_parallel`（必要なら `gpu_stage_parallel`）へ置換。
3. `default_max_parallel_pages_cpu` / `default_max_parallel_pages_gpu` 関数を削除し、新キー用 default 関数へ差し替え。
4. `resolved_max_parallel_pages(&self, prefer_gpu: bool)` を削除し、`resolved_page_parallel(&self)` / `resolved_job_parallel(&self)` を追加。
5. `uses_gpu_acceleration` の分岐前提を削減し、GPU本番前提コメントへ更新。
6. `from_toml` で旧キー名（`max_parallel_pages_cpu`, `max_parallel_pages_gpu`）を検知したら `ConfigError` を返す。
7. テストのうち旧仕様前提ケースを削除し、旧キー拒否ケースを追加。

8. [superbook-pdf/pipeline.toml](superbook-pdf/pipeline.toml)
9. `[concurrency]` から旧キー2つを削除し、新キーのみ残す。

10. [data/pipeline.toml](data/pipeline.toml)
11. 同上。

**Post-edit verification sequence**
1. `rg "max_parallel_pages_cpu|max_parallel_pages_gpu|resolved_max_parallel_pages\(" superbook-pdf/src superbook-pdf/pipeline.toml data/pipeline.toml`
2. `cargo test --features web config`
3. `cargo test --features web`

**Expected PR-A output artifacts**
1. 旧キー残存ゼロの検索結果ログ
2. config関連テスト成功ログ
3. 全体テスト成功ログ（失敗時は未解決で次へ進まない）
25. テスト: 画素寸法、ハッシュ近似、PDF出力品質比較

26. PR-F: 観測性と最終回帰
27. 対象: [superbook-pdf/src/api_server/metrics.rs](superbook-pdf/src/api_server/metrics.rs), [superbook-pdf/src/api_server/routes.rs](superbook-pdf/src/api_server/routes.rs), [superbook-pdf/tests](superbook-pdf/tests)
28. 完了条件: 待機時間・GPUステージ時間の可視化、主要回帰テスト通過
29. リスク: メトリクス計測自体のオーバーヘッド
30. 受入基準: 100ページケースで旧実装比 20%以上短縮を最低ラインとする