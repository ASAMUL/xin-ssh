mod credentials;
mod models;
mod sftp;
mod ssh;
mod system_info;
mod util;
mod workspace;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ssh::registry::TerminalRegistry::default())
        .manage(ssh::tunnel::TunnelRegistry::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            sftp::transfer::cancel_transfer,
            workspace::change_workspace_dir,
            ssh::shell::close_terminal_session,
            sftp::ops::create_remote_directory,
            sftp::ops::create_remote_file,
            workspace::delete_group,
            workspace::delete_profile,
            workspace::delete_snippet,
            workspace::delete_tunnel_config,
            sftp::ops::delete_remote_entries,
            workspace::get_app_status,
            workspace::init_workspace,
            system_info::get_remote_system_info,
            sftp::ops::list_remote_directory,
            workspace::list_snippets,
            workspace::list_tunnel_configs,
            workspace::mark_profile_connected,
            sftp::ops::stream_remote_directory,
            workspace::load_workspace,
            credentials::read_credential,
            sftp::ops::read_remote_file_text,
            credentials::remove_credential,
            sftp::ops::rename_remote_path,
            ssh::shell::resize_terminal,
            credentials::save_credential,
            workspace::save_group,
            workspace::save_profile,
            workspace::save_settings,
            workspace::save_snippet,
            workspace::save_tunnel_config,
            ssh::shell::send_terminal_input,
            sftp::transfer::start_download_transfer,
            ssh::shell::start_terminal_session,
            ssh::tunnel::start_tunnel,
            ssh::tunnel::stop_tunnel,
            sftp::transfer::start_upload_transfer,
            ssh::known_hosts::trust_host_key,
            ssh::test::test_connection,
            sftp::ops::write_remote_file_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
