use std::sync::{Arc, Mutex};

use tokio::time::Instant;

use crate::error::{port_invalid, required, AppError};
use crate::models::{ConnectionTestRequest, ConnectionTestResult};
use crate::ssh::client::{
    authenticate_session, connect_transport, disconnect_session, ConnectionTestClient, ServerKeySnapshot,
};

#[tauri::command]
pub async fn test_connection(
    request: ConnectionTestRequest,
) -> Result<ConnectionTestResult, AppError> {
    validate_connection_test_request(&request)?;

    run_connection_test(request).await
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
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    let started = Instant::now();

    let mut session = connect_transport(&host, request.port, None, handler).await?;
    authenticate_session(
        &mut session,
        username,
        request.auth_method,
        request.password,
        request.private_key_path,
        request.passphrase,
        request.agent,
    )
    .await?;
    let latency_ms = started.elapsed().as_millis();
    disconnect_session(&session, "connection test complete").await;

    let server_key = server_key
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default();

    Ok(ConnectionTestResult {
        authenticated: true,
        auth_method,
        latency_ms,
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
