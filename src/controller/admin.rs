use actix_web::{cookie::Cookie, get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use tera::Context;

use crate::{
    models::{
        adjust_speed, create_game, delete_blind_levels, delete_chip_types, delete_game,
        get_active_game, get_all_games, get_blind_levels, get_chip_types, insert_blind_level,
        insert_chip_type, pause_game, reset_game, reset_speed, resume_game, select_game,
        set_level, set_players_left, start_game, update_game_players,
    },
    views::{build_timer_view, level_label, LevelAdminView},
    AppState,
};

fn peer_ip(req: &HttpRequest) -> String {
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

fn redirect(path: &str) -> HttpResponse {
    HttpResponse::SeeOther()
        .insert_header(("Location", path))
        .finish()
}

fn render(state: &AppState, template: &str, ctx: &Context) -> HttpResponse {
    match state.tera.render(template, ctx) {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => {
            tracing::error!("Template error {template}: {e}");
            HttpResponse::InternalServerError().finish()
        }
    }
}

fn is_logged_in(req: &HttpRequest, state: &AppState) -> bool {
    req.cookie("admin_token")
        .map(|c| c.value() == state.admin_password)
        .unwrap_or(false)
}

#[get("/admin/login")]
pub async fn admin_login_page(state: web::Data<AppState>) -> HttpResponse {
    let ctx = Context::new();
    render(&state, "pages/admin/login.html", &ctx)
}

#[derive(Deserialize)]
pub struct LoginForm {
    pub password: String,
}

#[post("/admin/login")]
pub async fn admin_login_post(
    state: web::Data<AppState>,
    form: web::Form<LoginForm>,
) -> HttpResponse {
    if form.password == state.admin_password {
        let cookie = Cookie::build("admin_token", state.admin_password.clone())
            .path("/admin")
            .http_only(true)
            .finish();
        HttpResponse::SeeOther()
            .cookie(cookie)
            .insert_header(("Location", "/admin"))
            .finish()
    } else {
        let mut ctx = Context::new();
        ctx.insert("error", "Wrong password");
        render(&state, "pages/admin/login.html", &ctx)
    }
}

// ── Setup ─────────────────────────────────────────────────────────────────────

#[get("/admin")]
pub async fn setup_get(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if !is_logged_in(&req, &state) { return redirect("/admin/login"); }
    let ip = peer_ip(&req);
    tracing::info!(ip = %ip, "GET /admin setup page");
    let game = get_active_game(&state.db).await.unwrap_or(None);

    let mut ctx = Context::new();
    if let Some(ref g) = game {
        let chips = get_chip_types(&state.db, g.id).await.unwrap_or_default();
        let mut levels = get_blind_levels(&state.db, g.id).await.unwrap_or_default();

        // Pre-populate the form with speed-adjusted values so the admin can accept them by saving.
        if g.speed_steps != 0 {
            let speed_factor = 1.25_f64.powi(g.speed_steps as i32);
            let min_chip = chips.iter().filter(|c| c.value > 0).map(|c| c.value).min().unwrap_or(1);
            let current_idx = g.current_level as usize;
            for level in levels.iter_mut() {
                if (level.level_num as usize) > current_idx && !level.is_break {
                    level.big_blind = crate::schedule::round_to_unit(
                        (level.big_blind as f64 * speed_factor).round() as i64, min_chip,
                    );
                    level.small_blind = crate::schedule::floor_to_unit(
                        (level.small_blind as f64 * speed_factor).round() as i64, min_chip,
                    );
                }
            }
        }

        ctx.insert("chips", &chips);
        ctx.insert("levels", &levels);
        ctx.insert("speed_steps", &g.speed_steps);
    }
    ctx.insert("game", &game);
    ctx.insert("num_rows", &12i64);
    render(&state, "pages/admin/setup.html", &ctx)
}

#[derive(Debug, Deserialize)]
pub struct SetupForm {
    pub game_id: Option<i64>,
    pub num_tables: i64,
    pub num_players: i64,
    pub chips: Vec<ChipFormEntry>,
    pub levels: Vec<LevelFormEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ChipFormEntry {
    pub color: String,
    pub value: i64,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct LevelFormEntry {
    #[serde(default)]
    pub small: i64,
    #[serde(default)]
    pub big: i64,
    pub duration: i64,
    #[serde(default)]
    pub is_break: String,
}

#[post("/admin/setup")]
pub async fn setup_post(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let ip = peer_ip(&req);
    let qs_config = serde_qs::Config::new(5, false);
    let form: SetupForm = match qs_config.deserialize_bytes(&body) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(ip = %ip, "Form parse error: {e}");
            return HttpResponse::BadRequest().body("Invalid form data");
        }
    };
    tracing::info!(
        ip = %ip,
        game_id = ?form.game_id,
        num_tables = form.num_tables,
        num_players = form.num_players,
        chip_count = form.chips.len(),
        level_count = form.levels.len(),
        "POST /admin/setup"
    );

    // If editing an existing game, update in place (preserves timer state)
    // Otherwise, end all games and create a fresh one
    let game_id = if let Some(gid) = form.game_id {
        if let Err(e) = update_game_players(&state.db, gid, form.num_tables, form.num_players).await {
            tracing::error!("DB error updating game: {e}");
            return HttpResponse::InternalServerError().finish();
        }
        if let Err(e) = delete_chip_types(&state.db, gid).await {
            tracing::error!("DB error deleting chips: {e}");
            return HttpResponse::InternalServerError().finish();
        }
        if let Err(e) = delete_blind_levels(&state.db, gid).await {
            tracing::error!("DB error deleting levels: {e}");
            return HttpResponse::InternalServerError().finish();
        }
        gid
    } else {
        match create_game(&state.db, form.num_tables, form.num_players).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("DB error: {e}");
                return HttpResponse::InternalServerError().finish();
            }
        }
    };

    // Insert chip types (skip empty rows)
    for chip in &form.chips {
        if chip.color.trim().is_empty() || chip.total == 0 {
            continue;
        }
        if let Err(e) = insert_chip_type(&state.db, game_id, &chip.color, chip.value, chip.total).await {
            tracing::error!("DB error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    }

    // Insert blind levels (skip rows with duration=0 and not a break)
    let mut level_num = 0i64;
    for level in &form.levels {
        if level.duration == 0 {
            continue;
        }
        let is_break = level.is_break == "true";
        let small = if is_break { 0 } else { level.small };
        let big = if is_break { 0 } else { level.big };
        if let Err(e) = insert_blind_level(
            &state.db,
            game_id,
            level_num,
            small,
            big,
            level.duration * 60,
            is_break,
        )
        .await
        {
            tracing::error!("DB error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
        level_num += 1;
    }

    // Reset speed_steps so the adjusted values are now the new baseline.
    let _ = reset_speed(&state.db, game_id).await;

    redirect("/admin/game")
}

// ── Game control ──────────────────────────────────────────────────────────────

#[get("/admin/game")]
pub async fn game_get(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if !is_logged_in(&req, &state) { return redirect("/admin/login"); }
    let game = match get_active_game(&state.db).await {
        Ok(Some(g)) => g,
        Ok(None) => return redirect("/admin"),
        Err(e) => {
            tracing::error!("DB error: {e}");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let levels = get_blind_levels(&state.db, game.id).await.unwrap_or_default();
    let chips = get_chip_types(&state.db, game.id).await.unwrap_or_default();

    let timer = build_timer_view(&game, &levels);

    let current_idx = game.current_level as usize;

    // speed_steps: positive = faster blinds (1.25^n multiplier on future blind values)
    //              negative = slower blinds (0.8^n multiplier, i.e. 1/1.25)
    // 1.25 and 0.8 are exact inverses, so +1 then -1 = no change.
    let speed_steps = game.speed_steps;
    let speed_factor = 1.25_f64.powi(speed_steps as i32);

    // Minimum chip value — adjusted blinds must always be multiples of this.
    let min_chip = chips
        .iter()
        .filter(|c| c.value > 0)
        .map(|c| c.value)
        .min()
        .unwrap_or(1);

    let levels_admin: Vec<LevelAdminView> = levels
        .iter()
        .map(|l| {
            // Apply multiplier only to future levels (strictly after current)
            let is_future = (l.level_num as usize) > current_idx;
            let (adjusted_big, adjusted_small) = if is_future && !l.is_break && speed_steps != 0 {
                let big_raw = (l.big_blind as f64 * speed_factor).round() as i64;
                let small_raw = (l.small_blind as f64 * speed_factor).round() as i64;
                let big = crate::schedule::round_to_unit(big_raw, min_chip);
                let small = crate::schedule::floor_to_unit(small_raw, min_chip);
                (big, small)
            } else {
                (l.big_blind, l.small_blind)
            };
            LevelAdminView {
                level_num: l.level_num,
                small_blind: l.small_blind,
                big_blind: l.big_blind,
                duration_secs: l.duration_secs,
                duration_mins: l.duration_secs / 60,
                is_break: l.is_break,
                label: level_label(l),
                is_current: l.level_num == game.current_level,
                adjusted_small_blind: adjusted_small,
                adjusted_big_blind: adjusted_big,
                is_adjusted: is_future && !l.is_break && speed_steps != 0,
            }
        })
        .collect();

    let mut ctx = Context::new();
    ctx.insert("game_id", &game.id);
    ctx.insert("status", &game.status);
    ctx.insert("num_tables", &game.num_tables);
    ctx.insert("num_players", &game.num_players);
    ctx.insert("current_level", &game.current_level);
    ctx.insert("total_levels", &levels.len());
    ctx.insert("levels", &levels_admin);
    ctx.insert("chips", &chips);
    ctx.insert("timer", &timer);
    ctx.insert("has_game", &true);
    ctx.insert("speed_steps", &speed_steps);
    ctx.insert("players_left", &game.players_left);

    render(&state, "pages/admin/game.html", &ctx)
}

// ── Actions ───────────────────────────────────────────────────────────────────

#[post("/admin/game/start")]
pub async fn game_start(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, "POST /admin/game/start");
        let _ = start_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/pause")]
pub async fn game_pause(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, "POST /admin/game/pause");
        let _ = pause_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/resume")]
pub async fn game_resume(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, "POST /admin/game/resume");
        let _ = resume_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/next")]
pub async fn game_next_level(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let levels = get_blind_levels(&state.db, game.id).await.unwrap_or_default();
        let next = game.current_level + 1;
        if next < levels.len() as i64 {
            tracing::info!(ip = %ip, game_id = game.id, from_level = game.current_level, to_level = next, "POST /admin/game/next");
            let _ = set_level(&state.db, game.id, next).await;
        }
    }
    redirect("/admin/game")
}

#[post("/admin/game/prev")]
pub async fn game_prev_level(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let prev = (game.current_level - 1).max(0);
        tracing::info!(ip = %ip, game_id = game.id, from_level = game.current_level, to_level = prev, "POST /admin/game/prev");
        let _ = set_level(&state.db, game.id, prev).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/reset")]
pub async fn game_reset(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, "POST /admin/game/reset");
        let _ = reset_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/accelerate")]
pub async fn game_accelerate(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, old_speed_steps = game.speed_steps, "POST /admin/game/accelerate");
        let _ = adjust_speed(&state.db, game.id, 1).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/decelerate")]
pub async fn game_decelerate(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, old_speed_steps = game.speed_steps, "POST /admin/game/decelerate");
        let _ = adjust_speed(&state.db, game.id, -1).await;
    }
    redirect("/admin/game")
}

#[derive(Deserialize)]
pub struct PlayersForm {
    pub count: i64,
}

#[post("/admin/game/players")]
pub async fn game_set_players(
    state: web::Data<AppState>,
    req: HttpRequest,
    form: web::Form<PlayersForm>,
) -> HttpResponse {
    let ip = peer_ip(&req);
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        tracing::info!(ip = %ip, game_id = game.id, players_left = form.count, "POST /admin/game/players");
        let _ = set_players_left(&state.db, game.id, form.count).await;
    }
    redirect("/admin/game")
}

// ── Suggest schedule ──────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct SuggestScheduleForm {
    pub num_players: i64,
    pub total_duration_mins: i64,
    pub level_duration_mins: i64,
    #[serde(default = "default_rounds_before_break")]
    pub rounds_before_break: i64,
    pub chips: Vec<ChipFormEntry>,
}

fn default_rounds_before_break() -> i64 { 4 }

#[post("/admin/suggest-schedule")]
pub async fn suggest_schedule_handler(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Bytes,
) -> HttpResponse {
    let ip = peer_ip(&req);
    let qs_config = serde_qs::Config::new(5, false);
    let form: SuggestScheduleForm = match qs_config.deserialize_bytes(&body) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(ip = %ip, "suggest-schedule parse error: {e}");
            return HttpResponse::BadRequest().body("Invalid form data");
        }
    };
    tracing::info!(
        ip = %ip,
        num_players = form.num_players,
        total_duration_mins = form.total_duration_mins,
        level_duration_mins = form.level_duration_mins,
        rounds_before_break = form.rounds_before_break,
        "POST /admin/suggest-schedule"
    );
    let chip_inputs: Vec<crate::schedule::ChipInput> = form
        .chips
        .iter()
        .map(|c| crate::schedule::ChipInput {
            value: c.value,
            total: c.total,
        })
        .collect();
    let input = crate::schedule::ScheduleInput {
        chips: chip_inputs,
        num_players: form.num_players,
        total_duration_mins: form.total_duration_mins,
        level_duration_mins: form.level_duration_mins,
        rounds_before_break: form.rounds_before_break.max(1) as usize,
    };
    let levels = crate::schedule::suggest_schedule(&input);
    let level_count = levels.len();
    let mut ctx = Context::new();
    ctx.insert("levels", &levels);
    ctx.insert("level_count", &level_count);
    render(&state, "components/suggested_levels.html", &ctx)
}

// ── HTMX components ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BlindRowQuery {
    pub index: usize,
}

