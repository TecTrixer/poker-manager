pub mod admin;
pub mod player;
pub mod sse;

use actix_web::web;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(player::index)
        .service(player::rules)
        .service(sse::sse_timer)
        .service(admin::setup_get)
        .service(admin::setup_post)
        .service(admin::game_get)
        .service(admin::game_start)
        .service(admin::game_pause)
        .service(admin::game_resume)
        .service(admin::game_next_level)
        .service(admin::game_prev_level)
        .service(admin::game_reset)
        .service(admin::blind_row_component);
}
