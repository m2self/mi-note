use serde::{Deserialize, Serialize, Deserializer};
use serde_json::Value;

pub fn deserialize_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        _ => Ok(v.to_string()),
    }
}

#[allow(dead_code)]
pub fn deserialize_opt_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Value::deserialize(deserializer)?;
    match v {
        Value::String(s) => Ok(Some(s)),
        Value::Number(n) => Ok(Some(n.to_string())),
        Value::Null => Ok(None),
        _ => Ok(Some(v.to_string())),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub folder_id: Option<String>,
    pub color_id: i32,
    pub create_date: i64,
    pub modify_date: i64,
    pub subject: String,
    pub snippet: String,
    pub content: Option<String>,
    #[serde(deserialize_with = "deserialize_id")]
    pub tag: String,
    pub status: String,
    pub setting: Option<Setting>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: String,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub folder_id: Option<String>,
    pub create_date: i64,
    pub modify_date: i64,
    pub subject: String,
    #[serde(deserialize_with = "deserialize_id")]
    pub tag: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub data: Option<Value>,
    pub theme_id: i32,
    pub version: i32,
    pub sticky_time: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileData {
    pub digest: String,
    pub file_id: String,
    pub mime_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotesResponse {
    pub entries: Vec<Note>,
    pub folders: Vec<Folder>,
    pub last_page: bool,
    #[serde(deserialize_with = "deserialize_id")]
    pub sync_tag: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct EntryResponse {
    pub entry: Note,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct FolderResponse {
    pub folder: Folder,
}
