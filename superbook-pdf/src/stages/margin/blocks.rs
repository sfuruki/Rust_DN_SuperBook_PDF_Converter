use crate::stage::StageResult;

pub fn apply_margin(_image_path: &std::path::Path) -> StageResult {
    // 現行経路では margin モジュールとの実処理接続が未完了のため、
    // ステージ責務のみを独立化している。
    Ok(())
}
