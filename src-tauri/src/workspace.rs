use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};

use tauri::{AppHandle, Manager};
use tempfile::NamedTempFile;

use crate::error::{codes, io_error, AppError};
use crate::models::{
    AppStateFile, AppStatus, AuthMethod, ConnectionExportEntry, ConnectionGroup, ConnectionProfile,
    ProfilesFile, Snippet, SnippetsFile, SshConfigEntry, TerminalBackground, TunnelConfig,
    TunnelsFile, WorkspaceSettings, WorkspaceSnapshot,
};
use crate::util::{now_stamp, path_to_string};

const APP_STATE_FILE: &str = "app-state.json";
const PROFILES_FILE: &str = "profiles.json";
const SETTINGS_FILE: &str = "settings.json";
const TUNNELS_FILE: &str = "tunnels.json";
const SNIPPETS_FILE: &str = "snippets.json";

// 快照读取也可能恢复损坏文件，和读改写共用一把锁，避免默认值覆盖刚保存的配置。
static WORKSPACE_LOCK: Mutex<()> = Mutex::new(());
static APP_STATE_LOCK: Mutex<()> = Mutex::new(());

fn lock_workspace() -> MutexGuard<'static, ()> {
    WORKSPACE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

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
    let _guard = lock_workspace();
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
    let _guard = lock_workspace();
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
pub async fn read_ssh_config(path: String) -> Result<Vec<SshConfigEntry>, AppError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppError::new(
            codes::SSH_CONFIG_IMPORT_FAILED,
            "SSH config path is empty",
        ));
    }

    let content = fs::read_to_string(path).map_err(|error| {
        AppError::new(codes::SSH_CONFIG_IMPORT_FAILED, error.to_string())
            .with("path", path)
    })?;
    parse_ssh_config(&content)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionExportFile {
    format_version: u16,
    exported_at: String,
    profiles: Vec<ConnectionExportEntry>,
}

#[tauri::command]
pub async fn export_profiles(
    path: String,
    profiles: Vec<ConnectionExportEntry>,
) -> Result<(), AppError> {
    let path = path.trim();
    if path.is_empty() {
        return Err(AppError::new(
            codes::PROFILE_EXPORT_FAILED,
            "Export path is empty",
        ));
    }

    write_json_file(
        Path::new(path),
        &ConnectionExportFile {
            format_version: 1,
            exported_at: now_stamp(),
            profiles,
        },
    )
    .map_err(|error| AppError::new(codes::PROFILE_EXPORT_FAILED, error.to_string()).with("path", path))
}

// 只导入连接身份字段，避免把 Include、ProxyCommand 等本机 shell 语义带入应用配置。
fn parse_ssh_config(content: &str) -> Result<Vec<SshConfigEntry>, AppError> {
    let mut global = SshConfigDraft::default();
    let mut blocks: Vec<(Vec<String>, SshConfigDraft)> = Vec::new();
    let mut current_aliases: Option<Vec<String>> = None;
    let mut current = SshConfigDraft::default();
    let mut ignored_block = false;
    let mut pending = String::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let logical = if line.ends_with('\\') {
            pending.push_str(line.trim_end_matches('\\'));
            continue;
        } else if pending.is_empty() {
            line.to_string()
        } else {
            pending.push_str(line);
            let joined = pending.clone();
            pending.clear();
            joined
        };

        let (key, value) = split_ssh_config_line(&logical);
        let Some(value) = value else { continue };
        if key.eq_ignore_ascii_case("host") {
            if let Some(aliases) = current_aliases.take() {
                blocks.push((aliases, current));
            }
            let aliases: Vec<String> = value
                .split_whitespace()
                .filter(|alias| !alias.contains('*') && !alias.contains('?') && !alias.starts_with('!'))
                .map(str::to_string)
                .collect();
            let wildcard_defaults = value.split_whitespace().any(|alias| alias == "*");
            if !aliases.is_empty() {
                current_aliases = Some(aliases);
                current = global.clone();
                ignored_block = false;
            } else if wildcard_defaults {
                current_aliases = None;
                current = SshConfigDraft::default();
                ignored_block = false;
            } else {
                current_aliases = None;
                current = SshConfigDraft::default();
                ignored_block = true;
            }
            continue;
        }

        if !ignored_block {
            let draft = if current_aliases.is_some() { &mut current } else { &mut global };
            apply_ssh_config_option(draft, &key, &value);
        }
    }

    if let Some(aliases) = current_aliases {
        blocks.push((aliases, current));
    }

    let home = home_directory();
    let entries = blocks
        .into_iter()
        .flat_map(|(aliases, draft)| {
            let home = &home;
            aliases.into_iter().map(move |alias| SshConfigEntry {
                host: draft.host.clone().unwrap_or_else(|| alias.clone()),
                alias,
                username: draft.username.clone().unwrap_or_else(default_ssh_username),
                port: draft.port.unwrap_or(22),
                private_key_path: draft.identity_file.as_deref().map(|path| expand_home(path, home)),
            })
        })
        .filter(|entry| !entry.host.trim().is_empty())
        .collect();
    Ok(entries)
}

