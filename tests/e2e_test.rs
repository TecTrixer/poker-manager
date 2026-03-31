/// End-to-end tests: spin up the full actix-web app with an in-memory SQLite
/// database and exercise all common admin and player HTTP flows.
use actix_web::{test, web, App};
use poker_manager::{controller, AppState};
use sqlx::SqlitePool;
use tera::Tera;

// ── Test app factory ──────────────────────────────────────────────────────────

async fn make_app_state() -> web::Data<AppState> {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations failed");

    let tera = Tera::new("templates/**/*.html").expect("templates failed to load");

    web::Data::new(AppState {
        db: pool,
        tera,
        sse_senders: tokio::sync::RwLock::new(Vec::new()),
    })
}

/// Create a game via POST /admin/setup and return the body (redirects to /admin/game).
/// Returns the Location header of the redirect.
async fn post_setup(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    extra: &str,
) -> String {
    let form = format!(
        "num_tables=1&num_players=4\
         &chips[0][color]=White&chips[0][value]=5&chips[0][total]=80\
         &chips[1][color]=Red&chips[1][value]=25&chips[1][total]=40\
         &levels[0][is_break]=false&levels[0][small]=5&levels[0][big]=10&levels[0][duration]=15\
         &levels[1][is_break]=false&levels[1][small]=10&levels[1][big]=25&levels[1][duration]=15\
         &levels[2][is_break]=true&levels[2][small]=0&levels[2][big]=0&levels[2][duration]=10\
         &levels[3][is_break]=false&levels[3][small]=25&levels[3][big]=50&levels[3][duration]=15\
         &index=4{}",
        if extra.is_empty() { String::new() } else { format!("&{extra}") }
    );

    let req = test::TestRequest::post()
        .uri("/admin/setup")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(form)
        .to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(app, req).await;
    assert_eq!(
        resp.status().as_u16(),
        303,
        "POST /admin/setup should redirect"
    );
    resp.headers()
        .get("Location")
        .and_then(|v: &actix_web::http::header::HeaderValue| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

/// Assert a GET request returns HTTP 200 and body contains `needle`.
async fn assert_get_200(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
    needle: &str,
) {
    let req = test::TestRequest::get().uri(uri).to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(app, req).await;
    let status = resp.status().as_u16();
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).unwrap_or("");
    assert_eq!(
        status, 200,
        "GET {uri} expected 200, got {status}. Body: {}",
        &body_str[..body_str.len().min(400)]
    );
    if !needle.is_empty() {
        assert!(
            body_str.contains(needle),
            "GET {uri} body does not contain {needle:?}"
        );
    }
}

/// Assert a POST request redirects (303) to `expected_location`.
async fn assert_post_redirects(
    app: &impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    >,
    uri: &str,
    expected_location: &str,
) {
    let req = test::TestRequest::post().uri(uri).to_request();
    let resp: actix_web::dev::ServiceResponse = test::call_service(app, req).await;
    let status = resp.status().as_u16();
    assert_eq!(status, 303, "POST {uri} expected 303 redirect, got {status}");
    let loc = resp
        .headers()
        .get("Location")
        .and_then(|v: &actix_web::http::header::HeaderValue| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        loc, expected_location,
        "POST {uri} expected Location {expected_location:?}, got {loc:?}"
    );
}

// ── Player-facing pages ───────────────────────────────────────────────────────

#[actix_web::test]
async fn test_player_index_no_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    assert_get_200(&app, "/", "").await;
}

#[actix_web::test]
async fn test_player_index_with_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    // After setup, game is pending — index should render without 500
    assert_get_200(&app, "/", "").await;
}

#[actix_web::test]
async fn test_rules_page_no_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    assert_get_200(&app, "/rules", "Royal Flush").await;
}

#[actix_web::test]
async fn test_rules_page_with_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    assert_get_200(&app, "/rules", "Royal Flush").await;
}

// ── Admin setup ───────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_admin_setup_get_no_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    assert_get_200(&app, "/admin", "Game Setup").await;
}

