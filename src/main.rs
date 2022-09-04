use axum::{
    error_handling::HandleErrorLayer, extract, extract::*, http::StatusCode,
    response::IntoResponse, routing::get, Router,
};
use axum_macros::debug_handler;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::borrow::Borrow;
use std::ops::Deref;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};

use crate::domain::{InputSchema, SharedState, Subject};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod domain;

async fn get_subject_versions(
    Path(subject): Path<String>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    let versions: Vec<i32> = data
        .read()
        .await
        .get_subject_versions(&subject)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(versions))
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

    match state.register_subject_version(name, body) {
        Ok(next_id) => (
            StatusCode::CREATED,
            Json(json!({
                "id": next_id,
            })),
        ),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))),
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

    let shared = domain::initializeState();
    let app = Router::with_state(Arc::clone(&shared))
        .route(
            "/subjects/:subject/versions",
            get(get_subject_versions).post(register_subject_version),
        )
        .route("/schemas/:id", get(get_subject_by_id))
        .route("/subjects", get(list_subjects))
        //.route("/subjects/:subject/versions/:version", get())
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
        format!("Unhandled internal error: {}", error).to_string(),
    )
}