#[derive(Clone, Default)]
struct SshConfigDraft {
    host: Option<String>,
    username: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
}

fn split_ssh_config_line(line: &str) -> (String, Option<String>) {
    let line = line
        .split_once(" #")
        .map(|(before, _)| before)
        .unwrap_or(line)
        .trim();
    let mut parts = line.splitn(2, |character: char| character.is_whitespace());
    let key = parts.next().unwrap_or_default().to_string();
    let value = parts.next().map(str::trim).filter(|value| !value.is_empty());
    if value.is_some() {
        return (key, value.map(str::to_string));
    }

    let mut equals = line.splitn(2, '=');
    let key = equals.next().unwrap_or_default().trim().to_string();
    (key, equals.next().map(str::trim).filter(|value| !value.is_empty()).map(str::to_string))
}

fn apply_ssh_config_option(draft: &mut SshConfigDraft, key: &str, value: &str) {
    match key.to_ascii_lowercase().as_str() {
        "hostname" => draft.host = Some(value.to_string()),
        "user" => draft.username = Some(value.to_string()),
        "port" => draft.port = value.parse::<u16>().ok().filter(|port| *port > 0),
        "identityfile" if draft.identity_file.is_none() => draft.identity_file = Some(value.to_string()),
        _ => {}
    }
}

fn home_directory() -> String {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default()
}

fn default_ssh_username() -> String {
    std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_default()
}

fn expand_home(path: &str, home: &str) -> String {
    if home.is_empty() || path == "~" {
        return if path == "~" && !home.is_empty() { home.to_string() } else { path.to_string() };
    }
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        return PathBuf::from(home).join(rest).to_string_lossy().into_owned();
    }
    path.to_string()
}

#[tauri::command]
pub async fn change_workspace_dir(
    app: AppHandle,
    workspace_dir: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
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
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_profile = profile;
    next_profile.updated_at = now_stamp();
    let no_saved_credentials = matches!(&next_profile.auth_method, AuthMethod::Agent | AuthMethod::KeyboardInteractive);
    if no_saved_credentials {
        next_profile.credential_ref = None;
    }

    if let Some(existing) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == next_profile.id)
    {
        if existing.deleted_at.is_some() {
            return Err(AppError::new(
                codes::PROFILE_NOT_FOUND,
                "Connection profile not found",
            ));
        }
        // 最近连接、背景和识别到的系统由专用命令维护，旧表单快照不能把它们写回旧值或清空。
        next_profile.last_connected_at = existing.last_connected_at.clone();
        next_profile.terminal_background = existing.terminal_background.clone();
        next_profile.detected_os = existing.detected_os.clone();
        // 先保存认证方式，再删除秘密；保留引用能在清理失败或进程中断后重试。
        if no_saved_credentials {
            next_profile.credential_ref = existing.credential_ref.clone();
        }
        *existing = next_profile;
    } else {
        profiles_file.profiles.push(next_profile);
    }

    finish_profile_change(&workspace, &mut profiles_file)
}

