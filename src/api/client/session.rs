use std::time::Duration;

use axum::{Json, extract::State};
use axum_client_ip::ClientIp;
use conduwuit::{
	Err, Error, Result, debug, err, info,
	utils::{self, ReadyExt, hash, stream::BroadbandExt},
	warn,
};
use conduwuit_core::{debug_error, debug_warn};
use conduwuit_service::Services;
use futures::StreamExt;
use lettre::{Address, message::Mailbox};
use ruma::{
	OwnedClientSecret, OwnedDeviceId, OwnedSessionId, OwnedUserId, UserId,
	api::client::{
		error::ErrorKind,
		session::{get_login_token, logout, logout_all},
	},
};
use serde::{Deserialize, Serialize};
use service::mailer::messages;
use service::uiaa::Identity;
use serde_json::json;

use super::{DEVICE_ID_LENGTH, TOKEN_LENGTH};
use crate::Ruma;

#[derive(Debug, Serialize)]
pub(crate) struct LoginResponse {
	pub user_id: OwnedUserId,
	pub access_token: String,
	pub device_id: Option<OwnedDeviceId>,
	pub home_server: Option<String>,
	pub refresh_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct LoginEmailRequestTokenResponse {
	sid: String,
	email: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginEmailRequestTokenRequest {
	client_secret: String,
	login: Option<String>,
	password: Option<String>,
	send_attempt: Option<usize>,
	sid: Option<String>,
	token: Option<String>,
	device_id: Option<OwnedDeviceId>,
	initial_device_display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginEmailSubmitTokenRequest {
	client_secret: String,
	sid: String,
	token: String,
	device_id: Option<OwnedDeviceId>,
	initial_device_display_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum LoginFlowResponse {
	Request(LoginEmailRequestTokenResponse),
	Final(LoginResponse),
}

/// # `GET /_matrix/client/v3/login`
///
/// Returns the homeserver's supported custom login flow.
pub(crate) async fn get_login_types_route() -> Result<Json<serde_json::Value>> {
	Ok(Json(json!({
		"flows": [
			{ "stages": ["password", "email_code"] }
		]
	})))
}

async fn email_for_login(services: &Services, user_id: &UserId) -> Result<Address> {
	if let Some(email) = services
		.threepid
		.get_email_for_localpart(user_id.localpart())
		.await
	{
		return Ok(email);
	}

	return Err!(Request(Forbidden("This account does not have an email address.")));
}

/// Authenticates the given user by its ID and its password.
///
/// Returns the user ID if successful, and an error otherwise.
#[tracing::instrument(skip_all, fields(%user_id), name = "password", level = "debug")]
pub(crate) async fn password_login(
	services: &Services,
	user_id: &UserId,
	lowercased_user_id: &UserId,
	password: &str,
) -> Result<OwnedUserId> {
	// Restrict login to accounts only of type 'password', including untyped
	// legacy accounts which are equivalent to 'password'.
	if services
		.users
		.origin(user_id)
		.await
		.is_ok_and(|origin| origin != "password")
	{
		return Err!(Request(Forbidden("Account does not permit password login.")));
	}

	let (hash, user_id) = match services.users.password_hash(user_id).await {
		| Ok(hash) => (hash, user_id),
		| Err(_) => services
			.users
			.password_hash(lowercased_user_id)
			.await
			.map(|hash| (hash, lowercased_user_id))
			.map_err(|_| err!(Request(Forbidden("Invalid identifier or password."))))?,
	};

	if hash.is_empty() {
		return Err!(Request(UserDeactivated("The user has been deactivated")));
	}

	hash::verify_password(password, &hash)
		.inspect_err(|e| debug_error!("{e}"))
		.map_err(|_| err!(Request(Forbidden("Invalid identifier or password."))))?;

	Ok(user_id.to_owned())
}

/// Authenticates the given user through the configured LDAP server.
///
/// Creates the user if the user is found in the LDAP and do not already have an
/// account.
#[tracing::instrument(skip_all, fields(%user_id), name = "ldap", level = "debug")]
pub(super) async fn ldap_login(
	services: &Services,
	user_id: &UserId,
	lowercased_user_id: &UserId,
	password: &str,
) -> Result<OwnedUserId> {
	let (user_dn, is_ldap_admin) = match services.config.ldap.bind_dn.as_ref() {
		| Some(bind_dn) if bind_dn.contains("{username}") =>
			(bind_dn.replace("{username}", lowercased_user_id.localpart()), None),
		| _ => {
			debug!("Searching user in LDAP");

			let dns = services.users.search_ldap(user_id).await?;
			if dns.len() >= 2 {
				return Err!(Ldap("LDAP search returned two or more results"));
			}

			let Some((user_dn, is_admin)) = dns.first() else {
				return password_login(services, user_id, lowercased_user_id, password).await;
			};

			(user_dn.clone(), *is_admin)
		},
	};

	let user_id = services
		.users
		.auth_ldap(&user_dn, password)
		.await
		.map(|()| lowercased_user_id.to_owned())?;

	// LDAP users are automatically created on first login attempt. This is a very
	// common feature that can be seen on many services using a LDAP provider for
	// their users (synapse, Nextcloud, Jellyfin, ...).
	//
	// LDAP users are crated with a dummy password but non empty because an empty
	// password is reserved for deactivated accounts. The conduwuit password field
	// will never be read to login a LDAP user so it's not an issue.
	if !services.users.exists(lowercased_user_id).await {
		services
			.users
			.create(lowercased_user_id, Some("*"), Some("ldap"))
			.await?;
	}

	// Only sync admin status if LDAP can actually determine it.
	// None means LDAP cannot determine admin status (manual config required).
	if let Some(is_ldap_admin) = is_ldap_admin {
		let is_conduwuit_admin = services.admin.user_is_admin(lowercased_user_id).await;

		if is_ldap_admin && !is_conduwuit_admin {
			Box::pin(services.admin.make_user_admin(lowercased_user_id)).await?;
		} else if !is_ldap_admin && is_conduwuit_admin {
			Box::pin(services.admin.revoke_admin(lowercased_user_id)).await?;
		}
	}

	Ok(user_id)
}

pub(crate) async fn handle_login(
	services: &Services,
	login: &str,
	password: &str,
) -> Result<OwnedUserId> {
	debug!("Got password login type");
	let user_id_or_localpart = if let Ok(email) = Address::try_from(login.to_owned()) {
		services
			.threepid
			.get_localpart_for_email(<Address as AsRef<str>>::as_ref(&email))
			.await
			.ok_or_else(|| err!(Request(Forbidden("Invalid identifier or password"))))?
	} else if let Ok(user_id) = UserId::parse_with_server_name(login, &services.config.server_name)
	{
		user_id.localpart().to_owned()
	} else {
		return Err!(Request(InvalidParam("Identifier type not recognized")));
	};

	let user_id =
		UserId::parse_with_server_name(user_id_or_localpart, &services.config.server_name)
			.map_err(|_| err!(Request(InvalidUsername("User ID is malformed"))))?;

	let user_id = resolve_login_user_id(services, &user_id).await;

	if !services.globals.user_is_local(&user_id) {
		return Err!(Request(Unknown("User ID does not belong to this homeserver")));
	}

	if services.users.is_locked(&user_id).await? {
		return Err(Error::BadRequest(ErrorKind::UserLocked, "This account has been locked."));
	}

	if services.users.is_login_disabled(&user_id).await {
		warn!(%user_id, "user attempted to log in with a login-disabled account");
		return Err!(Request(Forbidden("This account is not permitted to log in.")));
	}

	if cfg!(feature = "ldap") && services.config.ldap.enable {
		match Box::pin(ldap_login(services, &user_id, &user_id, password)).await {
			| Ok(user_id) => Ok(user_id),
			| Err(err) if services.config.ldap.ldap_only => Err(err),
			| Err(err) => {
				debug_warn!("{err}");
				password_login(services, &user_id, &user_id, password).await
			},
		}
	} else {
		password_login(services, &user_id, &user_id, password).await
	}
}

async fn resolve_login_user_id(services: &Services, user_id: &UserId) -> OwnedUserId {
	let lowercased_user_id = UserId::parse_with_server_name(
		user_id.localpart().to_lowercase(),
		&services.config.server_name,
	)
	.unwrap();

	if services.users.exists(&lowercased_user_id).await {
		lowercased_user_id.to_owned()
	} else if services.users.exists(user_id).await {
		user_id.to_owned()
	} else {
		user_id.to_owned()
	}
}

/// # `POST /_matrix/client/v3/login`
///
/// Handles the custom password + email code login flow.
#[tracing::instrument(skip_all, fields(%client), name = "login", level = "info")]
pub(crate) async fn login_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	Json(body): Json<LoginEmailRequestTokenRequest>,
) -> Result<Json<LoginFlowResponse>> {
	if let (Some(sid), Some(token)) = (body.sid.as_ref(), body.token.as_ref()) {
		let sid = sid
			.parse::<OwnedSessionId>()
			.map_err(|_| err!(Request(InvalidParam("Invalid sid"))))?;
		let client_secret = body
			.client_secret
			.parse::<OwnedClientSecret>()
			.map_err(|_| err!(Request(InvalidParam("Invalid client_secret"))))?;
		let response = finish_login_after_email_code(
			&services,
			client.to_string(),
			LoginEmailSubmitTokenRequest {
				client_secret: client_secret.to_string(),
				sid: sid.to_string(),
				token: token.clone(),
				device_id: body.device_id,
				initial_device_display_name: body.initial_device_display_name,
			},
		)
		.await?;

		return Ok(Json(LoginFlowResponse::Final(response)));
	}

	if body.sid.is_some() || body.token.is_some() {
		return Err!(Request(InvalidParam(
			"Both sid and token are required to finish login"
		)));
	}

	if !services.threepid.email_requirement().may_change() {
		return Err!(Request(Forbidden("Email verification is unavailable.")));
	}

	let login = body
		.login
		.as_deref()
		.ok_or_else(|| err!(Request(InvalidParam("Login is required"))))?;
	let password = body
		.password
		.as_deref()
		.ok_or_else(|| err!(Request(InvalidParam("Password is required"))))?;
	let send_attempt = body
		.send_attempt
		.ok_or_else(|| err!(Request(InvalidParam("send_attempt is required"))))?;

	let user_id = handle_login(&services, login, password).await?;
	let email = email_for_login(&services, &user_id).await?;

	let session = services
		.threepid
		.send_validation_code_email(
			Mailbox::new(None, email.clone()),
			|verification_code| messages::LoginCode {
				user_id: &user_id,
				verification_code,
			},
			&body
				.client_secret
				.parse::<OwnedClientSecret>()
				.map_err(|_| err!(Request(InvalidParam("Invalid client_secret"))))?,
			send_attempt,
		)
		.await?;

	info!("{user_id} started login verification from IP {client}");

	Ok(Json(LoginFlowResponse::Request(LoginEmailRequestTokenResponse {
		sid: session.to_string(),
		email: email.to_string(),
	})))
}

#[tracing::instrument(skip_all, fields(%client), name = "login_email_submit", level = "info")]
pub(crate) async fn finish_login_after_email_code(
	services: &Services,
	client: String,
	body: LoginEmailSubmitTokenRequest,
) -> Result<LoginResponse> {
	let sid = body
		.sid
		.parse::<OwnedSessionId>()
		.map_err(|_| err!(Request(InvalidParam("Invalid sid"))))?;
	let client_secret = body
		.client_secret
		.parse::<OwnedClientSecret>()
		.map_err(|_| err!(Request(InvalidParam("Invalid client_secret"))))?;

	services
		.threepid
		.try_validate_session(&sid, &body.token)
		.await
		.map_err(|message| err!(Request(ThreepidAuthFailed("{message}"))))?;

	let email = services
		.threepid
		.consume_valid_session(&sid, &client_secret)
		.await
		.map_err(|message| err!(Request(ThreepidAuthFailed("{message}"))))?;

	let Some(localpart) = services
		.threepid
		.get_localpart_for_email(<Address as AsRef<str>>::as_ref(&email))
		.await
	else {
		return Err!(Request(Forbidden("This account is not associated with a user.")));
	};

	let user_id = UserId::parse_with_server_name(localpart, &services.config.server_name)
		.map_err(|_| err!(Request(InvalidUsername("User ID is malformed"))))?;

	if services.users.is_locked(&user_id).await? {
		return Err(Error::BadRequest(ErrorKind::UserLocked, "This account has been locked."));
	}

	if services.users.is_login_disabled(&user_id).await {
		warn!(%user_id, "user attempted to log in with a login-disabled account");
		return Err!(Request(Forbidden("This account is not permitted to log in.")));
	}

	let response = complete_login(
		services,
		client,
		user_id,
		body.device_id,
		body.initial_device_display_name,
	)
	.await?;

	info!("{} completed login verification", response.user_id);

	Ok(response)
}

async fn complete_login(
	services: &Services,
	client: String,
	user_id: OwnedUserId,
	device_id: Option<OwnedDeviceId>,
	initial_device_display_name: Option<String>,
) -> Result<LoginResponse> {
	let device_exists = if let Some(ref provided_device_id) = device_id {
		services
			.users
			.all_device_ids(&user_id)
			.ready_any(|v| v == provided_device_id)
			.await
	} else {
		false
	};

	let device_id = device_id.unwrap_or_else(|| utils::random_string(DEVICE_ID_LENGTH).into());
	let token = services.users.generate_unique_token().await;

	if device_exists {
		services.users.set_token(&user_id, &device_id, &token).await?;
	} else {
		services
			.users
			.create_device(
				&user_id,
				&device_id,
				&token,
				initial_device_display_name,
				Some(client.clone()),
			)
			.await?;
	}

	Ok(LoginResponse {
		user_id,
		access_token: token,
		device_id: Some(device_id),
		home_server: Some(services.config.server_name.to_string()),
		refresh_token: None,
	})
}

/// # `POST /_matrix/client/v1/login/get_token`
///
/// Allows a logged-in user to get a short-lived token which can be used
/// to log in with the m.login.token flow.
///
/// <https://spec.matrix.org/v1.13/client-server-api/#post_matrixclientv1loginget_token>
#[tracing::instrument(skip_all, fields(%client), name = "login_token", level = "info")]
pub(crate) async fn login_token_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	body: Ruma<get_login_token::v1::Request>,
) -> Result<get_login_token::v1::Response> {
	if !services.server.config.login_via_existing_session {
		return Err!(Request(Forbidden("Login via an existing session is not enabled")));
	}

	let sender_user = body.sender_user();

	// Prompt the user to confirm with their password using UIAA
	let _ = services
		.uiaa
		.authenticate_password(&body.auth, Some(Identity::from_user_id(sender_user)))
		.await?;

	let login_token = utils::random_string(TOKEN_LENGTH);
	let expires_in = services.users.create_login_token(sender_user, &login_token);

	Ok(get_login_token::v1::Response {
		expires_in: Duration::from_millis(expires_in),
		login_token,
	})
}

/// # `POST /_matrix/client/v3/logout`
///
/// Log out the current device.
///
/// - Invalidates access token
/// - Deletes device metadata (device id, device display name, last seen ip,
///   last seen ts)
/// - Forgets to-device events
/// - Triggers device list updates
#[tracing::instrument(skip_all, fields(%client), name = "logout", level = "info")]
pub(crate) async fn logout_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	body: Ruma<logout::v3::Request>,
) -> Result<logout::v3::Response> {
	let (sender_user, sender_device) = body.sender();
	services
		.users
		.remove_device(sender_user, sender_device)
		.await;
	services
		.pusher
		.get_pushkeys(sender_user)
		.map(ToOwned::to_owned)
		.broad_filter_map(async |pushkey| {
			services
				.pusher
				.get_pusher_device(&pushkey)
				.await
				.ok()
				.as_ref()
				.is_some_and(|pusher_device| pusher_device == sender_device)
				.then_some(pushkey)
		})
		.for_each(async |pushkey| {
			services.pusher.delete_pusher(sender_user, &pushkey).await;
		})
		.await;

	Ok(logout::v3::Response::new())
}

/// # `POST /_matrix/client/r0/logout/all`
///
/// Log out all devices of this user.
///
/// - Invalidates all access tokens
/// - Deletes all device metadata (device id, device display name, last seen ip,
///   last seen ts)
/// - Forgets all to-device events
/// - Triggers device list updates
///
/// Note: This is equivalent to calling [`GET
/// /_matrix/client/r0/logout`](fn.logout_route.html) from each device of this
/// user.
#[tracing::instrument(skip_all, fields(%client), name = "logout", level = "info")]
pub(crate) async fn logout_all_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	body: Ruma<logout_all::v3::Request>,
) -> Result<logout_all::v3::Response> {
	let sender_user = body.sender_user();
	services
		.users
		.all_device_ids(sender_user)
		.for_each(|device_id| services.users.remove_device(sender_user, device_id))
		.await;
	services
		.pusher
		.get_pushkeys(sender_user)
		.for_each(async |pushkey| {
			services.pusher.delete_pusher(sender_user, pushkey).await;
		})
		.await;

	Ok(logout_all::v3::Response::new())
}
