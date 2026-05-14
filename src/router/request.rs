use std::{
	fmt::Debug,
	sync::{Arc, atomic::Ordering},
	time::Duration,
};

use axum::{
	body::{Body, to_bytes},
	extract::State,
	response::{IntoResponse, Response},
};
use conduwuit::{Result, debug, debug_error, debug_warn, err, error, info, trace};
use conduwuit_service::Services;
use futures::FutureExt;
use http::{Method, Request, StatusCode, Uri};
use serde_json::Value;
use tokio::time::sleep;
use tracing::Span;

#[tracing::instrument(name = "request", level = "debug", skip_all)]
pub(crate) async fn handle(
	State(services): State<Arc<Services>>,
	req: http::Request<axum::body::Body>,
	next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
	if !services.server.running() {
		debug_warn!(
			method = %req.method(),
			uri = %req.uri(),
			"unavailable pending shutdown"
		);

		return Err(StatusCode::SERVICE_UNAVAILABLE);
	}

	let uri = req.uri().clone();
	let method = req.method().clone();
	let services_ = services.clone();
	let parent = Span::current();
	let task = services.server.runtime().spawn(async move {
		tokio::select! {
			response = execute(&services_, req, next, &parent) => response,
			response = services_.server.until_shutdown()
				.then(|()| {
					let timeout = services_.server.config.client_shutdown_timeout;
					let timeout = Duration::from_secs(timeout);
					sleep(timeout)
				})
				.map(|()| StatusCode::SERVICE_UNAVAILABLE)
				.map(IntoResponse::into_response) => response,
		}
	});

	task.await
		.map_err(unhandled)
		.and_then(move |result| handle_result(&method, &uri, result))
}

#[tracing::instrument(
	name = "handle",
	level = "debug",
	parent = parent,
	skip_all,
	fields(
		active = %services
			.server
			.metrics
			.requests_handle_active
			.fetch_add(1, Ordering::Relaxed),
		handled = %services
			.server
			.metrics
			.requests_handle_finished
			.load(Ordering::Relaxed),
	)
)]
async fn execute(
	// we made a safety contract that Services will not go out of scope
	// during the request; this ensures a reference is accounted for at
	// the base frame of the task regardless of its detachment.
	services: &Arc<Services>,
	req: http::Request<axum::body::Body>,
	next: axum::middleware::Next,
	parent: &Span,
) -> Response {
	#[cfg(debug_assertions)]
	conduwuit::defer! {{
		_ = services.server
			.metrics
			.requests_handle_finished
			.fetch_add(1, Ordering::Relaxed);
		_ = services.server
			.metrics
			.requests_handle_active
			.fetch_sub(1, Ordering::Relaxed);
	}};

	next.run(req).await
}

#[tracing::instrument(name = "auth_request", level = "info", skip_all)]
pub(crate) async fn auth_request(
	State(services): State<Arc<Services>>,
	req: Request<Body>,
	next: axum::middleware::Next,
) -> Result<Response, StatusCode> {
	let path = req.uri().path().to_owned();
	let method = req.method().clone();

	if let Some(label) = auth_request_label(&path) {
		let max_body_size = services.server.config.max_request_size;
		let (parts, body) = req.into_parts();
		let body = to_bytes(body, max_body_size)
			.await
			.map_err(|_| {
				debug_warn!(%method, %path, %label, "failed to read auth request body");
				StatusCode::BAD_REQUEST
			})?;
		let body_keys = json_keys(&body);

		info!(%method, %path, %label, ?body_keys, "auth request start");

		let req = Request::from_parts(parts, Body::from(body));
		let response = next.run(req).await;
		info!(
			%method,
			%path,
			%label,
			status = %response.status(),
			"auth request end"
		);
		return Ok(response);
	}

	Ok(next.run(req).await)
}

fn handle_result(method: &Method, uri: &Uri, result: Response) -> Result<Response, StatusCode> {
	let status = result.status();
	let code = status.as_u16();
	let reason = status.canonical_reason().unwrap_or("Unknown Reason");

	if status.is_server_error() {
		error!(%method, %uri, "{code} {reason}");
	} else if status.is_client_error() {
		debug_error!(%method, %uri, "{code} {reason}");
	} else if status.is_redirection() {
		debug!(%method, %uri, "{code} {reason}");
	} else {
		trace!(%method, %uri, "{code} {reason}");
	}

	if status == StatusCode::METHOD_NOT_ALLOWED {
		return Ok(err!(Request(Unrecognized("Method Not Allowed"))).into_response());
	}

	Ok(result)
}

fn auth_request_label(path: &str) -> Option<&'static str> {
	match path {
		"/_matrix/client/v3/login" => Some("login"),
		"/_matrix/client/v3/register" => Some("register"),
		"/_matrix/client/v3/register/email/requestToken" => Some("register_request_token"),
		"/_matrix/client/v3/register/email/submitToken" => Some("register_submit_token"),
		"/_matrix/client/v3/account/password/email/requestToken" => {
			Some("password_reset_request_token")
		},
		"/_matrix/client/v3/account/password/email/submitToken" => {
			Some("password_reset_submit_token")
		},
		"/_matrix/client/v3/account/password/email/reset" => Some("password_reset"),
		"/_matrix/client/v3/account/password" => Some("legacy_password_change"),
		_ => None,
	}
}

fn json_keys(body: &[u8]) -> Vec<String> {
	serde_json::from_slice::<Value>(body)
		.ok()
		.and_then(|value| value.as_object().cloned())
		.map(|map| map.keys().cloned().collect())
		.unwrap_or_default()
}

#[cold]
fn unhandled<Error: Debug>(e: Error) -> StatusCode {
	error!("unhandled error or panic during request: {e:?}");

	StatusCode::INTERNAL_SERVER_ERROR
}
