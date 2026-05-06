use std::fmt::Write;

use axum::{Json, extract::State};
use axum_client_ip::ClientIp;
use conduwuit::{Err, Result, debug_info, err, utils};
use conduwuit_core::debug_warn;
use conduwuit_service::Services;
use lettre::{Address, message::Mailbox};
use ruma::{OwnedClientSecret, OwnedDeviceId, OwnedSessionId, OwnedUserId, UserId};
use ruma::push::Ruleset;
use serde::{Deserialize, Serialize};
use service::mailer::messages;

use super::{DEVICE_ID_LENGTH, TOKEN_LENGTH};
use crate::Ruma;

const RANDOM_USER_ID_LENGTH: usize = 10;

#[derive(Debug, Deserialize)]
pub(crate) struct RegistrationEmailRequestTokenRequest {
	pub client_secret: OwnedClientSecret,
	pub email: String,
	pub send_attempt: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistrationEmailRequestTokenResponse {
	pub sid: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegistrationEmailSubmitTokenRequest {
	pub client_secret: OwnedClientSecret,
	pub sid: OwnedSessionId,
	pub token: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RegistrationEmailSubmitTokenResponse {
	pub sid: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RegisterRequest {
	pub email: String,
	pub client_secret: OwnedClientSecret,
	pub sid: OwnedSessionId,
	pub password: String,
	pub username: Option<String>,
	pub device_id: Option<OwnedDeviceId>,
	pub initial_device_display_name: Option<String>,
	#[serde(default)]
	pub inhibit_login: bool,
}

/// # `GET /_matrix/client/v3/register/available`
#[tracing::instrument(skip_all, fields(%client), name = "register_available", level = "info")]
pub(crate) async fn get_register_available_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	body: Ruma<ruma::api::client::account::get_username_availability::v3::Request>,
) -> Result<ruma::api::client::account::get_username_availability::v3::Response> {
	let user_id =
		match UserId::parse_with_server_name(&body.username, services.globals.server_name()) {
			| Ok(user_id) => {
				if let Err(e) = user_id.validate_strict() {
					return Err!(Request(InvalidUsername(debug_warn!(
						"Username {} contains disallowed characters or spaces: {e}",
						body.username
					))));
				}

				user_id
			},
			| Err(e) => {
				return Err!(Request(InvalidUsername(debug_warn!(
					"Username {} is not valid: {e}",
					body.username
				))));
			},
		};

	if services.users.exists(&user_id).await {
		return Err!(Request(UserInUse("User ID is not available.")));
	}

	Ok(ruma::api::client::account::get_username_availability::v3::Response {
		available: true,
	})
}

/// # `POST /_matrix/client/v3/register/email/requestToken`
///
/// Request a verification code for a new account.
pub(crate) async fn request_registration_token_via_email_route(
	State(services): State<crate::State>,
	Json(body): Json<RegistrationEmailRequestTokenRequest>,
) -> Result<Json<RegistrationEmailRequestTokenResponse>> {
	if !services.config.allow_registration && !services.firstrun.is_first_run() {
		return Err!(Request(Forbidden(
			"This server is not accepting registrations at this time."
		)));
	}

	if !services.threepid.email_requirement().may_change() {
		return Err!(Request(Forbidden("Email verification is unavailable.")));
	}

	let Ok(email) = Address::try_from(body.email.clone()) else {
		return Err!(Request(InvalidParam("Invalid email address.")));
	};

	if services
		.threepid
		.get_localpart_for_email(&email)
		.await
		.is_some()
	{
		return Err!(Request(ThreepidInUse("This email address is already in use.")));
	}

	let session = services
		.threepid
		.send_validation_code_email(
			Mailbox::new(None, email),
			|verification_code| messages::NewAccountCode {
				verification_code,
			},
			&body.client_secret,
			body.send_attempt,
		)
		.await?;

	Ok(Json(RegistrationEmailRequestTokenResponse {
		sid: session.to_string(),
	}))
}

/// # `POST /_matrix/client/v3/register/email/submitToken`
///
/// Validate a registration email code.
pub(crate) async fn submit_registration_token_via_email_route(
	State(services): State<crate::State>,
	Json(body): Json<RegistrationEmailSubmitTokenRequest>,
) -> Result<Json<RegistrationEmailSubmitTokenResponse>> {
	services
		.threepid
		.try_validate_session(&body.sid, &body.token)
		.await
		.map_err(|message| err!(Request(ThreepidAuthFailed("{message}"))))?;

	Ok(Json(RegistrationEmailSubmitTokenResponse {
		sid: body.sid.to_string(),
	}))
}

/// # `POST /_matrix/client/v3/register`
///
/// Create an account after the email address has already been verified.
#[tracing::instrument(skip_all, fields(%client), name = "register", level = "info")]
pub(crate) async fn register_route(
	State(services): State<crate::State>,
	ClientIp(client): ClientIp,
	Json(body): Json<RegisterRequest>,
) -> Result<Json<ruma::api::client::account::register::v3::Response>> {
	if !services.config.allow_registration && !services.firstrun.is_first_run() {
		return Err!(Request(Forbidden(
			"This server is not accepting registrations at this time."
		)));
	}

	if !services.threepid.email_requirement().may_change() {
		return Err!(Request(Forbidden("Email verification is unavailable.")));
	}

	let Ok(email) = Address::try_from(body.email.clone()) else {
		return Err!(Request(InvalidParam("Invalid email address.")));
	};

	let expected_email = email.clone();

	if services
		.threepid
		.get_localpart_for_email(&expected_email)
		.await
		.is_some()
	{
		return Err!(Request(ThreepidInUse("This email address is already in use.")));
	}

	let emergency_mode_enabled = services.config.emergency_password.is_some();
	let supplied_username = body.username.clone().or_else(|| {
		if !email.user().is_empty() {
			Some(email.user().to_owned())
		} else {
			None
		}
	});

	let user_id = determine_registration_user_id(
		&services,
		supplied_username,
		false,
		emergency_mode_enabled,
	)
	.await?;

	if services.appservice.is_exclusive_user_id(&user_id).await && !emergency_mode_enabled {
		return Err!(Request(Exclusive("Username is reserved by an appservice.")));
	}

	let email = services
		.threepid
		.consume_valid_session(&body.sid, &body.client_secret)
		.await
		.map_err(|message| err!(Request(ThreepidAuthFailed("{message}"))))?;

	if email != expected_email {
		return Err!(Request(ThreepidAuthFailed(
			"Verification email does not match the supplied address"
		)));
	}

	services.users.create(&user_id, Some(body.password.as_str()), None).await?;

	// Associate the verified email with the new account. If another account sniped it in
	// the small window between verification and creation, keep the account creation failure
	// visible to the user.
	services
		.threepid
		.associate_localpart_email(user_id.localpart(), &email)
		.await?;

	let mut displayname = user_id.localpart().to_owned();
	if !services.globals.new_user_displayname_suffix().is_empty() {
		write!(displayname, " {}", services.globals.new_user_displayname_suffix())?;
	}
	services.users.set_displayname(&user_id, Some(displayname.clone()));

	services
		.account_data
		.update(
			None,
			&user_id,
			ruma::events::GlobalAccountDataEventType::PushRules.to_string().into(),
			&serde_json::to_value(ruma::events::push_rules::PushRulesEvent {
				content: ruma::events::push_rules::PushRulesEventContent {
					global: Ruleset::server_default(&user_id),
				},
			})?,
		)
		.await?;

	let no_device = body.inhibit_login;
	let (token, device) = if !no_device {
		let device_id = body
			.device_id
			.clone()
			.unwrap_or_else(|| utils::random_string(DEVICE_ID_LENGTH).into());
		let new_token = utils::random_string(TOKEN_LENGTH);

		services
			.users
			.create_device(
				&user_id,
				&device_id,
				&new_token,
				body.initial_device_display_name.clone(),
				Some(client.to_string()),
			)
			.await?;
		debug_info!(%user_id, %device_id, "User account was created");
		(Some(new_token), Some(device_id))
	} else {
		(None, None)
	};

	if services.server.config.admin_room_notices {
		services
			.admin
			.notice(&format!("New user \"{user_id}\" registered on this server."))
			.await;
	}

	let was_first_user = services.firstrun.empower_first_user(&user_id).await?;
	if !was_first_user && services.config.suspend_on_register {
		services
			.users
			.suspend_account(&user_id, &services.globals.server_user)
			.await;
	}

	Ok(Json(ruma::api::client::account::register::v3::Response {
		access_token: token,
		user_id,
		device_id: device,
		refresh_token: None,
		expires_in: None,
	}))
}

async fn determine_registration_user_id(
	services: &Services,
	supplied_username: Option<String>,
	_guest: bool,
	emergency_mode_enabled: bool,
) -> Result<OwnedUserId> {
	if let Some(supplied_username) = supplied_username {
		if services
			.globals
			.forbidden_usernames()
			.is_match(&supplied_username)
			&& !emergency_mode_enabled
		{
			return Err!(Request(Forbidden("Username is forbidden")));
		}

		let user_id = match UserId::parse_with_server_name(
			&supplied_username,
			services.globals.server_name(),
		) {
			| Ok(user_id) => {
				if let Err(e) = user_id.validate_strict() {
					if !emergency_mode_enabled {
						return Err!(Request(InvalidUsername(debug_warn!(
							"Username {supplied_username} contains disallowed characters or spaces: {e}"
						))));
					}
				}

				if !services.globals.user_is_local(&user_id) {
					return Err!(Request(InvalidUsername(
						"Username {supplied_username} is not local to this server"
					)));
				}

				user_id
			},
			| Err(e) => {
				return Err!(Request(InvalidUsername(debug_warn!(
					"Username {supplied_username} is not valid: {e}"
				))));
			},
		};

		if services.users.exists(&user_id).await {
			return Err!(Request(UserInUse("User ID is not available.")));
		}

		Ok(user_id)
	} else {
		loop {
			let user_id = UserId::parse_with_server_name(
				utils::random_string(RANDOM_USER_ID_LENGTH).to_lowercase(),
				services.globals.server_name(),
			)
			.unwrap();

			if !services.users.exists(&user_id).await {
				break Ok(user_id);
			}
		}
	}
}
