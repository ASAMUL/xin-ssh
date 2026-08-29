use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};
use tempfile::NamedTempFile;

use crate::error::{codes, io_error, AppError};
use crate::models::{
    AppStateFile, AppStatus, ConnectionGroup, ConnectionProfile, ProfilesFile, Snippet,
    SnippetsFile, TunnelConfig, TunnelsFile, WorkspaceSettings, WorkspaceSnapshot,
};
use crate::util::{now_stamp, path_to_string};

const APP_STATE_FILE: &str = "app-state.json";
const PROFILES_FILE: &str = "profiles.json";
const SETTINGS_FILE: &str = "settings.json";
const TUNNELS_FILE: &str = "tunnels.json";
const SNIPPETS_FILE: &str = "snippets.json";

pub(crate) use crate::models::CURRENT_SCHEMA_VERSION;

#[tauri::command]
pub async fn get_app_status(app: AppHandle) -> Result<AppStatus, AppError> {
    let state = read_app_state(&app)?;
    let workspace_dir = state.and_then(|state_file| state_file.workspace_dir);
    let initialized = workspace_dir
        .as_ref()
        .map(|dir| workspace_files_exist(Path::new(dir)))
        .unwrap_or(false);

    Ok(AppStatus {
        initialized,
        workspace_dir,
    })
}

