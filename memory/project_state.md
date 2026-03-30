---
name: Poker Manager Project State
description: Current implementation status and key architectural decisions for the poker-manager app
type: project
---

Basic functionality has been implemented. The app builds and all routes return 200.

**Why:** This is a from-scratch Rust/actix-web poker tournament timer app.

**How to apply:** Use this as the baseline when implementing new features.

## Stack
- Rust + actix-web 4, actix-web-lab (SSE), sqlx (runtime queries, no compile-time macros), tera templates, serde_qs for form parsing
- SQLite with `sqlite:poker.db?mode=rwc` URL

## Architecture
- `web::Data<AppState>` (NOT `web::Data<Arc<AppState>>`) — actix's `Data::from(Arc<T>)` gives `Data<T>`
- Background task `broadcast_loop` receives `web::Data<AppState>` clone
- SSE senders stored in `AppState.sse_senders: RwLock<Vec<mpsc::Sender<sse::Event>>>`
- All DB queries use runtime `sqlx::query_as::<_, T>()` (no `query!` macro) to avoid compile-time DATABASE_URL dependency
- Models (Game, BlindLevel, ChipType) derive `sqlx::FromRow` + `serde::Serialize`

## Routes implemented
- GET / — player timer page (uses SSE for live updates)
- GET /rules — rules reference
- GET /sse/timer — SSE stream (event name: "timer")
- GET /admin — setup form (redirects to /admin/game if game is active)
- POST /admin/setup — saves game config
- GET /admin/game — game control panel
- POST /admin/game/{start,pause,resume,next,prev,reset}
- GET /admin/components/blind-row?index=N — HTMX component for dynamic form rows

## Timer logic
- `level_started_at` + `paused_duration_secs` + optional `paused_at` track time precisely
- SSE sends timer HTML fragment every 1 second

## Key files
- src/main.rs — server setup, AppState definition
- src/models/game.rs — all DB queries
- src/views/mod.rs — view structs (LevelView has duration_mins field)
- src/controller/{player,admin,sse}.rs — handlers
- templates/ — tera templates
- static/ — htmx.min.js, sse.js, style.css (dark theme)
