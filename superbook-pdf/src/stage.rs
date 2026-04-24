//! Stage trait と PageContext の定義
//!
//! 構築方針に従い、全ステップを共通の Stage trait で抽象化する。
//! CLI と WebUI の両方が同じ PipelineRunner を呼び出す構造にする。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

// ============================================================
// StageResult / StageError
// ============================================================

/// ステージ処理結果
pub type StageResult = Result<(), StageError>;

/// ステージ処理エラー
#[derive(Debug, Error)]
pub enum StageError {
    #[error("IO error in stage '{stage}': {source}")]
    Io {
        stage: &'static str,
        #[source]
        source: std::io::Error,
    },

    #[error("Image processing error in stage '{stage}': {message}")]
    Image {
        stage: &'static str,
        message: String,
    },

    #[error("AI service error in stage '{stage}': {message}")]
    AiService {
        stage: &'static str,
        message: String,
    },

    #[error("Configuration error in stage '{stage}': {message}")]
    Config {
        stage: &'static str,
        message: String,
    },

    #[error("Validation failed in stage '{stage}': {message}")]
    Validation {
        stage: &'static str,
        message: String,
    },

    #[error("Stage '{stage}' skipped: {reason}")]
    Skipped {
        stage: &'static str,
        reason: String,
    },

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl StageError {
    /// エラーがスキップ（非致命的）かどうか
    pub fn is_skipped(&self) -> bool {
        matches!(self, StageError::Skipped { .. })
    }
}

// ============================================================
// PageStatus（進捗通知用）
// ============================================================

/// ページ処理の進捗状態
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PageProcessingStatus {
    /// 待機中
    Pending,
    /// 処理中（現在のステージ名）
    Processing(String),
    /// 完了
    Done,
    /// エラー
    Failed(String),
    /// スキップ
    Skipped(String),
}

impl Default for PageProcessingStatus {
    fn default() -> Self {
        Self::Pending
    }
}

// ============================================================
// PageContext（ページ単位の状態）
// ============================================================

/// ページ単位の処理コンテキスト
///
/// 各ステージはこの構造体を受け取り、image_path や text_path を
/// 上書きしながら処理を進める（上書き方式）。
#[derive(Debug, Clone)]
pub struct PageContext {
    /// ページID（1始まり）
    pub page_id: usize,

    /// このページの中間ファイル格納ディレクトリ
    /// 例: /data/work/0001/
    pub work_dir: PathBuf,

    /// 現在の画像パス（常に最新の WebP）
    /// 各ステージが上書きすることで処理が進む
    pub image_path: PathBuf,

    /// OCR テキストパス（OCR ステージ後に設定される）
    pub text_path: Option<PathBuf>,

    /// ページ単位 Markdown パス（Markdown ステージ後に設定される）
    pub markdown_path: Option<PathBuf>,

    /// 検出されたページ番号（PageNumber ステージ後に設定される）
    pub detected_page_number: Option<i32>,

    /// 進捗状態（WebSocket 通知用）
    pub status: PageProcessingStatus,

    /// 元の PDF ページ番号（抽出時に設定）
    pub source_page_number: Option<u32>,
}

impl PageContext {
    /// 新しい PageContext を作成する
    ///
    /// # Arguments
    /// * `page_id` - ページID（1始まり）
    /// * `work_base_dir` - 作業ディレクトリの基底パス（/data/work/ など）
    pub fn new(page_id: usize, work_base_dir: &std::path::Path) -> Self {
        // 物理ファイル配置は 0 始まりで統一する: /work/0000/gaozou.webp
        let work_index = page_id.saturating_sub(1);
        let work_dir = work_base_dir.join(format!("{:04}", work_index));
        let image_path = work_dir.join("gaozou.webp");
        let text_path = work_dir.join("ocr.txt");
        Self {
            page_id,
            work_dir,
            image_path,
            text_path: Some(text_path),
            markdown_path: None,
            detected_page_number: None,
            status: PageProcessingStatus::Pending,
            source_page_number: None,
        }
    }

    /// 作業ディレクトリを確保する
    pub fn ensure_work_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.work_dir)
    }

    /// ステータスを「処理中」に更新する
    pub fn set_processing(&mut self, stage_name: &str) {
        self.status = PageProcessingStatus::Processing(stage_name.to_string());
    }

    /// ステータスを「完了」に更新する
    pub fn set_done(&mut self) {
        self.status = PageProcessingStatus::Done;
    }

    /// ステータスを「エラー」に更新する
    pub fn set_failed(&mut self, reason: &str) {
        self.status = PageProcessingStatus::Failed(reason.to_string());
    }
}

// ============================================================
// Stage trait（全ステップ共通インターフェース）
// ============================================================

/// パイプラインの各処理ステップが実装するトレイト
///
/// CLI と WebUI の両方から同じステージが呼び出される。
/// Vec<Box<dyn Stage>> に積むだけで処理順序が決まる。
#[async_trait]
pub trait Stage: Send + Sync {
    /// ステージを実行する
    ///
    /// `ctx` の `image_path` を読み込み、処理結果で上書きすることで
    /// 次のステージに引き継がれる。
    async fn run(&self, ctx: &mut PageContext) -> StageResult;

    /// ステージ名（ログ・進捗表示に使用）
    fn name(&self) -> &'static str;

    /// このステージが有効かどうか（設定で無効化できる）
    fn is_enabled(&self) -> bool {
        true
    }
}