#[actix_web::test]
async fn test_admin_setup_post_creates_game_and_redirects() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    let loc = post_setup(&app, "").await;
    assert_eq!(loc, "/admin/game");
}

#[actix_web::test]
async fn test_admin_setup_get_with_pending_game_shows_current_values() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    // /admin should show setup form populated with existing game data
    assert_get_200(&app, "/admin", "Game Setup").await;
}

#[actix_web::test]
async fn test_admin_setup_get_with_running_game_shows_notice() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    // Start the game
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;
    // Now /admin should show the setup form with a "running" notice (not redirect)
    assert_get_200(&app, "/admin", "running").await;
}

#[actix_web::test]
async fn test_admin_setup_reconfigure_running_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    // Create a game and start it
    post_setup(&app, "").await;
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;

    // Get the game id from /admin/game page
    let req = test::TestRequest::get().uri("/admin/game").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).unwrap();
    // Extract game_id — it appears in the hidden input or in the page
    // We know it's game 1 since it's the first game in a fresh DB.
    // Re-post setup with game_id=1 to reconfigure in-place.
    let form = "game_id=1&num_tables=2&num_players=6\
        &chips[0][color]=Blue&chips[0][value]=10&chips[0][total]=60\
        &levels[0][is_break]=false&levels[0][small]=10&levels[0][big]=20&levels[0][duration]=20\
        &index=1";
    let req = test::TestRequest::post()
        .uri("/admin/setup")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(form)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 303);
    // After reconfigure, game should still be running
    let _ = body_str; // suppress unused warning
    assert_get_200(&app, "/admin/game", "running").await;
}

// ── Game control ──────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_admin_game_get_redirects_without_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    let req = test::TestRequest::get().uri("/admin/game").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 303);
    assert_eq!(
        resp.headers().get("Location").and_then(|v| v.to_str().ok()),
        Some("/admin")
    );
}

#[actix_web::test]
async fn test_admin_game_get_with_pending_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    assert_get_200(&app, "/admin/game", "pending").await;
}

#[actix_web::test]
async fn test_admin_game_full_lifecycle() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;

    // Start
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "running").await;

    // Pause
    assert_post_redirects(&app, "/admin/game/pause", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "paused").await;

    // Resume
    assert_post_redirects(&app, "/admin/game/resume", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "running").await;

    // Next level
    assert_post_redirects(&app, "/admin/game/next", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "running").await;

    // Prev level
    assert_post_redirects(&app, "/admin/game/prev", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "running").await;

    // Reset
    assert_post_redirects(&app, "/admin/game/reset", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "pending").await;
}

#[actix_web::test]
async fn test_admin_game_next_stays_within_bounds() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;

    // Advance past all 4 levels — should not panic or 500
    for _ in 0..6 {
        assert_post_redirects(&app, "/admin/game/next", "/admin/game").await;
    }
    assert_get_200(&app, "/admin/game", "").await;
}

#[actix_web::test]
async fn test_admin_game_prev_at_first_level() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;
    // Prev on level 0 should stay at 0 without error
    assert_post_redirects(&app, "/admin/game/prev", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "").await;
}

// ── Speed adjustments ─────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_admin_accelerate_decelerate() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;

    // Accelerate twice
    assert_post_redirects(&app, "/admin/game/accelerate", "/admin/game").await;
    assert_post_redirects(&app, "/admin/game/accelerate", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "blinds +2 steps").await;

    // Decelerate once: net +1
    assert_post_redirects(&app, "/admin/game/decelerate", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "blinds +1 steps").await;

    // Decelerate back to 0: indicator should vanish
    assert_post_redirects(&app, "/admin/game/decelerate", "/admin/game").await;
    let req = test::TestRequest::get().uri("/admin/game").to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).unwrap();
    assert!(!body_str.contains("steps"), "speed indicator should be gone at steps=0");
}

