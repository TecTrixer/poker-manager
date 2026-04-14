pub mod admin;
pub mod player;
pub mod sse;

use actix_web::{get, web, HttpRequest, HttpResponse};

pub(super) fn peer_ip(req: &HttpRequest) -> String {
    if let Some(val) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = val.to_str() {
            if let Some(first) = s.split(',').next() {
                let ip = first.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }
    if let Some(val) = req.headers().get("x-real-ip") {
        if let Ok(s) = val.to_str() {
            let ip = s.trim();
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    let info = req.connection_info();
    info.peer_addr().unwrap_or("unknown").to_string()
}

#[get("/health")]
async fn health() -> HttpResponse {
    HttpResponse::Ok().finish()
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(health)
        .service(player::index)
        .service(player::rules)
        .service(sse::sse_timer)
        .service(admin::admin_login_page)
        .service(admin::admin_login_post)
        .service(admin::setup_get)
        .service(admin::setup_post)
        .service(admin::game_get)
        .service(admin::game_start)
        .service(admin::game_pause)
        .service(admin::game_resume)
        .service(admin::game_next_level)
        .service(admin::game_prev_level)
        .service(admin::game_reset)
        .service(admin::game_accelerate)
        .service(admin::game_decelerate)
        .service(admin::game_set_players)
        .service(admin::blind_row_component)
        .service(admin::suggest_schedule_handler)
        .service(admin::games_list)
        .service(admin::games_new)
        .service(admin::games_select)
        .service(admin::games_delete);
}
