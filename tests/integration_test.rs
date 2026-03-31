use poker_manager::models::{
    create_game, get_blind_levels, get_chip_types, get_game_by_id, insert_blind_level,
    insert_chip_type, pause_game, reset_game, resume_game, start_game,
};
use poker_manager::schedule::{suggest_schedule, ChipInput, ScheduleInput};
use sqlx::SqlitePool;

async fn setup_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory db");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");
    pool
}

#[tokio::test]
async fn test_create_and_get_game() {
    let pool = setup_db().await;
    let id = create_game(&pool, 2, 10).await.unwrap();
    let game = get_game_by_id(&pool, id).await.unwrap().unwrap();
    assert_eq!(game.num_tables, 2);
    assert_eq!(game.num_players, 10);
    assert_eq!(game.status, "pending");
    assert_eq!(game.current_level, 0);
    assert!(game.level_started_at.is_none());
}

#[tokio::test]
async fn test_blind_levels() {
    let pool = setup_db().await;
    let id = create_game(&pool, 1, 8).await.unwrap();
    insert_blind_level(&pool, id, 0, 25, 50, 1200, false)
        .await
        .unwrap();
    insert_blind_level(&pool, id, 1, 50, 100, 1200, false)
        .await
        .unwrap();
    insert_blind_level(&pool, id, 2, 0, 0, 600, true)
        .await
        .unwrap();
    let levels = get_blind_levels(&pool, id).await.unwrap();
    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0].level_num, 0);
    assert_eq!(levels[0].small_blind, 25);
    assert_eq!(levels[0].big_blind, 50);
    assert!(!levels[0].is_break);
    assert_eq!(levels[1].level_num, 1);
    assert_eq!(levels[2].level_num, 2);
    assert!(levels[2].is_break);
    assert_eq!(levels[2].small_blind, 0);
    assert_eq!(levels[2].big_blind, 0);
}

#[tokio::test]
async fn test_game_state_transitions() {
    let pool = setup_db().await;
    let id = create_game(&pool, 1, 6).await.unwrap();

    start_game(&pool, id).await.unwrap();
    let game = get_game_by_id(&pool, id).await.unwrap().unwrap();
    assert_eq!(game.status, "running");
    assert!(game.level_started_at.is_some());
    assert!(game.paused_at.is_none());

    pause_game(&pool, id).await.unwrap();
    let game = get_game_by_id(&pool, id).await.unwrap().unwrap();
    assert_eq!(game.status, "paused");
    assert!(game.paused_at.is_some());

    resume_game(&pool, id).await.unwrap();
    let game = get_game_by_id(&pool, id).await.unwrap().unwrap();
    assert_eq!(game.status, "running");
    assert!(game.paused_at.is_none());

    reset_game(&pool, id).await.unwrap();
    let game = get_game_by_id(&pool, id).await.unwrap().unwrap();
    assert_eq!(game.status, "pending");
    assert_eq!(game.current_level, 0);
    assert!(game.level_started_at.is_none());
}

#[tokio::test]
async fn test_chip_types() {
    let pool = setup_db().await;
    let id = create_game(&pool, 1, 8).await.unwrap();
    insert_chip_type(&pool, id, "White", 1, 100).await.unwrap();
    insert_chip_type(&pool, id, "Red", 5, 80).await.unwrap();
    let chips = get_chip_types(&pool, id).await.unwrap();
    assert_eq!(chips.len(), 2);
    // Ordered by value ascending
    assert_eq!(chips[0].color, "White");
    assert_eq!(chips[0].value, 1);
    assert_eq!(chips[0].total_count, 100);
    assert_eq!(chips[1].color, "Red");
    assert_eq!(chips[1].value, 5);
    assert_eq!(chips[1].total_count, 80);
}

#[test]
fn test_suggest_schedule_basic() {
    // 8 players, 100×1 + 80×5 = 500 total value → stack=62 → starting_big=1
    // 120 min / 20 min = 6 slots: play,play,play,play,break,play
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
    assert_eq!(levels.len(), 6);

    assert!(!levels[0].is_break);
    assert!(!levels[1].is_break);
    assert!(!levels[2].is_break);
    assert!(!levels[3].is_break);
    assert!(levels[4].is_break);
    assert!(!levels[5].is_break);

    for (i, l) in levels.iter().enumerate() {
        assert_eq!(l.index, i);
        assert_eq!(l.duration, 20);
    }

    assert_eq!(levels[4].small, 0);
    assert_eq!(levels[4].big, 0);

    let playing: Vec<_> = levels.iter().filter(|l| !l.is_break).collect();
    for w in playing.windows(2) {
        assert!(w[1].big >= w[0].big, "big blinds should be non-decreasing");
    }
}

#[test]
fn test_suggest_schedule_edge_cases() {
    // Zero players
    let result = suggest_schedule(&ScheduleInput {
        chips: vec![ChipInput { value: 5, total: 100 }],
        num_players: 0,
        total_duration_mins: 60,
        level_duration_mins: 15,
        rounds_before_break: 4,
    });
    assert!(result.is_empty());

    // Zero total duration
    let result = suggest_schedule(&ScheduleInput {
        chips: vec![ChipInput { value: 5, total: 100 }],
        num_players: 8,
        total_duration_mins: 0,
        level_duration_mins: 15,
        rounds_before_break: 4,
    });
    assert!(result.is_empty());

    // Zero chip value
    let result = suggest_schedule(&ScheduleInput {
        chips: vec![ChipInput { value: 0, total: 100 }],
        num_players: 8,
        total_duration_mins: 60,
        level_duration_mins: 15,
        rounds_before_break: 4,
    });
    assert!(result.is_empty());
}

#[test]
fn test_suggest_schedule_realistic() {
    // Typical tournament: 8 players, 100 chips of 1, 100 chips of 5, 50 chips of 25
    // Total value = 100 + 500 + 1250 = 1850 → stack = 231
    // starting_big = round_to_clean(231/50) = round_to_clean(4) = 5
    // 3 hours, 20 min levels = 9 slots
    let input = ScheduleInput {
        chips: vec![
            ChipInput { value: 1, total: 100 },
            ChipInput { value: 5, total: 100 },
            ChipInput { value: 25, total: 50 },
        ],
        num_players: 8,
        total_duration_mins: 180,
        level_duration_mins: 20,
        rounds_before_break: 4,
    };
    let levels = suggest_schedule(&input);
    assert_eq!(levels.len(), 9);
    // First 4 are play, 5th is break, next 4 are play
    assert!(levels[4].is_break);
    // All durations correct
    for l in &levels {
        assert_eq!(l.duration, 20);
    }
}
