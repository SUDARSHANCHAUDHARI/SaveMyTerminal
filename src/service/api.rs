use crate::{protocol::Event, service::SessionRegistry};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State},
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
};
use secrecy::{ExposeSecret, SecretString};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct ApiState {
    pub registry: SessionRegistry,
    pub token: SecretString,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/health", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/events", post(post_event))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .with_state(state)
}

async fn authenticate(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let supplied = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if supplied
        .as_bytes()
        .ct_eq(state.token.expose_secret().as_bytes())
        .unwrap_u8()
        != 1
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(request).await)
}

async fn post_event(
    State(state): State<ApiState>,
    Json(event): Json<Event>,
) -> Result<Json<crate::protocol::SessionSnapshot>, (StatusCode, String)> {
    state
        .registry
        .apply(event)
        .await
        .map(Json)
        .map_err(|error| (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()))
}
