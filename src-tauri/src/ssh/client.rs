use std::sync::{Arc, Mutex};

use russh::{
    client,
    keys::{load_secret_key, ssh_key, PrivateKeyWithHashAlg, PublicKey},
};

use crate::models::AuthMethod;
use crate::util::connection_error;

#[derive(Clone, Default)]
pub(crate) struct ServerKeySnapshot {
    pub(crate) fingerprint: Option<String>,
    pub(crate) algorithm: Option<String>,
}

pub(crate) struct ConnectionTestClient {
    pub(crate) server_key: Arc<Mutex<ServerKeySnapshot>>,
}

impl client::Handler for ConnectionTestClient {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        if let Ok(mut snapshot) = self.server_key.lock() {
            snapshot.fingerprint = Some(
                server_public_key
                    .fingerprint(ssh_key::HashAlg::Sha256)
                    .to_string(),
            );
            snapshot.algorithm = Some(server_public_key.algorithm().to_string());
        }

        Ok(true)
    }
}

pub(crate) async fn authenticate_session(
    session: &mut client::Handle<ConnectionTestClient>,
    username: String,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key_path: Option<String>,
    passphrase: Option<String>,
) -> Result<(), String> {
    let auth_result = match auth_method {
        AuthMethod::Password => {
            let password = password
                .filter(|value| !value.is_empty())
                .ok_or_else(|| "Password is required".to_string())?;

            session
                .authenticate_password(username, password)
                .await
                .map_err(connection_error)?
        }
        AuthMethod::PrivateKey => {
            let private_key_path = private_key_path
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| "Private key path is required".to_string())?;
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
        Err("Authentication failed".to_string())
    }
}
