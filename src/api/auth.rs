use reqwest::{header, Client as HttpClient, redirect};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiAccount {
    pub cookie: String,
    pub timeout: Duration,
    pub user_agent: String,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct LoginUrlResp {
    data: LoginUrlData,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct LoginUrlData {
    #[serde(rename = "loginUrl")]
    login_url: String,
}

impl MiAccount {
    pub fn new(cookie: &str) -> Self {
        Self {
            cookie: cookie.to_string(),
            timeout: Duration::from_secs(5),
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string(),
        }
    }

    #[allow(dead_code)]
    pub fn with_ua(mut self, ua: String) -> Self {
        self.user_agent = ua;
        self
    }

    fn now_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }

    pub async fn gen_micloud_cookie(&self) -> crate::api::MiResult<String> {
        let service_login_url = self.get_login_url().await?;
        let sts_url = self.get_sts_url(&service_login_url).await?;
        let new_cookies = self.get_micloud_cookie_final(&sts_url).await?;

        // 合并原始 Cookie 和新获取的 STS Cookie
        Ok(self.merge_cookies(&self.cookie, &new_cookies))
    }

    pub fn merge_cookies(&self, old: &str, new: &str) -> String {
        let mut mp = HashMap::new();

        let mut add_to_map = |s: &str| {
            for part in s.split(';') {
                let part = part.trim();
                if part.is_empty() { continue; }
                let kv: Vec<&str> = part.splitn(2, '=').collect();
                if kv.len() == 2 {
                    mp.insert(kv[0].trim().to_string(), kv[1].trim().to_string());
                }
            }
        };

        add_to_map(old);
        add_to_map(new);

        let mut res = Vec::new();
        let mut keys: Vec<_> = mp.keys().collect();
        keys.sort();

        for k in keys {
            let v = mp.get(k).unwrap();
            res.push(format!("{}={}", k, v));
        }

        let merged = res.join("; ");
        merged
    }

    async fn get_login_url(&self) -> crate::api::MiResult<String> {
        let url = format!("https://i.mi.com/api/user/login?&followUp=https%3A%2F%2Fi.mi.com%2F&_locale=zh_CN&ts={}", Self::now_ms());
        let client = HttpClient::new();
        let resp = client.get(&url).send().await?;
        let json: LoginUrlResp = resp.json().await?;
        Ok(json.data.login_url)
    }

    async fn get_sts_url(&self, service_login_url: &str) -> crate::api::MiResult<String> {
        let client = HttpClient::builder()
            .redirect(redirect::Policy::none())
            .build()?;

        let resp = client.get(service_login_url)
            .header(header::COOKIE, &self.cookie)
            .header(header::USER_AGENT, &self.user_agent)
            .send().await?;

        println!("STS Redirect Resp Status: {}", resp.status());

        if let Some(location) = resp.headers().get(header::LOCATION) {
            let loc_str = location.to_str()?.to_string();
            println!("STS Redirect Location: {}", loc_str);
            Ok(loc_str)
        } else {
            let body = resp.text().await?;
            println!("STS Redirect Body (no location): {}", body);
            Err("no location in service login resp".into())
        }
    }

    async fn get_micloud_cookie_final(&self, sts_url: &str) -> crate::api::MiResult<String> {
        let client = HttpClient::builder()
            .redirect(redirect::Policy::none())
            .build()?;

        let resp = client.get(sts_url)
            .header(header::COOKIE, &self.cookie)
            .header(header::USER_AGENT, &self.user_agent)
            .send().await?;

        println!("Final Cookie Resp Status: {}", resp.status());

        let headers = resp.headers().clone();
        let cookies: Vec<String> = headers
            .get_all(header::SET_COOKIE)
            .iter()
            .filter_map(|h| h.to_str().ok())
            .map(|s| s.to_string())
            .collect();

        if cookies.is_empty() {
            return Err("Session expired. Please login again via browser to get a fresh cookie.".into());
        }

        let combined_cookies = cookies.join("; ");
        let tidied = self.tidy_kvs(&combined_cookies);
        println!("Generated MiCloud Cookie: {}", tidied);
        Ok(tidied)
    }

    pub fn tidy_kvs(&self, s: &str) -> String {
        let mut mp = HashMap::new();

        for part in s.split(|c| c == ',' || c == ';') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() < 2 {
                continue;
            }
            let k = kv[0].trim();
            let v = kv[1].trim();

            let k_lower = k.to_lowercase();
            // Preserve domain-prefixed cookies for i.mi.com
            // only skip generic attributes that are not part of the identity
            if k_lower == "path" || k_lower == "domain" || k_lower == "expires" || k_lower == "secure" || k_lower == "httponly" || k_lower == "samesite" || k_lower == "max-age" {
                continue;
            }

            if !v.is_empty() && v != "\"\"" {
                mp.insert(k.to_string(), v.to_string());
            }
        }

        let mut res = Vec::new();
        for (k, v) in mp {
            res.push(format!("{}={}", k, v));
        }
        res.join("; ")
    }

    #[allow(dead_code)]
    pub fn get_service_token(&self) -> String {
        let st = self.get_value_by_key("serviceToken");
        if !st.is_empty() { return st; }
        // check with domain prefix if needed, but usually just serviceToken
        self.get_value_by_key("serviceToken")
    }

    #[allow(dead_code)]
    fn get_value_by_key(&self, key: &str) -> String {
        for pair in self.cookie.split(';') {
            let kv: Vec<&str> = pair.splitn(2, '=').collect();
            if kv.len() < 2 {
                continue;
            }
            if kv[0].trim() == key {
                return kv[1].trim().to_string();
            }
        }
        String::new()
    }
}
