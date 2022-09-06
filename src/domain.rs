use crate::VersionParam;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct InputSchema {
    schema: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Subject {
    pub id: i32,
    pub name: String,
    pub version: u32,
    pub schema: String,
}
pub type SharedState = Arc<RwLock<AppState>>;
pub fn initialize_state() -> SharedState {
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
    let schemas_by_name: HashMap<String, Vec<Subject>> = schemas_by_id
        .clone()
        .iter()
        .map(|(_k, v)| {
            let mut vec: Vec<Subject> = Vec::new();
            vec.push(v.clone());
            (v.name.clone(), vec)
        })
        .collect();

    let app_state = AppState {
        schemas_by_id: schemas_by_id.clone(),
        schemas_by_name: schemas_by_name.clone(),
    };
    let shared = SharedState::new(RwLock::new(app_state));
    shared
}
pub struct AppState {
    schemas_by_name: HashMap<String, Vec<Subject>>,
    schemas_by_id: HashMap<i32, Subject>,
}

impl AppState {
    pub fn get_subject_by_name_and_version(
        &self,
        name: &str,
        version_param: VersionParam,
    ) -> Option<Subject> {
        match version_param {
            VersionParam::Latest => Some(
                self.schemas_by_name
                    .get(name)?
                    .into_iter()
                    .max_by_key(|&x| x.version)?
                    .clone(),
            ),
            VersionParam::Version(version) => {
                let subject: Vec<&Subject> = self
                    .schemas_by_name
                    .get(name)?
                    .into_iter()
                    .filter(|x1| x1.version == version)
                    .collect();
                if subject.len() > 0 {
                    Some(subject[0].clone())
                } else {
                    None
                }
            }
        }
    }
    pub fn get_subject_versions(&self, subject: &str) -> Option<Vec<u32>> {
        let versions: Vec<u32> = self
            .schemas_by_name
            .get(subject)
            .unwrap()
            .into_iter()
            .map(|s| s.version)
            .collect();
        Some(versions)
    }
    pub fn list_subjects(&self) -> Vec<String> {
        self.schemas_by_name
            .keys()
            .into_iter()
            .map(|x| x.clone())
            .collect()
    }
    pub fn get_subject_by_id(&self, id: i32) -> Option<Subject> {
        Some(self.schemas_by_id.get(&id)?.clone())
    }
    pub fn register_subject_version(
        &mut self,
        subject_name: String,
        body: InputSchema,
    ) -> Result<i32, &'static str> {
        let next_schema_id = self
            .schemas_by_id
            .keys()
            .max()
            .and_then(|t| Some(t + 1))
            .unwrap_or(1);

        let mut all_subject_versions = match self.schemas_by_name.get(&subject_name) {
            Some(x) => x.clone(),
            None => Vec::new(),
        };
        let new_version = all_subject_versions
            .iter()
            .map(|x1| x1.version)
            .max()
            .unwrap_or(0)
            + 1;
        if new_version > 3 {
            return Err("Sorry, 3 versions maximum");
        }

        let subject = Subject {
            id: next_schema_id,
            name: subject_name.clone(),
            version: new_version,
            schema: body.schema.clone(),
        };
        all_subject_versions.push(subject.clone());

        // store back the updated state
        self.schemas_by_name
            .insert(subject_name, all_subject_versions.clone());
        self.schemas_by_id.insert(next_schema_id, subject.clone());
        Ok(next_schema_id)
    }
}