#[actix_web::test]
async fn test_accelerate_adjusts_future_blind_display() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await;
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;

    // Accelerate once — future levels should show adjusted blinds
    assert_post_redirects(&app, "/admin/game/accelerate", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "was").await; // "(was X / Y)" appears for adjusted levels
}

// ── Schedule suggestion ───────────────────────────────────────────────────────

#[actix_web::test]
async fn test_suggest_schedule_endpoint() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    let form = "num_players=8&total_duration_mins=120&level_duration_mins=20\
        &rounds_before_break=4\
        &chips[0][color]=White&chips[0][value]=5&chips[0][total]=100\
        &chips[1][color]=Red&chips[1][value]=25&chips[1][total]=60";
    let req = test::TestRequest::post()
        .uri("/admin/suggest-schedule")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(form)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).unwrap();
    // Response is HTML fragment with level rows
    assert!(body_str.contains("level-row"), "should contain level rows");
}

#[actix_web::test]
async fn test_suggest_schedule_blinds_within_chip_range() {
    use poker_manager::schedule::{suggest_schedule, ChipInput, ScheduleInput};
    // Min chip = 25. All blinds must be multiples of 25.
    let input = ScheduleInput {
        chips: vec![
            ChipInput { value: 25, total: 40 },
            ChipInput { value: 100, total: 20 },
        ],
        num_players: 4,
        total_duration_mins: 120,
        level_duration_mins: 20,
        rounds_before_break: 4,
    };
    let levels = suggest_schedule(&input);
    assert!(!levels.is_empty());
    let total_value = 25 * 40 + 100 * 20; // 3000
    for l in &levels {
        if l.is_break {
            assert_eq!(l.small, 0);
            assert_eq!(l.big, 0);
        } else {
            assert_eq!(l.big % 25, 0, "big blind {} not multiple of 25", l.big);
            assert_eq!(l.small % 25, 0, "small blind {} not multiple of 25", l.small);
            assert!(l.big <= total_value / 4, "big blind {} exceeds chip cap {}", l.big, total_value / 4);
        }
    }
}

#[actix_web::test]
async fn test_suggest_schedule_blinds_within_chip_range_small_chips() {
    use poker_manager::schedule::{suggest_schedule, ChipInput, ScheduleInput};
    // Min chip = 1. Standard test.
    let input = ScheduleInput {
        chips: vec![
            ChipInput { value: 1, total: 100 },
            ChipInput { value: 5, total: 80 },
        ],
        num_players: 8,
        total_duration_mins: 120,
        level_duration_mins: 20,
        rounds_before_break: 4,
    };
    let levels = suggest_schedule(&input);
    let total_value = 100 + 400; // 500
    for l in &levels {
        if !l.is_break {
            assert!(l.big <= total_value / 4, "big blind {} exceeds chip cap {}", l.big, total_value / 4);
            assert!(l.big > 0, "big blind must be > 0");
        }
    }
}

// ── HTMX component: blind-row ─────────────────────────────────────────────────

#[actix_web::test]
async fn test_blind_row_component() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    assert_get_200(&app, "/admin/components/blind-row?index=5", "level-row").await;
}

#[actix_web::test]
async fn test_blind_row_component_oob_hidden() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    let req = test::TestRequest::get()
        .uri("/admin/components/blind-row?index=5")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body = test::read_body(resp).await;
    let body_str = std::str::from_utf8(&body).unwrap();
    // The OOB level_count input must be type="hidden" (not a visible field)
    assert!(
        body_str.contains("type=\"hidden\""),
        "OOB level_count input must have type=hidden, got: {body_str}"
    );
    // index should increment: index=5 → next_index=6
    assert!(body_str.contains("value=\"6\""), "next_index should be 6");
}

// ── Game management ───────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_admin_games_list_empty() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    assert_get_200(&app, "/admin/games", "Games").await;
}

#[actix_web::test]
async fn test_admin_games_new_creates_and_redirects() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    assert_post_redirects(&app, "/admin/games/new", "/admin/games").await;
    // Game list should now show the game
    assert_get_200(&app, "/admin/games", "pending").await;
}