#[get("/admin/components/blind-row")]
pub async fn blind_row_component(
    state: web::Data<AppState>,
    query: web::Query<BlindRowQuery>,
) -> HttpResponse {
    let mut ctx = Context::new();
    ctx.insert("index", &query.index);
    ctx.insert("oob_update", &true);
    ctx.insert("next_index", &(query.index + 1));
    render(&state, "components/blind_row.html", &ctx)
}

// ── Game management ───────────────────────────────────────────────────────────

#[get("/admin/games")]
pub async fn games_list(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if !is_logged_in(&req, &state) { return redirect("/admin/login"); }
    let ip = peer_ip(&req);
    tracing::info!(ip = %ip, "GET /admin/games");
    let games = get_all_games(&state.db).await.unwrap_or_default();
    let active = get_active_game(&state.db).await.unwrap_or(None);
    let mut ctx = Context::new();
    ctx.insert("games", &games);
    ctx.insert("active_game", &active);
    render(&state, "pages/admin/games.html", &ctx)
}

#[post("/admin/games/new")]
pub async fn games_new(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    let ip = peer_ip(&req);
    tracing::info!(ip = %ip, "POST /admin/games/new — creating new game");
    match create_game(&state.db, 1, 8).await {
        Ok(id) => {
            tracing::info!(ip = %ip, game_id = id, "New game created");
        }
        Err(e) => {
            tracing::error!(ip = %ip, "DB error creating game: {e}");
        }
    }
    redirect("/admin/games")
}

#[derive(Deserialize)]
pub struct GameIdPath {
    pub id: i64,
}

#[post("/admin/games/{id}/select")]
pub async fn games_select(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<GameIdPath>,
) -> HttpResponse {
    let ip = peer_ip(&req);
    let game_id = path.id;
    tracing::info!(ip = %ip, game_id = game_id, "POST /admin/games/select");
    if let Err(e) = select_game(&state.db, game_id).await {
        tracing::error!(ip = %ip, "DB error selecting game: {e}");
    }
    redirect("/admin/games")
}

#[post("/admin/games/{id}/delete")]
pub async fn games_delete(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<GameIdPath>,
) -> HttpResponse {
    let ip = peer_ip(&req);
    let game_id = path.id;
    tracing::info!(ip = %ip, game_id = game_id, "POST /admin/games/delete");
    if let Err(e) = delete_game(&state.db, game_id).await {
        tracing::error!(ip = %ip, "DB error deleting game: {e}");
    }
    redirect("/admin/games")
}
