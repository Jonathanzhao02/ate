#![allow(unused_imports)]
use tracing::{info, warn, debug, error, trace, instrument, span, Level};
use error_chain::bail;
use std::{io::stdout, path::Path};
use std::io::Write;
use url::Url;
use std::sync::Arc;
use chrono::Duration;

use ate::prelude::*;
use ate::error::LoadError;
use ate::error::TransformError;
use ate::utils::chain_key_4hex;

use crate::helper::conf_cmd;
use crate::prelude::*;
use crate::model::*;
use crate::request::*;
use crate::service::AuthService;
use crate::helper::*;
use crate::error::*;
use crate::helper::*;

use super::sudo::*;

impl AuthService
{
    pub async fn process_login(self: Arc<Self>, request: LoginRequest) -> Result<LoginResponse, LoginFailed>
    {
        info!("login attempt: {}", request.email);

        // Create the super key and token
        let (super_key, token) = match self.compute_super_key(request.secret) {
            Some(a) => a,
            None => {
                warn!("login attempt denied ({}) - no master key", request.email);
                return Err(LoginFailed::NoMasterKey);
            }
        };

        // Create the super session
        let mut super_session = self.master_session.clone();
        super_session.user.add_read_key(&super_key);

        // Compute which chain the user should exist within
        let chain_key = chain_key_4hex(request.email.as_str(), Some("redo"));
        let chain = self.registry.open(&self.auth_url, &chain_key).await?;
        let dio = chain.dio_full(&super_session).await;

        // Attempt to load the object (if it fails we will tell the caller)
        let user_key = PrimaryKey::from(request.email.clone());
        let mut user = match dio.load::<User>(&user_key).await {
            Ok(a) => a,
            Err(LoadError(LoadErrorKind::NotFound(_), _)) => {
                warn!("login attempt denied ({}) - not found", request.email);
                return Err(LoginFailed::UserNotFound(request.email));
            },
            Err(LoadError(LoadErrorKind::TransformationError(TransformErrorKind::MissingReadKey(_)), _)) => {
                warn!("login attempt denied ({}) - wrong password", request.email);
                return Err(LoginFailed::WrongPassword);
            },
            Err(err) => {
                warn!("login attempt denied ({}) - error - ", err);
                bail!(err);
            }
        };
        
        // Check if the account is locked or not yet verified
        match user.status.clone() {
            UserStatus::Locked(until) => {
                let local_now = chrono::Local::now();
                let utc_now = local_now.with_timezone(&chrono::Utc);
                if until > utc_now {
                    let duration = until - utc_now;
                    warn!("login attempt denied ({}) - account locked until {}", request.email, until);
                    return Err(LoginFailed::AccountLocked(duration.to_std().unwrap()));
                }
            },
            UserStatus::Unverified =>
            {
                match request.verification_code {
                    Some(a) => {
                        if Some(a.to_lowercase()) != user.verification_code.clone().map(|a| a.to_lowercase()) {
                            warn!("login attempt denied ({}) - wrong password", request.email);
                            return Err(LoginFailed::WrongPassword);
                        } else {
                            let mut user = user.as_mut();
                            user.verification_code = None;
                            user.status = UserStatus::Nominal;
                        }
                    },
                    None => {
                        warn!("login attempt denied ({}) - unverified", request.email);
                        return Err(LoginFailed::Unverified(request.email));
                    }
                }
            },
            UserStatus::Nominal => { },
        };
        dio.commit().await?;

        // Add all the authorizations
        let mut session = compute_user_auth(&user);
        session.token = Some(token.clone());

        // Return the session that can be used to access this user
        let user = user.take();
        info!("login attempt accepted ({})", request.email);
        Ok(LoginResponse {
            user_key,
            nominal_read: user.nominal_read,
            nominal_write: user.nominal_write,
            sudo_read: user.sudo_read,
            sudo_write: user.sudo_write,
            authority: session,
            message_of_the_day: None,
        })
    }

    pub(crate) fn master_key(&self) -> Option<EncryptKey>
    {
        self.master_session.user.read_keys().map(|a| a.clone()).next()
    }

    pub fn compute_super_key(&self, secret: EncryptKey) -> Option<(EncryptKey, EncryptedSecureData<EncryptKey>)>
    {
        // Create a session with crypto keys based off the username and password
        let master_key = match self.master_session.user.read_keys().next() {
            Some(a) => a.clone(),
            None => { return None; }
        };
        let super_key = AteHash::from_bytes_twice(master_key.value(), secret.value());
        let super_key = EncryptKey::from_seed_bytes(super_key.to_bytes(), KeySize::Bit192);
        let token = EncryptedSecureData::new(&master_key, super_key).unwrap();
        
        Some((super_key, token))
    }

    pub fn compute_super_key_from_hash(&self, hash: AteHash) -> Option<EncryptKey>
    {
        // Create a session with crypto keys based off the username and password
        let master_key = match self.master_session.user.read_keys().next() {
            Some(a) => a.clone(),
            None => { return None; }
        };
        let super_key = AteHash::from_bytes_twice(master_key.value(), hash.to_bytes());
        let super_key = EncryptKey::from_seed_bytes(super_key.to_bytes(), KeySize::Bit192);
        Some(super_key)
    }
}