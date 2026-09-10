mod background;
mod credentials;
mod error;
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
        .manage(ssh::interactive::AuthenticationRegistry::default())
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
            sftp::delete::start_remote_delete,
            sftp::delete::cancel_remote_delete,
            workspace::get_app_status,
            workspace::init_workspace,
            system_info::get_remote_system_info,
            background::import_terminal_background,
            sftp::ops::list_remote_directory,
            workspace::list_snippets,
            workspace::list_tunnel_configs,
            workspace::mark_profile_connected,
            workspace::move_profile,
            workspace::reorder_groups,
            workspace::retry_deleted_credential_cleanup,
            sftp::ops::stream_remote_directory,
            workspace::load_workspace,
            credentials::read_credential,
            sftp::ops::read_remote_file_text,
            background::read_terminal_background,
            credentials::remove_credential,
            sftp::ops::rename_remote_path,
            ssh::shell::resize_terminal,
            credentials::save_credential,
            workspace::save_group,
            workspace::save_profile,
            workspace::save_settings,
            workspace::set_profile_background,
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
            ssh::agent::list_agent_identities,
            ssh::interactive::respond_authentication,
            sftp::ops::write_remote_file_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
