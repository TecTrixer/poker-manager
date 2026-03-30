use actix_web::{get, post, web, HttpResponse};
use serde::Deserialize;
use tera::Context;

use crate::{
    models::{
        create_game, end_all_games, get_active_game, get_blind_levels,
        get_chip_types, insert_blind_level, insert_chip_type, pause_game, reset_game, resume_game,
        set_level, start_game,
    },
    views::{build_timer_view, format_time, level_label, LevelAdminView},
    AppState,
};

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

// ── Setup ─────────────────────────────────────────────────────────────────────

#[get("/admin")]
pub async fn setup_get(state: web::Data<AppState>) -> HttpResponse {
    let game = get_active_game(&state.db).await.unwrap_or(None);

    // If there's an active game that has been started, go to game control
    if let Some(ref g) = game {
        if g.status != "pending" {
            return redirect("/admin/game");
        }
    }

    let mut ctx = Context::new();
    ctx.insert("game", &game);
    ctx.insert("num_rows", &12i64);
    render(&state, "pages/admin/setup.html", &ctx)
}

#[derive(Debug, Deserialize)]
pub struct SetupForm {
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
    body: web::Bytes,
) -> HttpResponse {
    let form: SetupForm = match serde_qs::from_bytes(&body) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!("Form parse error: {e}");
            return HttpResponse::BadRequest().body("Invalid form data");
        }
    };

    // End any existing games
    if let Err(e) = end_all_games(&state.db).await {
        tracing::error!("DB error: {e}");
        return HttpResponse::InternalServerError().finish();
    }

    let game_id = match create_game(&state.db, form.num_tables, form.num_players).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("DB error: {e}");
            return HttpResponse::InternalServerError().finish();
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

    redirect("/admin/game")
}

// ── Game control ──────────────────────────────────────────────────────────────

#[get("/admin/game")]
pub async fn game_get(state: web::Data<AppState>) -> HttpResponse {
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

    let current_idx = game.current_level as usize;
    let current_level_info = levels.get(current_idx).map(|l| crate::views::LevelView {
        small_blind: l.small_blind,
        big_blind: l.big_blind,
        duration_secs: l.duration_secs,
        duration_mins: l.duration_secs / 60,
        is_break: l.is_break,
        label: level_label(l),
    });
    let next_level_info = levels.get(current_idx + 1).map(|l| crate::views::LevelView {
        small_blind: l.small_blind,
        big_blind: l.big_blind,
        duration_secs: l.duration_secs,
        duration_mins: l.duration_secs / 60,
        is_break: l.is_break,
        label: level_label(l),
    });

    let secs_remaining = current_level_info
        .as_ref()
        .map(|_| {
            levels
                .get(current_idx)
                .map(|l| game.seconds_remaining(l))
                .unwrap_or(0)
        })
        .unwrap_or(0);

    let levels_admin: Vec<LevelAdminView> = levels
        .iter()
        .map(|l| LevelAdminView {
            level_num: l.level_num,
            small_blind: l.small_blind,
            big_blind: l.big_blind,
            duration_secs: l.duration_secs,
            duration_mins: l.duration_secs / 60,
            is_break: l.is_break,
            label: level_label(l),
            is_current: l.level_num == game.current_level,
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
    ctx.insert("current_level_info", &current_level_info);
    ctx.insert("next_level_info", &next_level_info);
    ctx.insert("seconds_remaining", &secs_remaining);
    ctx.insert("time_display", &format_time(secs_remaining));

    render(&state, "pages/admin/game.html", &ctx)
}

// ── Actions ───────────────────────────────────────────────────────────────────

#[post("/admin/game/start")]
pub async fn game_start(state: web::Data<AppState>) -> HttpResponse {
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let _ = start_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/pause")]
pub async fn game_pause(state: web::Data<AppState>) -> HttpResponse {
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let _ = pause_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/resume")]
pub async fn game_resume(state: web::Data<AppState>) -> HttpResponse {
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let _ = resume_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/next")]
pub async fn game_next_level(state: web::Data<AppState>) -> HttpResponse {
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let levels = get_blind_levels(&state.db, game.id).await.unwrap_or_default();
        let next = game.current_level + 1;
        if next < levels.len() as i64 {
            let _ = set_level(&state.db, game.id, next).await;
            // If game was running, keep it running (set_level already clears pause)
        }
    }
    redirect("/admin/game")
}

#[post("/admin/game/prev")]
pub async fn game_prev_level(state: web::Data<AppState>) -> HttpResponse {
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let prev = (game.current_level - 1).max(0);
        let _ = set_level(&state.db, game.id, prev).await;
    }
    redirect("/admin/game")
}

#[post("/admin/game/reset")]
pub async fn game_reset(state: web::Data<AppState>) -> HttpResponse {
    if let Ok(Some(game)) = get_active_game(&state.db).await {
        let _ = reset_game(&state.db, game.id).await;
    }
    redirect("/admin/game")
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
