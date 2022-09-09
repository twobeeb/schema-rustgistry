use crate::VersionParam;
use avro_rs::Schema;
use md5::Digest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
use std::iter::Map;
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
pub type SubjectVersion = u32;
pub type SubjectName = String;
pub type SchemaId = i32;
pub type SchemaIdAndSubjects = (SchemaId, HashMap<SubjectName, SubjectVersion>);
pub type SharedState = Arc<RwLock<AppState>>;
pub fn initialize_state() -> SharedState {
    // We suppose that the map is init using external data.

    let mut app_state = AppState {
        schemas_by_id: HashMap::new(),
        schemas_by_name: HashMap::new(),
        hashes: HashMap::new(),
    };
    app_state
        .register_subject_version(
            "blublu",
            InputSchema {
                schema: "[\"long\"]".to_string(),
            },
        )
        .expect("TODO: panic message");
    SharedState::new(RwLock::new(app_state))
}
pub struct AppState {
    schemas_by_name: HashMap<String, Vec<Subject>>,
    schemas_by_id: HashMap<i32, Subject>,
    hashes: HashMap<Digest, SchemaIdAndSubjects>,
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
                    .iter()
                    .max_by_key(|x| x.version)?
                    .clone(),
            ),
            VersionParam::Version(version) => {
                let subject: Vec<&Subject> = self
                    .schemas_by_name
                    .get(name)?
                    .iter()
                    .filter(|x1| x1.version == version)
                    .collect();
                if subject.is_empty() {
                    None
                } else {
                    Some(subject[0].clone())
                }
            }
        }
    }
    pub fn get_subject_versions(&self, subject: &str) -> Option<Vec<u32>> {
        let versions: Vec<u32> = self
            .schemas_by_name
            .get(subject)?
            .iter()
            .map(|s| s.version)
            .collect();
        Some(versions)
    }
    pub fn list_subjects(&self) -> Vec<String> {
        self.schemas_by_name.keys().into_iter().cloned().collect()
    }
    pub fn get_subject_by_id(&self, id: i32) -> Option<Subject> {
        Some(self.schemas_by_id.get(&id)?.clone())
    }
    pub fn register_subject_version(
        &mut self,
        subject_name: &str,
        body: InputSchema,
    ) -> Result<i32, String> {
        let parsed_schema = Schema::parse_str(body.schema.as_str())
            .map_err(|e| format!("Schema could could be parsed: {e}"))?;

        let md5 = md5::compute(parsed_schema.canonical_form());
        match self.hashes.get(&md5) {
            // Found Subjects, need to confirm same or new
            Some(found) => {
                match found.1.get(subject_name) {
                    Some(s) => Ok(found.0),
                    None => {
                        let schema_id = found.0;
                        self.register_subject_with_id(schema_id, subject_name.to_string(), body.schema, md5)
                    }
                }
            },
            // Create the schema
            None => {
                let next_schema_id = self.schemas_by_id.keys().max().map_or(1,|t| t + 1);
                self.register_subject_with_id(next_schema_id, subject_name.to_string(), body.schema, md5)
            }
        }
    }
    fn register_subject_with_id(&mut self, schema_id: SchemaId, subject_name: SubjectName, schema: String, md5: Digest)->Result<i32, String>{
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
            return Err("Sorry, 3 versions maximum".to_string());
        }

        let subject = Subject {
            id: schema_id,
            name: subject_name.clone(),
            version: new_version,
            schema,
        };
        all_subject_versions.push(subject.clone());


        // store back the updated state
        let mut map: HashMap<SubjectName, SubjectVersion> = HashMap::new();
        map.insert(subject_name.to_string(), new_version);


        self.hashes.insert(md5,(schema_id, map));

        self.schemas_by_name
            .insert(String::from(&subject_name), all_subject_versions.clone());
        self.schemas_by_id.insert(schema_id, subject);
        Ok(schema_id)
    }
}
