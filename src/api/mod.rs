pub mod models;
pub mod client;
pub mod auth;
pub mod config;

pub use client::Client;
pub use config::AppConfig;
// pub use models::*;

pub type MiError = Box<dyn std::error::Error + Send + Sync>;
pub type MiResult<T> = Result<T, MiError>;
