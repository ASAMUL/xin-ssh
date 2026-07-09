use std::{
    fs,
    path::{Path, PathBuf},
};

use tauri::{AppHandle, Manager};

use crate::models::{
    AppStateFile, AppStatus, ConnectionGroup, ConnectionProfile, ProfilesFile, WorkspaceSettings,
    WorkspaceSnapshot,
};
use crate::util::{error_message, now_stamp, path_to_string};

const APP_STATE_FILE: &str = "app-state.json";
const PROFILES_FILE: &str = "profiles.json";
const SETTINGS_FILE: &str = "settings.json";
pub(crate) const CURRENT_SCHEMA_VERSION: u16 = 1;

#[tauri::command]
pub async fn get_app_status(app: AppHandle) -> Result<AppStatus, String> {
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
) -> Result<WorkspaceSnapshot, String> {
    let workspace = match workspace_dir {
        Some(dir) if !dir.trim().is_empty() => PathBuf::from(dir),
        _ => default_workspace_dir(&app)?,
    };

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn load_workspace(app: AppHandle) -> Result<Option<WorkspaceSnapshot>, String> {
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
) -> Result<WorkspaceSnapshot, String> {
    let workspace = PathBuf::from(workspace_dir);

    if workspace_files_exist(&workspace) {
        write_app_state(&app, &workspace)?;
        return load_workspace_from_dir(&workspace);
    }

    ensure_workspace(&workspace)?;
    write_app_state(&app, &workspace)?;
    load_workspace_from_dir(&workspace)
}

#[tauri::command]
pub async fn save_profile(
    app: AppHandle,
    profile: ConnectionProfile,
) -> Result<WorkspaceSnapshot, String> {
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

#[tauri::command]
pub async fn delete_profile(app: AppHandle, profile_id: String) -> Result<WorkspaceSnapshot, String> {
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
pub async fn save_group(app: AppHandle, group: ConnectionGroup) -> Result<WorkspaceSnapshot, String> {
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
pub async fn delete_group(app: AppHandle, group_id: String) -> Result<WorkspaceSnapshot, String> {
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
) -> Result<WorkspaceSnapshot, String> {
    let workspace = active_workspace_dir(&app)?;
    write_settings_file(&workspace, &settings)?;
    load_workspace_from_dir(&workspace)
}

fn load_workspace_from_dir(workspace: &Path) -> Result<WorkspaceSnapshot, String> {
    ensure_workspace(workspace)?;
    let settings = read_settings_file(workspace)?;
    let profiles_file = read_profiles_file(workspace)?;

    Ok(WorkspaceSnapshot {
        workspace_dir: path_to_string(workspace),
        settings,
        groups: profiles_file.groups,
        profiles: profiles_file.profiles,
    })
}

fn ensure_workspace(workspace: &Path) -> Result<(), String> {
    fs::create_dir_all(workspace).map_err(error_message)?;

    let profiles_path = workspace.join(PROFILES_FILE);
    if !profiles_path.exists() {
        write_profiles_file(workspace, &default_profiles_file())?;
    }

    let settings_path = workspace.join(SETTINGS_FILE);
    if !settings_path.exists() {
        write_settings_file(workspace, &default_settings())?;
    }

    Ok(())
}

fn workspace_files_exist(workspace: &Path) -> bool {
    workspace.join(PROFILES_FILE).exists() && workspace.join(SETTINGS_FILE).exists()
}

fn read_app_state(app: &AppHandle) -> Result<Option<AppStateFile>, String> {
    let path = app_state_path(app)?;
    if !path.exists() {
        return Ok(None);
    }

    read_json_file(&path).map(Some)
}

fn write_app_state(app: &AppHandle, workspace: &Path) -> Result<(), String> {
    let path = app_state_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(error_message)?;
    }

    write_json_file(
        &path,
        &AppStateFile {
            schema_version: CURRENT_SCHEMA_VERSION,
            workspace_dir: Some(path_to_string(workspace)),
        },
    )
}

pub(crate) fn active_workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let state = read_app_state(app)?.ok_or_else(|| "Workspace is not initialized".to_string())?;
    let workspace_dir = state
        .workspace_dir
        .ok_or_else(|| "Workspace directory is not configured".to_string())?;

    Ok(PathBuf::from(workspace_dir))
}

fn app_state_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join(APP_STATE_FILE))
}

fn default_workspace_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(error_message)?;
    Ok(dir.join("workspace"))
}

fn read_profiles_file(workspace: &Path) -> Result<ProfilesFile, String> {
    read_json_file(&workspace.join(PROFILES_FILE))
}

fn write_profiles_file(workspace: &Path, profiles_file: &ProfilesFile) -> Result<(), String> {
    write_json_file(&workspace.join(PROFILES_FILE), profiles_file)
}

fn read_settings_file(workspace: &Path) -> Result<WorkspaceSettings, String> {
    read_json_file(&workspace.join(SETTINGS_FILE))
}

fn write_settings_file(workspace: &Path, settings: &WorkspaceSettings) -> Result<(), String> {
    write_json_file(&workspace.join(SETTINGS_FILE), settings)
}

pub(crate) fn read_json_file<T>(path: &Path) -> Result<T, String>
where
    T: serde::de::DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(error_message)?;
    serde_json::from_str(&content).map_err(error_message)
}

pub(crate) fn write_json_file<T>(path: &Path, value: &T) -> Result<(), String>
where
    T: serde::Serialize,
{
    let content = serde_json::to_string_pretty(value).map_err(error_message)?;
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content).map_err(error_message)?;

    if path.exists() {
        fs::remove_file(path).map_err(error_message)?;
    }

    fs::rename(temp_path, path).map_err(error_message)
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

fn default_settings() -> WorkspaceSettings {
    WorkspaceSettings {
        schema_version: CURRENT_SCHEMA_VERSION,
        locale: "zh-CN".to_string(),
        terminal_font_family: "Cascadia Code".to_string(),
        monitor_refresh_seconds: 5,
        terminal_theme: crate::models::default_terminal_theme(),
        terminal_font_size: crate::models::default_terminal_font_size(),
    }
}

fn default_group_id() -> String {
    "group-default".to_string()
}