// 记录最近一次成功连接时间；不 bump updated_at，避免「连了一下」触发同步冲突
#[tauri::command]
pub async fn mark_profile_connected(
    app: AppHandle,
    profile_id: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
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

// 连接后自动识别出的系统图标；和最近连接时间一样是本机观测结果，不 bump updated_at
#[tauri::command]
pub async fn set_profile_detected_os(
    app: AppHandle,
    profile_id: String,
    detected_os: Option<String>,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;

    if let Some(profile) = profiles_file
        .profiles
        .iter_mut()
        .find(|item| item.id == profile_id && item.deleted_at.is_none())
    {
        profile.detected_os = detected_os;
    }

    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn delete_profile(
    app: AppHandle,
    profile_id: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    soft_delete_profiles(&mut profiles_file, &deleted_at, |profile| {
        profile.id == profile_id
    });

    finish_profile_change(&workspace, &mut profiles_file)
}

// 清理成功前保留墓碑上的凭据引用，失败或进程退出后仍能重试。
fn soft_delete_profiles(
    profiles_file: &mut ProfilesFile,
    deleted_at: &str,
    matches: impl Fn(&ConnectionProfile) -> bool,
) {
    for profile in profiles_file
        .profiles
        .iter_mut()
        .filter(|profile| profile.deleted_at.is_none() && matches(profile))
    {
        profile.deleted_at = Some(deleted_at.to_string());
        profile.updated_at = deleted_at.to_string();
    }
}

fn finish_profile_change(
    workspace: &Path,
    profiles_file: &mut ProfilesFile,
) -> Result<WorkspaceSnapshot, AppError> {
    write_profiles_file(workspace, profiles_file)?;
    let mut snapshot = load_workspace_from_dir(workspace)?;
    // 配置已落盘，清理失败通过快照中保留的引用提示并重试，不误报成保存失败。
    if cleanup_unused_credentials(workspace, profiles_file).is_ok() {
        snapshot.profiles = profiles_file.profiles.clone();
    }
    Ok(snapshot)
}

fn profile_needs_no_credentials(profile: &ConnectionProfile) -> bool {
    profile.deleted_at.is_some()
        || matches!(&profile.auth_method, AuthMethod::Agent | AuthMethod::KeyboardInteractive)
}

// 只清理没有密码/私钥配置继续引用的凭据，成功后才移除用于失败重试的引用。
fn cleanup_unused_credentials(
    workspace: &Path,
    profiles_file: &mut ProfilesFile,
) -> Result<(), AppError> {
    let active_refs: HashSet<String> = profiles_file
        .profiles
        .iter()
        .filter(|profile| !profile_needs_no_credentials(profile))
        .filter_map(|profile| profile.credential_ref.clone())
        .collect();
    let credential_refs: Vec<String> = profiles_file
        .profiles
        .iter()
        .filter(|profile| profile_needs_no_credentials(profile))
        .filter_map(|profile| profile.credential_ref.clone())
        .filter(|reference| !active_refs.contains(reference))
        .collect();
    crate::credentials::remove_credential_records(workspace, &credential_refs)?;
    let mut changed = false;
    for profile in profiles_file
        .profiles
        .iter_mut()
        .filter(|profile| profile_needs_no_credentials(profile))
    {
        changed |= profile.credential_ref.take().is_some();
    }
    if changed {
        write_profiles_file(workspace, profiles_file)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn retry_deleted_credential_cleanup(
    app: AppHandle,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    cleanup_unused_credentials(&workspace, &mut profiles_file).map_err(|_| {
        AppError::new(
            codes::CREDENTIAL_CLEANUP_FAILED,
            "Saved credentials could not be removed",
        )
    })?;
    load_workspace_from_dir(&workspace)
}

// 默认分组承担兜底归属，后端也必须防止其被改名或保存成删除状态。
#[tauri::command]
pub async fn save_group(
    app: AppHandle,
    group: ConnectionGroup,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut next_group = group;
    next_group.name = next_group.name.trim().to_string();
    if next_group.name.is_empty() {
        return Err(AppError::new(
            codes::GROUP_NAME_REQUIRED,
            "A group name is required",
        ));
    }
    if next_group.id == default_group_id() {
        next_group.name = "Ungrouped".to_string();
        next_group.deleted_at = None;
    }
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

// 保留连接时按原顺序追加到默认分组，避免混入已有排序；一并删除时保留凭据清理重试信息。
#[tauri::command]
pub async fn delete_group(
    app: AppHandle,
    group_id: String,
    delete_profiles: bool,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    if group_id == default_group_id() {
        return Err(AppError::new(
            codes::GROUP_DEFAULT_PROTECTED,
            "The default group cannot be deleted",
        ));
    }

    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let deleted_at = now_stamp();

    if let Some(group) = profiles_file
        .groups
        .iter_mut()
        .find(|item| item.id == group_id)
    {
        group.deleted_at = Some(deleted_at.clone());
        group.updated_at = deleted_at.clone();
    }

    if delete_profiles {
        soft_delete_profiles(&mut profiles_file, &deleted_at, |profile| {
            profile.group_id == group_id
        })
    } else {
        let next_order = profiles_file
            .profiles
            .iter()
            .filter(|profile| {
                profile.deleted_at.is_none() && profile.group_id == default_group_id()
            })
            .map(|profile| profile.sort_order)
            .max()
            .unwrap_or(-1)
            + 1;
        let mut moved_profiles: Vec<_> = profiles_file
            .profiles
            .iter_mut()
            .filter(|profile| profile.deleted_at.is_none() && profile.group_id == group_id)
            .collect();
        moved_profiles.sort_by_key(|profile| profile.sort_order);
        for (index, profile) in moved_profiles.into_iter().enumerate() {
            profile.group_id = default_group_id();
            profile.sort_order = next_order + index as i32;
            profile.updated_at = deleted_at.clone();
        }
    }

    finish_profile_change(&workspace, &mut profiles_file)
}

// 一次写入整组顺序，避免逐条保存产生半完成的布局，同时将默认分组固定在首位。
#[tauri::command]
pub async fn reorder_groups(
    app: AppHandle,
    group_ids: Vec<String>,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let mut groups: Vec<_> = profiles_file
        .groups
        .iter_mut()
        .filter(|group| group.deleted_at.is_none())
        .collect();
    groups.sort_by_key(|group| {
        (
            group.id != default_group_id(),
            group_ids
                .iter()
                .position(|id| id == &group.id)
                .unwrap_or(usize::MAX),
            group.sort_order,
        )
    });
    let now = now_stamp();
    for (index, group) in groups.into_iter().enumerate() {
        if group.sort_order != index as i32 {
            group.sort_order = index as i32;
            group.updated_at = now.clone();
        }
    }
    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

// 归属和目标组排序必须同时落盘，避免跨组拖拽中途失败后出现位置与分组不一致。
#[tauri::command]
pub async fn move_profile(
    app: AppHandle,
    profile_id: String,
    group_id: String,
    before_profile_id: Option<String>,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    if !profiles_file
        .groups
        .iter()
        .any(|group| group.id == group_id && group.deleted_at.is_none())
    {
        return Err(AppError::new(
            codes::GROUP_NOT_FOUND,
            "The group no longer exists",
        ));
    }
    let profile = profiles_file
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id && profile.deleted_at.is_none())
        .ok_or_else(|| {
            AppError::new(codes::PROFILE_NOT_FOUND, "The connection no longer exists")
        })?;
    let now = now_stamp();
    profile.group_id = group_id.clone();
    profile.updated_at = now.clone();
    let mut ordered_ids: Vec<_> = profiles_file
        .profiles
        .iter()
        .filter(|profile| {
            profile.deleted_at.is_none() && profile.group_id == group_id && profile.id != profile_id
        })
        .collect();
    ordered_ids.sort_by_key(|profile| profile.sort_order);
    let mut ordered_ids: Vec<String> = ordered_ids
        .into_iter()
        .map(|profile| profile.id.clone())
        .collect();
    let insert_at = before_profile_id
        .as_ref()
        .and_then(|before| ordered_ids.iter().position(|id| id == before))
        .unwrap_or(ordered_ids.len());
    ordered_ids.insert(insert_at, profile_id);
    for profile in profiles_file
        .profiles
        .iter_mut()
        .filter(|profile| profile.deleted_at.is_none() && profile.group_id == group_id)
    {
        if let Some(index) = ordered_ids.iter().position(|id| id == &profile.id) {
            if profile.sort_order != index as i32 {
                profile.sort_order = index as i32;
                profile.updated_at = now.clone();
            }
        }
    }
    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    patch: serde_json::Map<String, serde_json::Value>,
    workspace_dir: String,
) -> Result<WorkspaceSettings, AppError> {
    let _guard = lock_workspace();
    let workspace = expected_workspace_dir(&app, &workspace_dir)?;
    // 只合并调用方改动的顶层字段，null 保留为显式清除背景，未传字段沿用最新存档。
    let mut value = serde_json::to_value(read_settings_file(&workspace)?).map_err(io_error)?;
    let fields = value
        .as_object_mut()
        .ok_or_else(|| invalid_settings("Expected an object"))?;
    for (field, value) in patch {
        if field == "schemaVersion" || !fields.contains_key(&field) {
            return Err(invalid_settings(format!("Unknown setting: {field}")));
        }
        fields.insert(field, value);
    }
    let settings: WorkspaceSettings = serde_json::from_value(value).map_err(invalid_settings)?;
    write_settings_file(&workspace, &settings)?;
    Ok(settings)
}

fn invalid_settings(detail: impl std::fmt::Display) -> AppError {
    AppError::new(codes::SETTINGS_INVALID, "Invalid settings patch").with("detail", detail)
}

// 背景单独更新，防抖保存期间的连接打点、改名和分组调整都应保留。
#[tauri::command]
pub async fn set_profile_background(
    app: AppHandle,
    profile_id: String,
    terminal_background: Option<TerminalBackground>,
    workspace_dir: String,
) -> Result<WorkspaceSnapshot, AppError> {
    let _guard = lock_workspace();
    let workspace = expected_workspace_dir(&app, &workspace_dir)?;
    let mut profiles_file = read_profiles_file(&workspace)?;
    let profile = profiles_file
        .profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id && profile.deleted_at.is_none())
        .ok_or_else(|| AppError::new(codes::PROFILE_NOT_FOUND, "Connection profile not found"))?;
    profile.terminal_background = terminal_background;
    profile.updated_at = now_stamp();
    write_profiles_file(&workspace, &profiles_file)?;
    load_workspace_from_dir(&workspace)
}

fn expected_workspace_dir(app: &AppHandle, expected: &str) -> Result<PathBuf, AppError> {
    let workspace = active_workspace_dir(app)?;
    if workspace != PathBuf::from(expected) {
        return Err(AppError::new(
            codes::WORKSPACE_CHANGED,
            "Workspace changed before saving",
        ));
    }
    Ok(workspace)
}

// —— SSH 隧道配置（tunnels.json，按需创建；软删除同 profiles）——

#[tauri::command]
pub async fn list_tunnel_configs(app: AppHandle) -> Result<Vec<TunnelConfig>, AppError> {
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    Ok(valid_records(
        read_tunnels_file(&workspace)?.tunnels,
        |tunnel| &tunnel.id,
    ))
}

#[tauri::command]
pub async fn save_tunnel_config(
    app: AppHandle,
    tunnel: TunnelConfig,
) -> Result<Vec<TunnelConfig>, AppError> {
    let _guard = lock_workspace();
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
    let _guard = lock_workspace();
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
    let _guard = lock_workspace();
    let workspace = active_workspace_dir(&app)?;
    Ok(valid_records(
        read_snippets_file(&workspace)?.snippets,
        |snippet| &snippet.id,
    ))
}

#[tauri::command]
pub async fn save_snippet(app: AppHandle, snippet: Snippet) -> Result<Vec<Snippet>, AppError> {
    let _guard = lock_workspace();
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
    let _guard = lock_workspace();
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
    let _guard = APP_STATE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
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
    let _guard = APP_STATE_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
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
        "[xin-ssh] {} is unreadable ({error}); {}",
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
