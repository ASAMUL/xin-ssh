mod commands;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::TerminalRegistry::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_stronghold::Builder::new(|password| {
                use ring::digest::{digest, SHA256};

                let mut material = Vec::with_capacity(
                    b"xin-terminal-stronghold-v1:".len() + password.len(),
                );
                material.extend_from_slice(b"xin-terminal-stronghold-v1:");
                material.extend_from_slice(password.as_bytes());

                digest(&SHA256, &material).as_ref().to_vec()
            })
            .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::archive_credential_store,
            commands::change_workspace_dir,
            commands::close_terminal_session,
            commands::delete_group,
            commands::delete_profile,
            commands::get_device_vault_password,
            commands::get_app_status,
            commands::init_workspace,
            commands::load_workspace,
            commands::resize_terminal,
            commands::save_group,
            commands::save_profile,
            commands::save_settings,
            commands::send_terminal_input,
            commands::start_terminal_session,
            commands::test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
