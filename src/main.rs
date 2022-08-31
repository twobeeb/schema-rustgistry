use axum::{
    error_handling::HandleErrorLayer,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::Duration,
};
use tower::{BoxError, ServiceBuilder};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Subject {
    id: i32,
    name: String,
    version: i32,
    schema: String,
}

struct AppState {
    schemas_by_name: HashMap<String, Subject>,
    schemas_by_id: HashMap<i32, Subject>,
}
type SharedState = Arc<RwLock<AppState>>;

async fn get_subject_by_name(
    Path(name): Path<String>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    let s = data
        .read()
        .unwrap()
        .schemas_by_name
        .get(name.as_str())
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(s.clone()))
}

async fn get_subject_by_id(
    Path(id): Path<i32>,
    State(data): State<SharedState>,
) -> Result<impl IntoResponse, StatusCode> {
    let result = data
        .read()
        .unwrap()
        .schemas_by_id
        .get(&id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(result))
}

async fn list_subjects(State(data): State<SharedState>) -> impl IntoResponse {
    let subjects: Vec<Subject> = data
        .read()
        .unwrap()
        .schemas_by_name
        .values()
        .cloned()
        .collect();
    (StatusCode::OK, Json(subjects))
}

async fn register_subject_version(
    Path(name): Path<String>,
    //axum::extract::RawBody(schema): String,
    State(data): State<SharedState>,
) -> impl IntoResponse {
    let name = name.to_string();
    let mut state = data.write().unwrap();

    let schema = "empty".to_string();

    let next = state.schemas_by_id.keys().max().unwrap() + 1;
    let subject = Subject {
        id: next,
        name: name.clone(),
        version: 1,
        schema: schema.clone(),
    };
    state.schemas_by_name.insert(name.clone(), subject.clone());
    state.schemas_by_id.insert(next, subject.clone());

    (StatusCode::CREATED, Json("New subejct registered: {name}!"))
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

    // We suppose that the map is init using external data.
    let mut schemas_by_id: HashMap<i32, Subject> = HashMap::new();
    schemas_by_id.insert(
        0,
        Subject {
            id: 0,
            name: "blublu".to_string(),
            version: 1,
            schema: "".to_string(),
        },
    );
    let schemas_by_name: HashMap<String, Subject> = schemas_by_id
        .clone()
        .iter()
        .map(|(_k, v)| (v.name.clone(), v.clone()))
        .collect();

    let app_state = AppState {
        schemas_by_id: schemas_by_id.clone(),
        schemas_by_name: schemas_by_name.clone(),
    };
    let shared = SharedState::new(RwLock::new(app_state));
    let app = Router::with_state(Arc::clone(&shared))
        .route(
            "/subjects/:name",
            get(get_subject_by_name).post(register_subject_version),
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
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {}", error)),
    )
}
