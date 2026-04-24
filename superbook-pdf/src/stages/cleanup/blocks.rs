use crate::stage::{StageError, StageResult};

pub async fn cleanup_work_dir(work_dir: &std::path::Path) -> StageResult {
    if work_dir.exists() {
        tokio::fs::remove_dir_all(work_dir)
            .await
            .map_err(|e| StageError::Io {
                stage: "cleanup",
                source: e,
            })?;
    }

    Ok(())
}
