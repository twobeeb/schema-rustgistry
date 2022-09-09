use axum::{
    error_handling::HandleErrorLayer,
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use axum_macros::debug_handler;
use eyre::Result;
use serde::{de, Deserialize, Deserializer};
use serde_json::{json, Value};
use std::{net::SocketAddr, sync::Arc, time::Duration};

use crate::domain::{InputSchema, SharedState};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod domain;

pub enum VersionParam {
    Version(u32),
    Latest,
}
impl<'de> Deserialize<'de> for VersionParam {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(deserializer)?.to_lowercase().as_str() {
            "latest" => Ok(VersionParam::Latest),
            text => match text.parse::<u32>() {
                Ok(id) => Ok(VersionParam::Version(id)),
                Err(_e) => Err(de::Error::custom(format!(
                    "Could not parse request param `version`. Expected positive int or \"latest\", got: \"{text}\""
                ))),
            }
        }
    }
}
async fn list_subject_versions(
    Path(subject): Path<String>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    let versions: Vec<u32> = data
        .read()
        .await
        .get_subject_versions(&subject)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(versions))
}
#[debug_handler]
async fn get_schema_by_subject_and_version(
    Path((subject, version)): Path<(String, VersionParam)>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    Ok(Json(
        data.read()
            .await
            .get_subject_by_name_and_version(&subject, version)
            .ok_or(StatusCode::NOT_FOUND)?,
    ))
}
async fn get_schema_string_by_subject_and_version(
    Path((subject, version)): Path<(String, VersionParam)>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    Ok(Json(
        data.read()
            .await
            .get_subject_by_name_and_version(&subject, version)
            .map(|subject| {
                let value: serde_json::Result<Value> =
                    serde_json::from_str(subject.schema.as_str());
                value.unwrap()
            })
            .ok_or(StatusCode::NOT_FOUND)?,
    ))
}

async fn get_subject_by_id(
    Path(id): Path<i32>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    let result = data
        .read()
        .await
        .get_subject_by_id(id)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(result))
}

async fn list_subjects(State(data): State<SharedState>) -> impl IntoResponse {
    let subjects: Vec<String> = data.read().await.list_subjects();
    (StatusCode::OK, Json(subjects))
}
#[debug_handler]
async fn register_subject_version(
    Path(name): Path<String>,
    State(data): State<SharedState>,
    Json(body): Json<InputSchema>,
) -> impl IntoResponse {
    let mut state = data.write().await;

    match state.register_subject_version(name.as_str(), body) {
        Ok(next_id) => (
            StatusCode::CREATED,
            Json(json!({
                "id": next_id,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e })),
        ),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "example_key_value_store=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let shared = domain::initialize_state();
    let app = Router::with_state(Arc::clone(&shared))
        .route(
            "/subjects/:subject/versions",
            get(list_subject_versions).post(register_subject_version),
        )
        .route(
            "/subjects/:subject/versions/:version",
            get(get_schema_by_subject_and_version),
        )
        .route(
            "/subjects/:subject/versions/:version/schema",
            get(get_schema_string_by_subject_and_version),
        )
        .route("/schemas/:id", get(get_subject_by_id))
        .route("/subjects", get(list_subjects))
        // Add middleware to all routes
        .layer(
            ServiceBuilder::new()
                // Handle errors from middleware
                .layer(HandleErrorLayer::new(handle_error))
                .load_shed()
                .concurrency_limit(1024)
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        );

    // Run our app with hyper
    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    tracing::debug!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, "request timed out".to_string());
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "service is overloaded, try again later".to_string(),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Unhandled internal error: {}", error),
    )
}
