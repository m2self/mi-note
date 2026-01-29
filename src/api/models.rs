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
    #[serde(default)]
    pub subject: String,
    #[serde(default, alias = "Title", alias = "topic", alias = "name", alias = "Topic", alias = "Name")]
    pub title: Option<String>,
    #[serde(default, alias = "contentPreview", alias = "summary", alias = "content_preview")]
    pub snippet: String,
    pub content: Option<String>,
    pub extra_info: Option<String>,
    #[serde(deserialize_with = "deserialize_id")]
    pub tag: String,
    pub status: String,
    pub setting: Option<Setting>,
}

impl Note {
    pub fn display_title(&self) -> String {
        // 1. Try title field
        if let Some(ref t) = self.title {
            let s = strip_tags(t);
            if !s.is_empty() { return s; }
        }

        // 2. Try subject field
        let s = strip_tags(&self.subject);
        if !s.is_empty() { return s; }

        // 3. Try extra_info parsing
        if let Some(ref extra) = self.extra_info {
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(extra) {
                if let Some(Value::String(t)) = map.get("title") {
                    let s = strip_tags(t);
                    if !s.is_empty() { return s; }
                }
            }
        }

        // 4. Try snippet first line
        let clean_snippet = strip_tags(&self.snippet);
        let first_line = clean_snippet.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() {
             return first_line.to_string();
        }

        "[No Title]".to_string()
    }

    pub fn clean_snippet(&self) -> String {
        strip_tags(&self.snippet)
    }
}

pub fn strip_tags(text: &str) -> String {
    strip_tags_multiline(text).replace("\n", " ")
          .replace("\r", " ")
          .trim()
          .to_string()
}

pub fn strip_tags_multiline(text: &str) -> String {
    // 1. Decode entities first so we can find tags like <text>
    let decoded = text.replace("&nbsp;", " ")
                      .replace("&lt;", "<")
                      .replace("&gt;", ">")
                      .replace("&amp;", "&")
                      .replace("&quot;", "\"")
                      .replace("&apos;", "'");

    let mut result = String::with_capacity(decoded.len());
    let mut in_tag = false;
    for c in decoded.chars() {
        if c == '<' {
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(c);
        }
    }
    result
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
    #[serde(default)]
    pub folders: Option<Vec<Folder>>,
    #[serde(default)]
    pub last_page: bool,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub sync_tag: Option<String>,
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