#[actix_web::test]
async fn test_admin_games_select_changes_active_game() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    // Create two games via setup (each replaces the previous selection)
    post_setup(&app, "").await; // game id=1, selected
    assert_post_redirects(&app, "/admin/games/new", "/admin/games").await; // game id=2, now selected

    // Select game 1 again
    assert_post_redirects(&app, "/admin/games/1/select", "/admin/games").await;
    // Game 1 should now be the active game — /admin/game should show it
    assert_get_200(&app, "/admin/game", "").await;
}

#[actix_web::test]
async fn test_admin_games_delete() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    post_setup(&app, "").await; // game id=1
    assert_post_redirects(&app, "/admin/games/new", "/admin/games").await; // game id=2, selected
    // Delete game 1 (not selected)
    assert_post_redirects(&app, "/admin/games/1/delete", "/admin/games").await;
    // Game list should still load
    assert_get_200(&app, "/admin/games", "Games").await;
}

// ── SSE endpoint ──────────────────────────────────────────────────────────────

#[actix_web::test]
async fn test_sse_timer_returns_event_stream() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;
    let req = test::TestRequest::get().uri("/sse/timer").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 200);
    let content_type = resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        content_type.contains("text/event-stream"),
        "SSE endpoint must return text/event-stream, got: {content_type}"
    );
}

// ── Full admin workflow ───────────────────────────────────────────────────────

#[actix_web::test]
async fn test_full_admin_workflow() {
    let state = make_app_state().await;
    let app = test::init_service(
        App::new().app_data(state).configure(controller::routes),
    )
    .await;

    // 1. Player page before any game
    assert_get_200(&app, "/", "").await;

    // 2. Setup a game
    let loc = post_setup(&app, "").await;
    assert_eq!(loc, "/admin/game");

    // 3. Admin setup page shows current game data
    assert_get_200(&app, "/admin", "Game Setup").await;

    // 4. Game control panel shows pending
    assert_get_200(&app, "/admin/game", "pending").await;

    // 5. Player page now shows timer
    assert_get_200(&app, "/", "").await;

    // 6. Start game
    assert_post_redirects(&app, "/admin/game/start", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "running").await;

    // 7. Player page during running game
    assert_get_200(&app, "/", "").await;

    // 8. Rules page during running game shows chip values
    assert_get_200(&app, "/rules", "White").await;

    // 9. Pause
    assert_post_redirects(&app, "/admin/game/pause", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "paused").await;

    // 10. Reconfigure while paused
    let form = "game_id=1&num_tables=1&num_players=4\
        &chips[0][color]=White&chips[0][value]=5&chips[0][total]=80\
        &levels[0][is_break]=false&levels[0][small]=5&levels[0][big]=10&levels[0][duration]=15\
        &levels[1][is_break]=false&levels[1][small]=10&levels[1][big]=25&levels[1][duration]=15\
        &index=2";
    let req = test::TestRequest::post()
        .uri("/admin/setup")
        .insert_header(("Content-Type", "application/x-www-form-urlencoded"))
        .set_payload(form)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status().as_u16(), 303);
    // Game should still be paused after reconfigure
    assert_get_200(&app, "/admin/game", "paused").await;

    // 11. Resume
    assert_post_redirects(&app, "/admin/game/resume", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "running").await;

    // 12. Accelerate future blinds
    assert_post_redirects(&app, "/admin/game/accelerate", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "blinds +1 steps").await;

    // 13. Decelerate (cancel acceleration)
    assert_post_redirects(&app, "/admin/game/decelerate", "/admin/game").await;

    // 14. Next level
    assert_post_redirects(&app, "/admin/game/next", "/admin/game").await;

    // 15. Reset
    assert_post_redirects(&app, "/admin/game/reset", "/admin/game").await;
    assert_get_200(&app, "/admin/game", "pending").await;
}
