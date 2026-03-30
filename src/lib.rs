use actix_web_lab::sse;
use tokio::sync::RwLock;
use tera::Tera;

pub mod controller;
pub mod db;
pub mod models;
pub mod schedule;
pub mod views;

pub struct AppState {
    pub db: sqlx::SqlitePool,
    pub tera: Tera,
    pub sse_senders: RwLock<Vec<tokio::sync::mpsc::Sender<sse::Event>>>,
}
