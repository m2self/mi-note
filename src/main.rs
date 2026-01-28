mod api;
mod webview;

use tokio::time::{sleep, Duration};
use crate::api::Client;
use crate::webview::WebViewManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting MiNote WebView...");

    let mut manager = WebViewManager::new();
    let cookies_arc = manager.cookies.clone();

    // Spawn background task to monitor cookies and perform API calls
    tokio::spawn(async move {
        println!("Background API monitor started.");
        let mut client: Option<Client> = None;

        loop {
            let cookie_opt = {
                let guard = cookies_arc.lock().unwrap();
                guard.clone()
            };

            if let Some(cookies) = cookie_opt {
                if client.is_none() {
                    println!("Cookies detected! Initializing API client...");
                    let config = api::AppConfig::load();
                    client = Some(Client::new(&cookies, config.user_agent));
                }

                if let Some(ref c) = client {
                    println!("--- Background API Operation ---");
                    match c.list_notes(10).await {
                        Ok(notes) => {
                            println!("Found {} notes in background:", notes.entries.len());
                            for note in notes.entries.iter().take(3) {
                                println!("  - [{}] {}", note.id, note.subject);
                            }
                        }
                        Err(e) => {
                            println!("Background API error: {}", e);
                            // If it's an auth error, we might need to clear client and wait for new cookies
                            if e.to_string().contains("Authentication") {
                                client = None;
                            }
                        }
                    }
                }
            } else {
                // Check if we have saved cookies in config
                let config = api::AppConfig::load();
                if let Some(saved_cookies) = config.account_cookie {
                    println!("Using saved cookies from config...");
                    client = Some(Client::new(&saved_cookies, config.user_agent));

                    // Put saved cookies into the shared state so UI doesn't overwrite if not needed
                    let mut guard = cookies_arc.lock().unwrap();
                    *guard = Some(saved_cookies);
                    continue; // Re-eval loop
                }
            }

            sleep(Duration::from_secs(60)).await;
        }
    });

    // Run WebView on the main thread (Win32 requirement for UI)
    manager.run();

    Ok(())
}
