use std::path::{Component, Path, PathBuf};

use syncflow_core::storage::{SpaceId, SyncedSpace};
use tauri::async_runtime::block_on;
use uuid::Uuid;

use crate::TauriState;

pub fn get_space_by_id(state: &TauriState, space_id: &str) -> Result<SyncedSpace, String> {
    let parsed = Uuid::parse_str(space_id).map_err(|_| "无效的同步空间 ID".to_string())?;
    let storage = block_on(async {
        let guard = state.storage.lock().await;
        guard.get_synced_space(&parsed).await
    })
    .map_err(|e| format!("读取同步空间失败: {e}"))?;

    storage.ok_or_else(|| "同步空间不存在".to_string())
}

pub fn resolve_space_path(
    state: &TauriState,
    space_id: &str,
    relative_path: Option<&str>,
) -> Result<(SyncedSpace, PathBuf), String> {
    let space = get_space_by_id(state, space_id)?;
    let root = PathBuf::from(&space.root_path);
    let root_canonical =
        std::fs::canonicalize(&root).map_err(|e| format!("同步空间根目录不可访问: {e}"))?;

    let relative = relative_path.unwrap_or("");
    validate_relative_path(relative)?;

    if relative.is_empty() {
        return Ok((space, root_canonical));
    }

    let joined = root_canonical.join(relative);
    let target = std::fs::canonicalize(&joined).map_err(map_resolve_error)?;

    if !target.starts_with(&root_canonical) {
        return Err("目标路径超出同步空间范围".to_string());
    }

    Ok((space, target))
}

fn validate_relative_path(relative_path: &str) -> Result<(), String> {
    let path = Path::new(relative_path);

    if path.is_absolute() {
        return Err("不允许使用绝对路径".to_string());
    }

    for component in path.components() {
        match component {
            Component::ParentDir => return Err("不允许使用 .. 路径段".to_string()),
            Component::Prefix(_) => return Err("不允许使用盘符前缀".to_string()),
            Component::RootDir => return Err("不允许使用根路径前缀".to_string()),
            Component::CurDir | Component::Normal(_) => {}
        }
    }

    Ok(())
}

fn map_resolve_error(error: std::io::Error) -> String {
    if error.kind() == std::io::ErrorKind::NotFound {
        "目标不存在".to_string()
    } else {
        format!("无法解析目标路径: {error}")
    }
}

pub fn strip_root_prefix(root_path: &Path, child_path: &Path) -> Result<String, String> {
    child_path
        .strip_prefix(root_path)
        .map_err(|_| "无法计算相对路径".to_string())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
}

pub fn parse_space_id(value: &str) -> Result<SpaceId, String> {
    Uuid::parse_str(value).map_err(|_| "无效的同步空间 ID".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_normal_relative_path() {
        assert!(validate_relative_path("docs/readme.md").is_ok());
    }

    #[test]
    fn rejects_parent_segments() {
        assert!(validate_relative_path("../secret.txt").is_err());
    }

    #[test]
    fn rejects_absolute_paths() {
        let absolute = if cfg!(windows) {
            "C:/Windows/System32"
        } else {
            "/etc/passwd"
        };
        assert!(validate_relative_path(absolute).is_err());
    }

    #[test]
    fn rejects_windows_drive_prefix() {
        assert!(validate_relative_path("C:temp/file.txt").is_err());
    }
}
