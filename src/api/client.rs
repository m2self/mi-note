use crate::api::models::*;
use crate::api::auth::MiAccount;
use reqwest::{header, Client as HttpClient};
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Clone)]
pub struct Client {
    http: HttpClient,
    account: Arc<RwLock<MiAccount>>,
    micloud_cookie: Arc<RwLock<String>>,
    user_agent: String,
}

type ReqResult = (Vec<u8>, reqwest::StatusCode);

impl Client {
    pub fn new(account_cookie: &str, user_agent: Option<String>) -> Self {
        let account = MiAccount::new(account_cookie);
        let http = HttpClient::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap();

        let initial_micloud_cookie = if account_cookie.contains("serviceToken") {
            account_cookie.to_string()
        } else {
            String::new()
        };

        let ua = user_agent.unwrap_or_else(|| "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string());

        Self {
            http,
            account: Arc::new(RwLock::new(account)),
            micloud_cookie: Arc::new(RwLock::new(initial_micloud_cookie)),
            user_agent: ua,
        }
    }

    fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    }

    async fn do_request(&self, method: reqwest::Method, url: String, form: Option<HashMap<String, String>>) -> crate::api::MiResult<ReqResult> {
        let is_mobile = self.user_agent.contains("iPhone") || self.user_agent.contains("Android") || self.user_agent.contains("Mobile");

        for _i in 0..3 {
            if _i > 0 {
                crate::dprintln!("[Background API] Attempt {} for URL: {}", _i + 1, url);
            }
            let mut req = self.http.request(method.clone(), &url);

            // Add cookies
            let cookie_str = self.micloud_cookie.read().await;
            let final_cookie = if !cookie_str.is_empty() {
                cookie_str.clone()
            } else {
                let account = self.account.read().await;
                account.cookie.clone()
            };
            req = req.header(header::COOKIE, final_cookie.as_str());

            // Headers matching actual browser behavior (from cURL analysis)
            req = req.header(header::USER_AGENT, &self.user_agent)
                .header(header::ACCEPT, "*/*")
                .header("Accept-Language", "en-US,en;q=0.9,zh-CN;q=0.8,zh;q=0.7")
                .header("priority", "u=1, i")
                .header(header::REFERER, "https://i.mi.com/note/h5")
                .header("Sec-Ch-Ua-Mobile", if is_mobile { "?1" } else { "?0" })
                .header("Sec-Ch-Ua-Platform", if is_mobile { "\"iOS\"" } else { "\"Windows\"" })
                .header("Sec-Fetch-Dest", "empty")
                .header("Sec-Fetch-Mode", "cors")
                .header("Sec-Fetch-Site", "same-origin");

            // X-XSRF-TOKEN is ONLY for POST/PUT/DELETE, NOT for GET requests
            if method != reqwest::Method::GET {
                if let Some(ph) = Self::extract_cookie_value(&final_cookie, "i.mi.com_ph") {
                    crate::dprintln!("Adding X-XSRF-TOKEN header for {} request", method);
                    req = req.header("X-XSRF-TOKEN", ph);
                }
            }

            if let Some(ref f) = form {
                req = req.form(f);
            }

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => {
                    crate::dprintln!("Request SEND ERROR: {} | URL: {}", e, url);
                    if _i == 2 { return Err(e.into()); }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
            };
            let status = resp.status();
            if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                let bytes = resp.bytes().await?.to_vec();
                let _body = String::from_utf8_lossy(&bytes);

                if _i < 2 {
                    crate::dprintln!("Attempting STS refresh...");

                    // Get fresh cookies via STS in a scope to ensure locks are released
                    let new_cookie_result = {
                        let account = self.account.read().await;  // Use read lock, not write
                        account.gen_micloud_cookie().await
                    };


                    let new_cookie = match new_cookie_result {
                        Ok(cookie) => {
                            crate::dprintln!("STS refresh successful.");
                            cookie
                        },
                        Err(e) => {
                            crate::dprintln!("STS refresh FAILED: {}", e);
                            return Err(format!("STS error: {}", e).into());
                        },
                    };


                    // Update the cookie in a separate scope
                    {
                        let mut mc = self.micloud_cookie.write().await;
                        *mc = new_cookie;
                    }

                    continue;
                }

                eprintln!("Session expired. Please login again.");
                return Err(format!("Authentication error: {}", status).into());
            }

            let status = resp.status();
            let bytes = resp.bytes().await?.to_vec();
            return Ok((bytes, status));
        }
        Err("Max retries reached".into())
    }

    #[allow(dead_code)]
    async fn request_json<T: serde::de::DeserializeOwned>(&self, method: reqwest::Method, url: String, form: Option<HashMap<String, String>>) -> crate::api::MiResult<T> {
        let (bytes, status) = self.do_request(method, url, form).await?;
        if !status.is_success() {
            return Err(format!("Request failed with status: {} (body: {})", status, String::from_utf8_lossy(&bytes)).into());
        }

        let json: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(j) => j,
            Err(e) => {
                crate::dprintln!("JSON Parse ERROR: {} | Body: {}", e, String::from_utf8_lossy(&bytes));
                return Err(e.into());
            }
        };
        let data = json.get("data").ok_or_else(|| {
            println!("API Error: No 'data' in response. Body: {}", json);
            "no data in response"
        })?;
        let result: T = serde_json::from_value(data.clone())?;
        Ok(result)
    }

    pub async fn list_notes(&self, limit: i32) -> crate::api::MiResult<NotesResponse> {
        let url = format!("https://i.mi.com/note/full/page/?limit={}&ts={}", limit, Self::now_ms());
        let (bytes, status) = self.do_request(reqwest::Method::GET, url, None).await?;

        if !status.is_success() {
            return Err(format!("Request failed with status: {}", status).into());
        }

        let json: serde_json::Value = serde_json::from_slice(&bytes)?;
        let data = json.get("data").ok_or_else(|| {
            println!("API Error: No 'data' in response. Body: {}", json);
            "no data in response"
        })?;

        if let Some(_entries) = data.get("entries").and_then(|e| e.as_array()) {
            crate::dprintln!("DEBUG: API returned {} notes.", _entries.len());
        }

        let result: NotesResponse = serde_json::from_value(data.clone())?;

        Ok(result)
    }

    #[allow(dead_code)]
    pub async fn get_note(&self, id: &str) -> crate::api::MiResult<Note> {
        let url = format!("https://i.mi.com/note/note/{}?ts={}", id, Self::now_ms());
        let data: serde_json::Value = self.request_json(reqwest::Method::GET, url, None).await?;

        // Strategy 1: Try wrapped EntryResponse
        if let Ok(res) = serde_json::from_value::<EntryResponse>(data.clone()) {
            return Ok(res.entry);
        }

        // Strategy 2: Try direct Note
        if let Ok(note) = serde_json::from_value::<Note>(data.clone()) {
            return Ok(note);
        }

        Err(format!("Could not parse Note from API response: {}", data).into())
    }

    #[allow(dead_code)]
    pub async fn create_note(&self, folder_id: &str, subject: &str, content: &str) -> crate::api::MiResult<Note> {
        let url = "https://i.mi.com/note/full/post";
        let mut params = HashMap::new();
        params.insert("folder_id".to_string(), folder_id.to_string());

        let mut entry = HashMap::new();
        entry.insert("subject", subject);
        entry.insert("content", content);
        let entry_json = serde_json::to_string(&entry)?;

        params.insert("entry".to_string(), entry_json);
        let service_token = self.account.read().await.get_service_token();
        params.insert("serviceToken".to_string(), service_token);

        let data: serde_json::Value = self.request_json(reqwest::Method::POST, url.to_string(), Some(params)).await?;

        // Success check: If we got here, request_json already checked status is_success
        // Try to return the created note, otherwise return a synthetic one based on inputs
        if let Ok(res) = serde_json::from_value::<EntryResponse>(data.clone()) {
            Ok(res.entry)
        } else if let Ok(note) = serde_json::from_value::<Note>(data.clone()) {
            Ok(note)
        } else {
            // Fallback: If creation succeeded at status level, return a dummy Note to satisfy types
            // UI will refresh anyway
            Ok(Note {
                id: "synthetic".to_string(),
                folder_id: Some(folder_id.to_string()),
                subject: subject.to_string(),
                snippet: "".to_string(),
                tag: "".to_string(),
                ..Default::default()
            })
        }
    }

    #[allow(dead_code)]
    pub async fn update_note(&self, id: &str, tag: &str, subject: &str, content: &str, folder_id: Option<&str>) -> crate::api::MiResult<Note> {
        let url = format!("https://i.mi.com/note/note/{}", id);
        let mut params = HashMap::new();
        params.insert("tag".to_string(), tag.to_string());

        let mut entry = HashMap::new();
        entry.insert("id".to_string(), id.to_string());
        entry.insert("tag".to_string(), tag.to_string());
        entry.insert("subject".to_string(), subject.to_string());
        entry.insert("content".to_string(), content.to_string());
        if let Some(fid) = folder_id {
            entry.insert("folderId".to_string(), fid.to_string());
        }
        let entry_json = serde_json::to_string(&entry)?;

        params.insert("entry".to_string(), entry_json);
        let service_token = self.account.read().await.get_service_token();
        params.insert("serviceToken".to_string(), service_token);

        let data: serde_json::Value = self.request_json(reqwest::Method::POST, url, Some(params)).await?;

        if let Ok(res) = serde_json::from_value::<EntryResponse>(data.clone()) {
            Ok(res.entry)
        } else if let Ok(note) = serde_json::from_value::<Note>(data.clone()) {
            Ok(note)
        } else {
            // Fallback for success cases with minimal body
            Ok(Note {
                id: id.to_string(),
                folder_id: folder_id.map(|s| s.to_string()),
                subject: subject.to_string(),
                snippet: "".to_string(),
                tag: tag.to_string(),
                ..Default::default()
            })
        }
    }

    #[allow(dead_code)]
    pub async fn delete_note(&self, id: &str, tag: &str, purge: bool) -> crate::api::MiResult<()> {
        let url = format!("https://i.mi.com/note/full/{}/delete", id);
        let mut params = HashMap::new();
        params.insert("tag".to_string(), tag.to_string());
        params.insert("purge".to_string(), purge.to_string());

        let service_token = self.account.read().await.get_service_token();
        params.insert("serviceToken".to_string(), service_token);

        let (_bytes, status) = self.do_request(reqwest::Method::POST, url, Some(params)).await?;
        if !status.is_success() {
            return Err(format!("Delete failed with status: {}", status).into());
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn create_folder(&self, subject: &str) -> crate::api::MiResult<Folder> {
        let url = "https://i.mi.com/note/folder/post";
        let mut params = HashMap::new();
        params.insert("subject".to_string(), subject.to_string());

        let service_token = self.account.read().await.get_service_token();
        params.insert("serviceToken".to_string(), service_token);

        let data: serde_json::Value = self.request_json(reqwest::Method::POST, url.to_string(), Some(params)).await?;

        if let Ok(res) = serde_json::from_value::<FolderResponse>(data.clone()) {
            Ok(res.folder)
        } else if let Ok(folder) = serde_json::from_value::<Folder>(data.clone()) {
            Ok(folder)
        } else {
            // Fallback for success (200 OK)
            Ok(Folder {
                id: "synthetic".to_string(),
                subject: subject.to_string(),
                tag: "".to_string(),
                ..Default::default()
            })
        }
    }

    #[allow(dead_code)]
    pub async fn delete_folder(&self, id: &str, tag: &str) -> crate::api::MiResult<()> {
        let url = format!("https://i.mi.com/note/folder/{}/delete", id);
        let mut params = HashMap::new();
        params.insert("tag".to_string(), tag.to_string());

        let service_token = self.account.read().await.get_service_token();
        params.insert("serviceToken".to_string(), service_token);

        let (_bytes, status) = self.do_request(reqwest::Method::POST, url, Some(params)).await?;
        if !status.is_success() {
            return Err(format!("Delete folder failed with status: {}", status).into());
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn update_folder(&self, id: &str, tag: &str, subject: &str) -> crate::api::MiResult<Folder> {
        let url = format!("https://i.mi.com/note/folder/{}", id);
        let mut params = HashMap::new();
        params.insert("tag".to_string(), tag.to_string());
        params.insert("subject".to_string(), subject.to_string());

        let service_token = self.account.read().await.get_service_token();
        params.insert("serviceToken".to_string(), service_token);

        let data: serde_json::Value = self.request_json(reqwest::Method::POST, url, Some(params)).await?;

        if let Ok(res) = serde_json::from_value::<FolderResponse>(data.clone()) {
            Ok(res.folder)
        } else if let Ok(folder) = serde_json::from_value::<Folder>(data.clone()) {
            Ok(folder)
        } else {
            // Fallback for success
            Ok(Folder {
                id: id.to_string(),
                subject: subject.to_string(),
                tag: tag.to_string(),
                ..Default::default()
            })
        }
    }

    fn extract_cookie_value(cookie_str: &str, name: &str) -> Option<String> {
        for part in cookie_str.split(';') {
            let part = part.trim();
            if part.starts_with(name) && part.contains('=') {
                let kv: Vec<&str> = part.splitn(2, '=').collect();
                if kv.len() == 2 && kv[0].trim() == name {
                    return Some(kv[1].trim().to_string());
                }
            }
        }
        None
    }
}
