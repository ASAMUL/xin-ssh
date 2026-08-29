use std::sync::{Arc, Mutex};
use std::time::Duration;

use russh::{
    client,
    keys::{load_secret_key, ssh_key, PrivateKeyWithHashAlg, PublicKey},
};
use tauri::AppHandle;

use crate::error::{codes, connection_error, required, AppError};
use crate::models::AuthMethod;
use crate::ssh::known_hosts;

pub(crate) const SSH_KEEPALIVE_INTERVAL_SECS: u64 = 30;

#[derive(Clone, Default)]
pub(crate) struct ServerKeySnapshot {
    pub(crate) fingerprint: Option<String>,
    pub(crate) algorithm: Option<String>,
}

pub(crate) struct ConnectionTestClient {
    pub(crate) server_key: Arc<Mutex<ServerKeySnapshot>>,
    // Some(fingerprint) 时启用 TOFU 校验：指纹不一致直接拒绝连接（None = 首次连接，记录后信任）
    pub(crate) expected_fingerprint: Option<String>,
}

impl client::Handler for ConnectionTestClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        let fingerprint = server_public_key
            .fingerprint(ssh_key::HashAlg::Sha256)
            .to_string();

        if let Ok(mut snapshot) = self.server_key.lock() {
            snapshot.fingerprint = Some(fingerprint.clone());
            snapshot.algorithm = Some(server_public_key.algorithm().to_string());
        }

        if let Some(expected) = &self.expected_fingerprint {
            return Ok(&fingerprint == expected);
        }

        Ok(true)
    }
}

// 建立 SSH TCP 连接并做 TOFU 指纹校验：已有记录则校验（不一致报 HOST_KEY_CHANGED），
// 首次连接成功后记录指纹。终端与隧道共用。
// keepalive_seconds: None = 默认 30s，Some(0) = 禁用。
pub(crate) async fn connect_with_tofu(
    app: &AppHandle,
    host: &str,
    port: u16,
    keepalive_seconds: Option<u64>,
) -> Result<client::Handle<ConnectionTestClient>, AppError> {
    let expected_fingerprint = known_hosts::lookup_fingerprint(app, host, port)?;
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
        expected_fingerprint: expected_fingerprint.clone(),
    };
    let keepalive = keepalive_seconds.unwrap_or(SSH_KEEPALIVE_INTERVAL_SECS);
    let config = Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: (keepalive > 0).then(|| Duration::from_secs(keepalive)),
        ..Default::default()
    });

    let session = match client::connect(config, (host, port), handler).await {
        Ok(session) => session,
        Err(error) => {
            let snapshot = server_key
                .lock()
                .map(|snapshot| snapshot.clone())
                .unwrap_or_default();

            if let (Some(expected), Some(actual)) = (&expected_fingerprint, &snapshot.fingerprint) {
                if expected != actual {
                    return Err(known_hosts::host_key_changed_error(expected, actual));
                }
            }

            return Err(connection_error(error));
        }
    };

    if expected_fingerprint.is_none() {
        let snapshot = server_key
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default();
        if let Some(fingerprint) = &snapshot.fingerprint {
            let _ = known_hosts::record_host_key(
                app,
                host,
                port,
                fingerprint,
                snapshot.algorithm.as_deref(),
            );
        }
    }

    Ok(session)
}

pub(crate) async fn authenticate_session(
    session: &mut client::Handle<ConnectionTestClient>,
    username: String,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
) -> Result<(), AppError> {
    let auth_result = match auth_method {
        AuthMethod::Password => {
            let password = password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| required("password"))?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| required("privateKeyPath"))?;
            let passphrase = passphrase.as_deref().filter(|value| !value.is_empty());
            let key_pair =
                load_secret_key(&private_key_path, passphrase).map_err(connection_error)?;
            let rsa_hash = session
                .best_supported_rsa_hash()
                .await
                .map_err(connection_error)?
                .flatten();

            session
                .authenticate_publickey(
                    username,
                    PrivateKeyWithHashAlg::new(Arc::new(key_pair), rsa_hash),
                )
                .await
                .map_err(connection_error)?
        }
    };

    if auth_result.success() {
        Ok(())
    } else {
        Err(AppError::new(codes::AUTH_FAILED, "Authentication failed"))
    }
}
