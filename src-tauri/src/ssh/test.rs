use tokio::time::Instant;
use tauri::{ipc::Channel, AppHandle};

use crate::error::{port_invalid, required, AppError};
use crate::models::{ConnectionTestRequest, ConnectionTestResult};
use crate::ssh::client::{
    authenticate_session, connect_verified, disconnect_session,
};
use crate::ssh::interactive::{AuthenticationContext, AuthenticationEvent};

#[tauri::command]
pub async fn test_connection(
    app: AppHandle,
    request: ConnectionTestRequest,
    authentication: Channel<AuthenticationEvent>,
) -> Result<ConnectionTestResult, AppError> {
    let authentication = AuthenticationContext::new(&app, authentication);
    validate_connection_test_request(&request)?;

    run_connection_test(&app, request, authentication).await
}

async fn run_connection_test(
    app: &AppHandle,
    request: ConnectionTestRequest,
    authentication: AuthenticationContext,
) -> Result<ConnectionTestResult, AppError> {
    let auth_method = request.auth_method.clone();
    let host = request.host.trim().to_string();
    let username = request.username.trim().to_string();
    let started = Instant::now();

    let (mut session, server_key) = connect_verified(app, &host, request.port, None).await?;
    authenticate_session(
        &mut session,
        username,
        request.auth_method,
        request.password,
        request.private_key_path,
        request.passphrase,
        request.agent,
        authentication,
    )
    .await?;
    let latency_ms = started.elapsed().as_millis();
    disconnect_session(&session, "connection test complete").await;

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
