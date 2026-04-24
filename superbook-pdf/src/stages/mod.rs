//! stages/ モジュール
//!
//! 構築方針で定義された全パイプラインステージを提供する。
//! 1ステージ1フォルダ構成で、各ステージ固有の処理ブロックを内包する。

pub mod cleanup;
pub mod color;
pub mod deskew;
pub mod load;
pub mod markdown;
pub mod markdown_merge;
pub mod margin;
pub mod normalize;
pub mod ocr;
pub mod page_number;
pub mod save;
pub mod upscale;
pub mod validation;

pub use cleanup::CleanupStage;
pub use color::ColorStage;
pub use deskew::DeskewStage;
pub use load::LoadStage;
pub use markdown::MarkdownStage;
pub use markdown_merge::MarkdownMergeStage;
pub use margin::MarginStage;
pub use normalize::NormalizeStage;
pub use ocr::OcrStage;
pub use page_number::PageNumberStage;
pub use save::SaveStage;
pub use upscale::UpscaleStage;
pub use validation::ValidationStage;