#[tauri::command]
pub async fn init_workspace(
    app: AppHandle,
    workspace_dir: Option<String>,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = match workspace_dir {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => default_workspace_dir(&app)?,
    };

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn load_workspace(app: AppHandle) -> Result<Option<WorkspaceSnapshot>, AppError> {
    let Some(state) = read_app_state(&app)? else {
        return Ok(None);
    };

    let Some(workspace_dir) = state.workspace_dir else {
        return Ok(None);
    };

    let workspace = PathBuf::from(workspace_dir);
    if !workspace_files_exist(&workspace) {
        return Ok(None);
    }

    load_workspace_from_dir(&workspace).map(Some)
}

#[tauri::command]
pub async fn change_workspace_dir(
    app: AppHandle,
    workspace_dir: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = PathBuf::from(workspace_dir);

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn save_profile(
    app: AppHandle,
    profile: ConnectionProfile,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_profile = profile;
    next_profile.updated_at = now_stamp();

    if let Some(existing) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == next_profile.id)
    {
        *existing = next_profile;
    } else {
        profiles_file.profiles.push(next_profile);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

// 记录最近一次成功连接时间；不 bump updated_at，避免「连了一下」触发同步冲突
#[tauri::command]
pub async fn mark_profile_connected(
    app: AppHandle,
    profile_id: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;

    if let Some(profile) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id)
    {
        profile.last_connected_at = Some(now_stamp());
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn delete_profile(
    app: AppHandle,
    profile_id: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(profile) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id)
    {
        profile.deleted_at = Some(deleted_at);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn save_group(
    app: AppHandle,
    group: ConnectionGroup,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_group = group;
    next_group.updated_at = now_stamp();

    if let Some(existing) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == next_group.id)
    {
        *existing = next_group;
    } else {
        profiles_file.groups.push(next_group);
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn delete_group(app: AppHandle, group_id: String) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(group) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == group_id)
    {
        group.deleted_at = Some(deleted_at.clone());
    }

    for profile in profiles_file
        .profiles
        .iter_mut()
        .filter(|profile| profile.group_id == group_id)
    {
        profile.group_id = default_group_id();
        profile.updated_at = deleted_at.clone();
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    settings: WorkspaceSettings,
) -> Result<WorkspaceSnapshot, AppError> {
    let workspace = active_workspace_dir(&app)?;
    write_settings_file(&workspace, &settings)?;
    load_workspace_from_dir(&workspace)
}

// —— SSH 隧道配置（tunnels.json，按需创建；软删除同 profiles）——

#[tauri::command]
pub async fn list_tunnel_configs(app: AppHandle) -> Result<Vec<TunnelConfig>, AppError> {
    let workspace = active_workspace_dir(&app)?;
    Ok(valid_records(read_tunnels_file(&workspace)?.tunnels, |tunnel| {
        &tunnel.id
    }))
}

#[tauri::command]
pub async fn save_tunnel_config(
    app: AppHandle,
    tunnel: TunnelConfig,
) -> Result<Vec<TunnelConfig>, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut tunnels_file = read_tunnels_file(&workspace)?;
    let mut next_tunnel = tunnel;
    next_tunnel.updated_at = now_stamp();

    if let Some(existing) = tunnels_file
        .tunnels
        .iter_mut()
        .find(|item| item.id == next_tunnel.id)
    {
        *existing = next_tunnel;
    } else {
        tunnels_file.tunnels.push(next_tunnel);
    }

    write_tunnels_file(&workspace, &tunnels_file)?;
    Ok(valid_records(tunnels_file.tunnels, |tunnel| &tunnel.id))
}

#[tauri::command]
pub async fn delete_tunnel_config(
    app: AppHandle,
    tunnel_id: String,
) -> Result<Vec<TunnelConfig>, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut tunnels_file = read_tunnels_file(&workspace)?;

    if let Some(tunnel) = tunnels_file
        .tunnels
        .iter_mut()
        .find(|item| item.id == tunnel_id)
    {
        tunnel.deleted_at = Some(now_stamp());
    }

    write_tunnels_file(&workspace, &tunnels_file)?;
    Ok(valid_records(tunnels_file.tunnels, |tunnel| &tunnel.id))
}

fn read_tunnels_file(workspace: &Path) -> Result<TunnelsFile, AppError> {
    read_json_file_or_recover(&workspace.join(TUNNELS_FILE), TunnelsFile::default)
}

fn write_tunnels_file(workspace: &Path, tunnels_file: &TunnelsFile) -> Result<(), AppError> {
    write_json_file(&workspace.join(TUNNELS_FILE), tunnels_file)
}

// —— 命令片段库（snippets.json，按需创建；软删除同 profiles）——

#[tauri::command]
pub async fn list_snippets(app: AppHandle) -> Result<Vec<Snippet>, AppError> {
    let workspace = active_workspace_dir(&app)?;
    Ok(valid_records(read_snippets_file(&workspace)?.snippets, |snippet| {
        &snippet.id
    }))
}

#[tauri::command]
pub async fn save_snippet(app: AppHandle, snippet: Snippet) -> Result<Vec<Snippet>, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut snippets_file = read_snippets_file(&workspace)?;
    let mut next_snippet = snippet;
    next_snippet.updated_at = now_stamp();

    if let Some(existing) = snippets_file
        .snippets
        .iter_mut()
        .find(|item| item.id == next_snippet.id)
    {
        *existing = next_snippet;
    } else {
        snippets_file.snippets.push(next_snippet);
    }

    write_snippets_file(&workspace, &snippets_file)?;
    Ok(valid_records(snippets_file.snippets, |snippet| &snippet.id))
}

#[tauri::command]
pub async fn delete_snippet(app: AppHandle, snippet_id: String) -> Result<Vec<Snippet>, AppError> {
    let workspace = active_workspace_dir(&app)?;
    let mut snippets_file = read_snippets_file(&workspace)?;

    if let Some(snippet) = snippets_file
        .snippets
        .iter_mut()
        .find(|item| item.id == snippet_id)
    {
        snippet.deleted_at = Some(now_stamp());
    }

    write_snippets_file(&workspace, &snippets_file)?;
    Ok(valid_records(snippets_file.snippets, |snippet| &snippet.id))
}

fn read_snippets_file(workspace: &Path) -> Result<SnippetsFile, AppError> {
    read_json_file_or_recover(&workspace.join(SNIPPETS_FILE), SnippetsFile::default)
}

fn write_snippets_file(workspace: &Path, snippets_file: &SnippetsFile) -> Result<(), AppError> {
    write_json_file(&workspace.join(SNIPPETS_FILE), snippets_file)
}

fn load_workspace_from_dir(workspace: &Path) -> Result<WorkspaceSnapshot, AppError> {
    ensure_workspace(workspace)?;
    let settings = read_settings_file(workspace)?;
    let profiles_file = read_profiles_file(workspace)?;

    Ok(WorkspaceSnapshot {
        workspace_dir: path_to_string(workspace),
        settings,
        groups: valid_records(profiles_file.groups, |group| &group.id),
        profiles: valid_records(profiles_file.profiles, |profile| &profile.id),
    })
}

// 单条记录缺 id 说明这条已经损坏（或被外部工具写坏），丢掉它而不是让整个文件失效
fn valid_records<T>(records: Vec<T>, id_of: impl Fn(&T) -> &String) -> Vec<T> {
    records
        .into_iter()
        .filter(|record| !id_of(record).trim().is_empty())
        .collect()
}

fn ensure_workspace(workspace: &Path) -> Result<(), AppError> {
    fs::create_dir_all(workspace).map_err(io_error)?;
    // 读一次即可：文件缺失或损坏都会在这里被重建为合法默认值
    read_profiles_file(workspace)?;
    read_settings_file(workspace)?;

    Ok(())
}

fn workspace_files_exist(workspace: &Path) -> bool {
    workspace.join(PROFILES_FILE).exists() && workspace.join(SETTINGS_FILE).exists()
}

fn read_app_state(app: &AppHandle) -> Result<Option<AppStateFile>, AppError> {
    let path = app_state_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    match read_json_file::<AppStateFile>(&path) {
        Ok(state) => Ok(Some(state)),
        Err(error) => {
            backup_corrupt_file(&path, &error);
            Ok(None)
        }
    }
}

fn write_app_state(app: &AppHandle, workspace: &Path) -> Result<(), AppError> {
    write_json_file(
        &app_state_path(app)?,
        &AppStateFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            workspace_dir: Some(path_to_string(workspace)),
        },
    )
}

pub(crate) fn active_workspace_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let state = read_app_state(app)?.ok_or_else(|| {
        AppError::new(
            codes::WORKSPACE_NOT_INITIALIZED,
            "Workspace is not initialized",
        )
    })?;
    let workspace_dir = state.workspace_dir.ok_or_else(|| {
        AppError::new(
            codes::WORKSPACE_DIR_MISSING,
            "Workspace directory is not configured",
        )
    })?;

    Ok(PathBuf::from(workspace_dir))
}

