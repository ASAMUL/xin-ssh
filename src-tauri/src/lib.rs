mod background;
mod clipboard;
mod credentials;
mod error;
mod models;
mod remote_exec;
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
        .plugin(tauri_plugin_clipboard_manager::init())
        .invoke_handler(tauri::generate_handler![
            sftp::transfer::cancel_transfer,
            sftp::transfer::pause_transfer,
            workspace::change_workspace_dir,
            ssh::shell::close_terminal_session,
            sftp::ops::create_remote_directory,
            sftp::ops::create_remote_file,
            sftp::ops::copy_remote_path,
            sftp::ops::chmod_remote_path,
            workspace::delete_group,
            workspace::delete_profile,
            workspace::delete_snippet,
            workspace::delete_tunnel_config,
            sftp::delete::start_remote_delete,
            sftp::delete::cancel_file_operation,
            remote_exec::disk_usage::start_disk_usage,
            remote_exec::archive::start_remote_compress,
            remote_exec::archive::start_remote_extract,
            sftp::partials::discard_partial_files,
            workspace::get_app_status,
            workspace::init_workspace,
            system_info::get_remote_system_info,
            system_info::measure_session_latency,
            background::import_terminal_background,
            sftp::ops::list_remote_directory,
            workspace::list_snippets,
            workspace::list_tunnel_configs,
            workspace::mark_profile_connected,
            workspace::move_profile,
            workspace::reorder_profiles,
            workspace::reorder_groups,
            workspace::retry_deleted_credential_cleanup,
            sftp::ops::stream_remote_directory,
            workspace::load_workspace,
            workspace::read_ssh_config,
            workspace::export_profiles,
            credentials::read_credential,
            sftp::ops::read_remote_file_text,
            sftp::ops::resolve_remote_home,
            sftp::ops::stat_remote_path,
            background::read_terminal_background,
            credentials::remove_credential,
            sftp::ops::rename_remote_path,
            ssh::shell::resize_terminal,
            credentials::save_credential,
            workspace::save_group,
            workspace::save_profile,
            workspace::save_settings,
            workspace::set_profile_background,
            workspace::set_profile_detected_os,
            workspace::save_snippet,
            workspace::save_tunnel_config,
            ssh::shell::send_terminal_input,
            sftp::transfer::start_download_transfer,
            ssh::shell::start_terminal_session,
            ssh::tunnel::start_tunnel,
            ssh::tunnel::stop_tunnel,
            ssh::forward::forward_terminal_port,
            ssh::forward::stop_terminal_forward,
            sftp::transfer::start_upload_transfer,
            ssh::known_hosts::trust_host_key,
            ssh::test::test_connection,
            ssh::agent::list_agent_identities,
            ssh::interactive::respond_authentication,
            sftp::ops::write_remote_file_text,
            sftp::ops::write_remote_file_bytes,
            clipboard::read_clipboard,
            clipboard::read_clipboard_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
