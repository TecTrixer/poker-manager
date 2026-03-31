use actix_files::Files;
use actix_web::{middleware::DefaultHeaders, web, App, HttpServer};
use tera::Tera;
use tracing::info;
use tracing_subscriber::EnvFilter;

use poker_manager::{controller, db, AppState};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:poker.db?mode=rwc".to_string());

    let db = db::init(&database_url)
        .await
        .expect("Failed to initialize database");

    let tera = Tera::new("templates/**/*.html").expect("Failed to load templates");

    let state_data = web::Data::new(AppState {
        db,
        tera,
        sse_senders: tokio::sync::RwLock::new(Vec::new()),
    });

    let state_for_loop = state_data.clone();
    tokio::spawn(async move {
        controller::sse::broadcast_loop(state_for_loop).await;
    });

    info!("Listening on http://0.0.0.0:{port}");

    HttpServer::new(move || {
        App::new()
            .app_data(state_data.clone())
            .service(
                web::scope("/static")
                    .wrap(DefaultHeaders::new().add(("Cache-Control", "public, max-age=604800")))
                    .service(Files::new("", "static").prefer_utf8(true)),
            )
            .configure(controller::routes)
    })
    .bind(format!("0.0.0.0:{port}"))?
    .run()
    .await
}