fn app_state_path(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app.path().app_data_dir().map_err(io_error)?;
    Ok(dir.join(APP_STATE_FILE))
}

fn default_workspace_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    let dir = app.path().app_data_dir().map_err(io_error)?;
    Ok(dir.join("workspace"))
}

fn read_profiles_file(workspace: &Path) -> Result<ProfilesFile, AppError> {
    read_json_file_or_recover(&workspace.join(PROFILES_FILE), default_profiles_file)
}

fn write_profiles_file(workspace: &Path, profiles_file: &ProfilesFile) -> Result<(), AppError> {
    write_json_file(&workspace.join(PROFILES_FILE), profiles_file)
}

fn read_settings_file(workspace: &Path) -> Result<WorkspaceSettings, AppError> {
    read_json_file_or_recover(&workspace.join(SETTINGS_FILE), WorkspaceSettings::default)
}

fn write_settings_file(workspace: &Path, settings: &WorkspaceSettings) -> Result<(), AppError> {
    write_json_file(&workspace.join(SETTINGS_FILE), settings)
}

pub(crate) fn read_json_file<T>(path: &Path) -> Result<T, AppError>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(io_error)?;
    serde_json::from_str(&content).map_err(|error| {
        AppError::new(codes::WORKSPACE_FILE_CORRUPT, error.to_string())
            .with("path", path_to_string(path))
            .with("detail", error)
    })
}

/// 读配置文件；文件缺失或内容损坏时写回一份合法默认值，保证磁盘上永远是可加载状态。
/// 损坏的原文件不会被删除，而是改名保留成 `<name>.corrupt-<时间戳>` 供人工找回。
pub(crate) fn read_json_file_or_recover<T>(
    path: &Path,
    make_default: impl FnOnce() -> T,
) -> Result<T, AppError>
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    if path.exists() {
        match read_json_file::<T>(path) {
            Ok(value) => return Ok(value),
            Err(error) => backup_corrupt_file(path, &error),
        }
    }

    let value = make_default();
    write_json_file(path, &value)?;
    Ok(value)
}

fn backup_corrupt_file(path: &Path, error: &AppError) {
    // 保留原扩展名再追加后缀，损坏文件在目录里一眼可见：profiles.json.corrupt-<ms>
    let backup = match path.file_name() {
        Some(name) => path.with_file_name(format!(
            "{}.corrupt-{}",
            name.to_string_lossy(),
            now_stamp()
        )),
        None => return,
    };
    let renamed = fs::rename(path, &backup).is_ok();

    eprintln!(
        "[xin-terminal] {} is unreadable ({error}); {}",
        path_to_string(path),
        if renamed {
            format!("kept a copy at {}", path_to_string(&backup))
        } else {
            "could not keep a backup copy".to_string()
        }
    );
}

/// 原子落盘：同目录写临时文件 → fsync → 原子 rename 覆盖目标。
///
/// 旧实现是「写 tmp → 删目标 → rename」，删和 rename 之间崩溃会直接丢掉整份配置。
/// `NamedTempFile::persist` 在 Windows 走 MoveFileEx(REPLACE_EXISTING)、
/// 在 Unix 走 rename(2)，两边都是原子替换，不存在目标文件消失的窗口。
pub(crate) fn write_json_file<T>(path: &Path, value: &T) -> Result<(), AppError>
where
    T: serde::Serialize,
{
    let content = serde_json::to_vec_pretty(value).map_err(io_error)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(io_error)?;

    let mut temp_file = NamedTempFile::new_in(parent).map_err(io_error)?;
    temp_file.write_all(&content).map_err(io_error)?;
    // rename 之前先把数据刷到盘上，否则断电后可能 rename 成功但文件是空的
    temp_file.as_file().sync_all().map_err(io_error)?;
    temp_file
        .persist(path)
        .map_err(|error| io_error(error.error))?;

    Ok(())
}

fn default_profiles_file() -> ProfilesFile {
    let now = now_stamp();
    ProfilesFile {
        schema_version: CURRENT_SCHEMA_VERSION,
        groups: vec![ConnectionGroup {
            id: default_group_id(),
            sync_id: default_group_id(),
            name: "Ungrouped".to_string(),
            sort_order: 0,
            collapsed: false,
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
        }],
        profiles: Vec::new(),
    }
}

fn default_group_id() -> String {
    "group-default".to_string()
}
