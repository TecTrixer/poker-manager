use actix_web::{get, web, HttpResponse};

use crate::{
    models::{get_active_game, get_blind_levels, get_chip_types},
    views::{build_chip_distribution, build_timer_view},
    AppState,
};

#[get("/")]
pub async fn index(state: web::Data<AppState>) -> HttpResponse {
    let game = match get_active_game(&state.db).await {
        Ok(g) => g,
        Err(e) => {
            tracing::error!("DB error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let mut ctx = tera::Context::new();

    if let Some(game) = game {
        let levels = get_blind_levels(&state.db, game.id).await.unwrap_or_default();
        let chips = get_chip_types(&state.db, game.id).await.unwrap_or_default();
        let timer = build_timer_view(&game, &levels);
        let chip_dist = build_chip_distribution(&chips, game.num_players);
        ctx.insert("timer", &timer);
        ctx.insert("chips", &chip_dist);
        ctx.insert("has_game", &true);
    } else {
        ctx.insert("has_game", &false);
    }

    match state.tera.render("pages/index.html", &ctx) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            tracing::error!("Template error: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/rules")]
pub async fn rules(state: web::Data<AppState>) -> HttpResponse {
    let game = get_active_game(&state.db).await.unwrap_or(None);
    let mut ctx = tera::Context::new();

    if let Some(game) = game {
        let chips = get_chip_types(&state.db, game.id).await.unwrap_or_default();
        ctx.insert("chips", &chips);
        ctx.insert("has_game", &true);
    } else {
        ctx.insert("has_game", &false);
        ctx.insert("chips", &Vec::<crate::models::ChipType>::new());
    }

    match state.tera.render("pages/rules.html", &ctx) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            tracing::error!("Template error: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}
