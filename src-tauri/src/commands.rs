use crate::models::{
    ConnectionProfile, RemoteFileEntry, SessionInfo, TerminalSnapshot, TransferTask,
};

#[tauri::command]
pub fn list_connection_profiles() -> Vec<ConnectionProfile> {
    mock_connections()
}

#[tauri::command]
pub fn open_session(connection_id: String) -> Result<SessionInfo, String> {
    let connection = mock_connections()
        .into_iter()
        .find(|profile| profile.id == connection_id)
        .ok_or_else(|| format!("Connection profile not found: {connection_id}"))?;

    Ok(SessionInfo {
        id: connection.id.clone(),
        connection_id: connection.id,
        name: connection.name,
        remote_path: "/var/www/html".to_string(),
        connected: true,
    })
}

#[tauri::command]
pub fn list_remote_directory(
    session_id: String,
    path: String,
) -> Result<Vec<RemoteFileEntry>, String> {
    if session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    if path.trim().is_empty() {
        return Err("Remote path is required".to_string());
    }

    Ok(mock_remote_files(path))
}

#[tauri::command]
pub fn list_transfer_tasks() -> Vec<TransferTask> {
    vec![
        TransferTask {
            id: "upload-1".to_string(),
            direction: "upload".to_string(),
            label: "Uploading 2 files".to_string(),
            speed: "12.0 MB/s".to_string(),
            progress: 68,
        },
        TransferTask {
            id: "download-1".to_string(),
            direction: "download".to_string(),
            label: "Downloading 1 file".to_string(),
            speed: "1.2 MB/s".to_string(),
            progress: 42,
        },
    ]
}

#[tauri::command]
pub fn terminal_snapshot(session_id: String) -> Result<TerminalSnapshot, String> {
    if session_id.trim().is_empty() {
        return Err("Session id is required".to_string());
    }

    Ok(TerminalSnapshot {
        session_id,
        pty_id: "pty-prod-web-1".to_string(),
        lines: vec![
            "root@192.168.1.10:~# ls -la /var/www/html".to_string(),
            "total 48".to_string(),
            "drwxr-xr-x 5 root root 4096 May 12 10:24 .".to_string(),
            "drwxr-xr-x 2 root root 4096 May 12 10:24 assets".to_string(),
            "drwxr-xr-x 2 root root 4096 May 12 10:24 images".to_string(),
            "-rw-r--r-- 1 root root 2450 May 12 09:41 index.html".to_string(),
        ],
    })
}

fn mock_connections() -> Vec<ConnectionProfile> {
    vec![
        ConnectionProfile {
            id: "prod-web".to_string(),
            name: "Production Web".to_string(),
            host: "192.168.1.10".to_string(),
            username: "root".to_string(),
            port: 22,
            status: "online".to_string(),
        },
        ConnectionProfile {
            id: "db-server".to_string(),
            name: "DB Server".to_string(),
            host: "192.168.1.20".to_string(),
            username: "ubuntu".to_string(),
            port: 22,
            status: "online".to_string(),
        },
        ConnectionProfile {
            id: "dev-server".to_string(),
            name: "Dev Server".to_string(),
            host: "dev.local".to_string(),
            username: "root".to_string(),
            port: 22,
            status: "offline".to_string(),
        },
    ]
}

fn mock_remote_files(path: String) -> Vec<RemoteFileEntry> {
    let entries = [
        ("assets", "folder", None, "drwxr-xr-x", "May 12, 2025 10:24"),
        ("images", "folder", None, "drwxr-xr-x", "May 12, 2025 10:24"),
        (
            "index.html",
            "file",
            Some(2_450),
            "-rw-r--r--",
            "May 12, 2025 09:41",
        ),
        (
            "styles.css",
            "file",
            Some(12_485),
            "-rw-r--r--",
            "May 12, 2025 09:41",
        ),
        (
            "app.js",
            "file",
            Some(8_623),
            "-rw-r--r--",
            "May 12, 2025 09:41",
        ),
        (
            "config.json",
            "file",
            Some(1_210),
            "-rw-r--r--",
            "May 12, 2025 09:41",
        ),
    ];

    entries
        .into_iter()
        .map(
            |(name, kind, size, permissions, modified)| RemoteFileEntry {
                id: name.replace('.', "-"),
                name: name.to_string(),
                path: format!("{}/{}", path.trim_end_matches('/'), name),
                kind: kind.to_string(),
                size,
                permissions: permissions.to_string(),
                modified: modified.to_string(),
            },
        )
        .collect()
}
