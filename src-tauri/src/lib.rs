mod commands;
mod models;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_stronghold::Builder::new(|password| {
                use argon2::{hash_raw, Config, Variant, Version};

                let config = Config {
                    lanes: 4,
                    mem_cost: 10_000,
                    time_cost: 10,
                    variant: Variant::Argon2id,
                    version: Version::Version13,
                    ..Default::default()
                };
                let salt = b"xin-terminal-stronghold-v1";

                hash_raw(password.as_ref(), salt, &config).expect("failed to derive vault key")
            })
            .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::archive_credential_store,
            commands::change_workspace_dir,
            commands::delete_group,
            commands::delete_profile,
            commands::get_device_vault_password,
            commands::get_app_status,
            commands::init_workspace,
            commands::load_workspace,
            commands::save_group,
            commands::save_profile,
            commands::save_settings,
            commands::test_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
