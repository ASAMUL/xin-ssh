mod commands;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::TerminalRegistry::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::change_workspace_dir,
            commands::close_terminal_session,
            commands::delete_group,
            commands::delete_profile,
            commands::get_app_status,
            commands::init_workspace,
            commands::get_remote_system_info,
            commands::list_remote_directory,
            commands::load_workspace,
            commands::read_credential,
            commands::remove_credential,
            commands::resize_terminal,
            commands::save_credential,
            commands::save_group,
            commands::save_profile,
            commands::save_settings,
            commands::send_terminal_input,
            commands::start_download_transfer,
            commands::start_terminal_session,
            commands::start_upload_transfer,
            commands::test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
