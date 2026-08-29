use std::sync::{Arc, Mutex};

use russh::{
    client,
    keys::{load_secret_key, PrivateKeyWithHashAlg},
    Disconnect,
};
use tokio::time::{timeout, Duration, Instant};

use crate::error::{codes, connection_error, port_invalid, required, timeout as timeout_error, AppError};
use crate::models::{AuthMethod, ConnectionTestRequest, ConnectionTestResult};
use crate::ssh::client::{ConnectionTestClient, ServerKeySnapshot};

const CONNECTION_TEST_TIMEOUT_SECONDS: u64 = 12;

#[tauri::command]
pub async fn test_connection(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, AppError> {
    validate_connection_test_request(&request)?;

    timeout(
        Duration::from_secs(CONNECTION_TEST_TIMEOUT_SECONDS),
        run_connection_test(request),
    )
    .await
    .map_err(|_| timeout_error("testConnection", CONNECTION_TEST_TIMEOUT_SECONDS))?
}

async fn run_connection_test(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, AppError> {
    let auth_method = request.auth_method.clone();
    let server_key = Arc::new(Mutex::new(ServerKeySnapshot::default()));
    let handler = ConnectionTestClient {
        server_key: Arc::clone(&server_key),
        // 连接测试只上报指纹信息，不做 TOFU 拦截
        expected_fingerprint: None,
    };
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(12)),
        ..Default::default()
    });
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    let started = Instant::now();

    let mut session = client::connect(config, (host.as_str(), request.port), handler)
        .await
        .map_err(connection_error)?;

    let auth_result = match request.auth_method {
        AuthMethod::Password => {
            let password = request
                .password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| required("password"))?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = request
                .private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| required("privateKeyPath"))?;
            let passphrase = request
                .passphrase
                .as_deref()
                .filter(|value| !value.is_empty());
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

    if !auth_result.success() {
        return Err(AppError::new(codes::AUTH_FAILED, "Authentication failed"));
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "connection test complete", "en")
        .await;

    let server_key = server_key
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default();

    Ok(ConnectionTestResult {
        authenticated: true,
        auth_method,
        latency_ms: started.elapsed().as_millis(),
        server_fingerprint: server_key.fingerprint,
        server_key_algorithm: server_key.algorithm,
    })
}

fn validate_connection_test_request(request: &ConnectionTestRequest) -> Result<(), AppError> {
    if request.host.trim().is_empty() {
        return Err(required("host"));
    }

    if request.username.trim().is_empty() {
        return Err(required("username"));
    }

    if request.port == 0 {
        return Err(port_invalid());
    }

    Ok(())
}
