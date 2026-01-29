#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            println!($($arg)*);
        }
    };
}

mod api;
mod webview;
mod state;
mod gui;

use tokio::time::{sleep, Duration};
use crate::api::Client;
use crate::webview::WebViewManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dprintln!("Starting MiNote WebView...");

    let mut manager = WebViewManager::new();
    let cookies_arc = manager.cookies.clone();

    // Spawn background task to monitor cookies and perform API calls
    tokio::spawn(async move {
        dprintln!("Background API monitor started.");
        let mut client: Option<Client> = None;
        let mut last_cookies: Option<String> = None;
        let mut last_ua: Option<String> = None;

        loop {
            let (cookie_opt, current_ua) = {
                let config = api::AppConfig::load();
                let guard = cookies_arc.lock().unwrap();
                (guard.clone(), config.user_agent.clone())
            };

            if let Some(cookies) = cookie_opt {
                let ua_changed = current_ua != last_ua;
                let cookie_changed = Some(cookies.clone()) != last_cookies;

                if client.is_none() || ua_changed || cookie_changed {
                    dprintln!("[Background API] Initializing/Updating API client (UA changed: {}, Cookie changed: {})", ua_changed, cookie_changed);
                    client = Some(Client::new(&cookies, current_ua.clone()));
                    last_cookies = Some(cookies.clone());
                    last_ua = current_ua;
                }

                if let Some(ref c) = client {
                    dprintln!("--- Background API Operation ---");
                    // Add a timeout to the future itself just in case
                    let list_future = c.list_notes(100);
                    match tokio::time::timeout(Duration::from_secs(45), list_future).await {
                        Ok(Ok(notes)) => {
                            dprintln!("Found {} notes in background.", notes.entries.len());
                            state::update_notes(notes.entries);
                        }
                        Ok(Err(e)) => {
                            eprintln!("[Background API Error] API reported error: {:?}", e);
                            if e.to_string().contains("Authentication") {
                                client = None;
                            }
                        }
                        Err(_) => {
                            eprintln!("[Background API Error] Request timed out after 45s");
                        }
                    }
                }
            } else {
                // Check if we have saved cookies in config
                let config = api::AppConfig::load();
                if let Some(saved_cookies) = config.account_cookie {
                    dprintln!("Using saved cookies from config...");
                    let mut guard = cookies_arc.lock().unwrap();
                    *guard = Some(saved_cookies);
                    continue;
                }
            }

            sleep(Duration::from_secs(60)).await;
        }
    });

    // Run WebView on the main thread (Win32 requirement for UI)
    manager.run();

    Ok(())
}
