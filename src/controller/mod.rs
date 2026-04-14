pub mod admin;
pub mod player;
pub mod sse;

use actix_web::web;

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.service(player::index)
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
