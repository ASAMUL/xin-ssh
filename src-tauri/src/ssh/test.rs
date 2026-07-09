use std::sync::{Arc, Mutex};

use russh::{
    client,
    keys::{load_secret_key, PrivateKeyWithHashAlg},
    Disconnect,
};
use tokio::time::{timeout, Duration, Instant};

use crate::models::{AuthMethod, ConnectionTestRequest, ConnectionTestResult};
use crate::ssh::client::{ConnectionTestClient, ServerKeySnapshot};
use crate::util::connection_error;

#[tauri::command]
pub async fn test_connection(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, String> {
    validate_connection_test_request(&request)?;

    timeout(Duration::from_secs(12), run_connection_test(request))
        .await
        .map_err(|_| "Connection test timed out after 12 seconds".to_string())?
}

async fn run_connection_test(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, String> {
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
                .ok_or_else(|| "Password is required".to_string())?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = request
                .private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Private key path is required".to_string())?;
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
        return Err("Authentication failed".to_string());
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

fn validate_connection_test_request(request: &ConnectionTestRequest) -> Result<(), String> {
    if request.host.trim().is_empty() {
        return Err("Host is required".to_string());
    }

    if request.username.trim().is_empty() {
        return Err("Username is required".to_string());
    }

    if request.port == 0 {
        return Err("Port must be greater than 0".to_string());
    }

    Ok(())
}
