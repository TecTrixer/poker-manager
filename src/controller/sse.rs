use actix_web::{get, web, HttpRequest, Responder};
use actix_web_lab::sse;
use std::time::Duration;

use crate::{
    models::{get_active_game, get_blind_levels},
    views::build_timer_view,
    AppState,
};

#[get("/sse/timer")]
pub async fn sse_timer(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let peer = req
        .connection_info()
        .peer_addr()
        .unwrap_or("unknown")
        .to_string();
    let (tx, rx) = tokio::sync::mpsc::channel(8);
    let client_count = {
        let mut senders = state.sse_senders.write().await;
        senders.push(tx);
        senders.len()
    };
    tracing::info!(ip = %peer, clients = client_count, "SSE client connected");
    sse::Sse::from_infallible_receiver(rx).with_keep_alive(Duration::from_secs(15))
}

pub async fn broadcast_loop(state: web::Data<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    let mut prev_client_count = 0usize;
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
        let current_count = senders.len();
        if current_count != prev_client_count {
            tracing::info!(clients = current_count, "SSE client count changed");
            prev_client_count = current_count;
        }
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
