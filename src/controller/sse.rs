use actix_web::{get, web, Responder};
use actix_web_lab::sse;
use std::time::Duration;

use crate::{
    models::{get_active_game, get_blind_levels},
    views::build_timer_view,
    AppState,
};

#[get("/sse/timer")]
pub async fn sse_timer(state: web::Data<AppState>) -> impl Responder {
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    state.sse_senders.write().await.push(tx);
    sse::Sse::from_infallible_receiver(rx).with_keep_alive(Duration::from_secs(15))
}

pub async fn broadcast_loop(state: web::Data<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    loop {
        interval.tick().await;

        let html = match render_timer_fragment(&state).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!("SSE render error: {e}");
                continue;
            }
        };

        let event: sse::Event = sse::Data::new(html).event("timer").into();

        let mut senders = state.sse_senders.write().await;
        senders.retain(|tx| !tx.is_closed());
        for tx in senders.iter() {
            let _ = tx.try_send(event.clone());
        }
    }
}

async fn render_timer_fragment(state: &AppState) -> Result<String, Box<dyn std::error::Error>> {
    let game = get_active_game(&state.db).await?;

    let levels = match &game {
        Some(g) => get_blind_levels(&state.db, g.id).await.unwrap_or_default(),
        None => vec![],
    };

    let mut ctx = tera::Context::new();
    if let Some(ref g) = game {
        let timer = build_timer_view(g, &levels);
        ctx.insert("timer", &timer);
        ctx.insert("has_game", &true);
    } else {
        ctx.insert("has_game", &false);
    }

    Ok(state.tera.render("components/timer.html", &ctx)?)
}
