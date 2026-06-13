use crate::{
    protocol::Event,
    service::{DashboardAuth, SessionCoordinator},
    storage::{DeleteOutcome, HistoryPage, HistoryStats},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderValue, Method, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::{
        IntoResponse, Redirect, Response, sse::Event as SseEvent, sse::KeepAlive, sse::Sse,
    },
    routing::{delete, get, post},
};
use cookie::{Cookie, SameSite};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiState {
    pub coordinator: SessionCoordinator,
    pub token: SecretString,
    pub dashboard_auth: DashboardAuth,
    pub base_url: String,
    pub dashboard_clients: Arc<AtomicUsize>,
}

pub fn router(state: ApiState) -> Router {
    let protected = Router::new()
        .route("/v1/health", get(|| async { StatusCode::NO_CONTENT }))
        .route("/v1/events", post(post_event))
        .route("/v1/dashboard-launch", post(create_dashboard_launch))
        .route("/v1/sessions/active", get(active_sessions))
        .route("/v1/sessions/stream", get(stream_sessions))
        .route("/v1/history", get(history).delete(purge_history))
        .route("/v1/history/stats", get(history_stats))
        .route("/v1/history/{session_id}", delete(delete_history))
        .route("/dashboard", get(crate::dashboard::index))
        .route("/dashboard/app.css", get(crate::dashboard::styles))
        .route("/dashboard/app.js", get(crate::dashboard::script))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), authenticate));
    Router::new()
        .route("/dashboard/launch", get(consume_dashboard_launch))
        .merge(protected)
        .with_state(state)
}

async fn authenticate(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let supplied_bearer = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    let bearer_matches = supplied_bearer
        .as_bytes()
        .ct_eq(state.token.expose_secret().as_bytes())
        .unwrap_u8()
        == 1;
    if bearer_matches {
        return Ok(next.run(request).await);
    }

    let browser_session = request
        .headers()
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            Cookie::split_parse(value)
                .filter_map(Result::ok)
                .find(|cookie| cookie.name() == "smt_dashboard")
                .map(|cookie| cookie.value().to_owned())
        });
    let browser_matches = browser_session
        .as_deref()
        .is_some_and(|session| state.dashboard_auth.validates_session(session));
    if !browser_matches {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if is_mutation(request.method()) {
        let same_origin = request
            .headers()
            .get("origin")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|origin| origin == state.base_url);
        if !same_origin {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    Ok(next.run(request).await)
}

#[derive(Serialize)]
struct DashboardLaunchResponse {
    launch_url: String,
}

async fn create_dashboard_launch(State(state): State<ApiState>) -> Json<DashboardLaunchResponse> {
    let token = state.dashboard_auth.create_launch_token();
    Json(DashboardLaunchResponse {
        launch_url: format!("{}/dashboard/launch?token={token}", state.base_url),
    })
}

#[derive(Deserialize)]
struct DashboardLaunchQuery {
    token: String,
}

async fn consume_dashboard_launch(
    State(state): State<ApiState>,
    Query(query): Query<DashboardLaunchQuery>,
) -> Result<Response, StatusCode> {
    let session = state
        .dashboard_auth
        .consume_launch_token(&query.token)
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let cookie = Cookie::build(("smt_dashboard", session))
        .http_only(true)
        .same_site(SameSite::Strict)
        .path("/")
        .build()
        .to_string();
    let mut response = Redirect::to("/dashboard").into_response();
    response.headers_mut().insert(
        "set-cookie",
        HeaderValue::from_str(&cookie).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );
    Ok(response)
}

fn is_mutation(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}

async fn active_sessions(
    State(state): State<ApiState>,
) -> Json<Vec<crate::protocol::SessionSnapshot>> {
    Json(state.coordinator.active_sessions().await)
}

async fn stream_sessions(
    State(state): State<ApiState>,
) -> Sse<impl futures_util::Stream<Item = Result<SseEvent, Infallible>>> {
    let mut receiver = state.coordinator.subscribe();
    let clients = state.dashboard_clients.clone();
    clients.fetch_add(1, Ordering::Relaxed);
    let stream = async_stream::stream! {
        let _guard = DashboardClientGuard(clients);
        let initial = receiver.borrow().clone();
        yield Ok(SseEvent::default().event("sessions").json_data(initial).unwrap());
        while receiver.changed().await.is_ok() {
            let sessions = receiver.borrow().clone();
            yield Ok(SseEvent::default().event("sessions").json_data(sessions).unwrap());
        }
    };
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("heartbeat"),
    )
}

struct DashboardClientGuard(Arc<AtomicUsize>);

impl Drop for DashboardClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    #[serde(default = "default_history_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

fn default_history_limit() -> u32 {
    50
}

#[derive(Serialize)]
struct ApiError {
    code: &'static str,
    message: &'static str,
}

type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;

async fn history(
    State(state): State<ApiState>,
    Query(query): Query<HistoryQuery>,
) -> ApiResult<Json<HistoryPage>> {
    if !(1..=100).contains(&query.limit) {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "invalid_pagination",
            "history limit must be between 1 and 100",
        ));
    }
    let store = state.coordinator.history_store();
    let result = tokio::task::spawn_blocking(move || store.history(query.limit, query.offset))
        .await
        .map_err(|_| history_unavailable())?
        .map_err(|_| history_unavailable())?;
    Ok(Json(result))
}

async fn history_stats(State(state): State<ApiState>) -> ApiResult<Json<HistoryStats>> {
    let store = state.coordinator.history_store();
    let result = tokio::task::spawn_blocking(move || store.stats())
        .await
        .map_err(|_| history_unavailable())?
        .map_err(|_| history_unavailable())?;
    Ok(Json(result))
}

async fn delete_history(
    State(state): State<ApiState>,
    Path(session_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let store = state.coordinator.history_store();
    let outcome = tokio::task::spawn_blocking(move || store.delete_finalized(session_id))
        .await
        .map_err(|_| history_unavailable())?
        .map_err(|_| history_unavailable())?;
    match outcome {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT),
        DeleteOutcome::Missing => Err(api_error(
            StatusCode::NOT_FOUND,
            "history_not_found",
            "session summary was not found",
        )),
        DeleteOutcome::Active => Err(api_error(
            StatusCode::CONFLICT,
            "session_active",
            "active sessions cannot be deleted",
        )),
    }
}

async fn purge_history(State(state): State<ApiState>) -> ApiResult<StatusCode> {
    let store = state.coordinator.history_store();
    tokio::task::spawn_blocking(move || store.purge_finalized())
        .await
        .map_err(|_| history_unavailable())?
        .map_err(|_| history_unavailable())?;
    Ok(StatusCode::NO_CONTENT)
}

fn history_unavailable() -> (StatusCode, Json<ApiError>) {
    api_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "history_unavailable",
        "session history is unavailable",
    )
}

fn api_error(
    status: StatusCode,
    code: &'static str,
    message: &'static str,
) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { code, message }))
}

async fn post_event(
    State(state): State<ApiState>,
    Json(event): Json<Event>,
) -> Result<Json<crate::protocol::SessionSnapshot>, (StatusCode, String)> {
    state
        .coordinator
        .apply(event)
        .await
        .map(Json)
        .map_err(|error| (StatusCode::UNPROCESSABLE_ENTITY, error.to_string()))
}
