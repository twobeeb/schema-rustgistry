use actix_web::error::ErrorNotFound;
use actix_web::web::Json;
use actix_web::{get, post, web, App, HttpServer, Responder, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Subject {
    id: i32,
    name: String,
    version: i32,
    schema: String,
}

struct AppState {
    schemas_by_name: Mutex<HashMap<String, Subject>>,
    schemas_by_id: Mutex<HashMap<i32, Subject>>,
    next_schema_id: Mutex<i32>,
}

#[get("/subjects/{name}")]
async fn get_subject_by_name(
    name: web::Path<String>,
    data: web::Data<AppState>,
) -> Result<impl Responder> {
    match data
        .schemas_by_name
        .lock()
        .unwrap()
        .get(name.into_inner().as_str())
    {
        Some(s) => Ok(Json(s.clone())),
        _ => Err(ErrorNotFound("Subject not Found")),
    }
}
#[get("/schemas/{id}")]
async fn get_subject_by_id(
    id: web::Path<i32>,
    data: web::Data<AppState>,
) -> Result<impl Responder> {
    match data.schemas_by_id.lock().unwrap().get(&(id.into_inner())) {
        Some(s) => Ok(Json(s.clone())),
        _ => Err(ErrorNotFound("Subject not Found")),
    }
}
#[get("/subjects")]
async fn list_subjects(data: web::Data<AppState>) -> impl Responder {
    let subjects: Vec<Subject> = data
        .schemas_by_name
        .lock()
        .unwrap()
        .values()
        .cloned()
        .collect();
    format!("{subjects:#?}")
}

#[post("/subjects/{name}")]
async fn register_subject_version(
    name: web::Path<String>,
    schema: String,
    data: web::Data<AppState>,
) -> impl Responder {
    let name = name.into_inner().clone();

    let mut schemas_by_name = data.schemas_by_name.lock().unwrap();
    let mut schemas_by_id = data.schemas_by_id.lock().unwrap();
    let mut next = data.next_schema_id.lock().unwrap();
    let subject = Subject {
        id: *next,
        name: name.clone(),
        version: 1,
        schema: schema.clone(),
    };
    schemas_by_name.insert(name.clone(), subject.clone());
    schemas_by_id.insert(*next, subject.clone());

    *next += 1;

    format!("New subejct registered: {name}!")
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "debug");
    std::env::set_var("RUST_BACKTRACE", "1");
    env_logger::init();

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

    let app_state = web::Data::new(AppState {
        schemas_by_id: Mutex::new(schemas_by_id.clone()),
        schemas_by_name: Mutex::new(schemas_by_name.clone()),
        next_schema_id: Mutex::new(2),
    });
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .service(list_subjects)
            .service(get_subject_by_name)
            .service(register_subject_version)
            .service(get_subject_by_id)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
