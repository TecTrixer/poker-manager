use chrono::Utc;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct Game {
    pub id: i64,
    pub status: String,
    pub num_tables: i64,
    pub num_players: i64,
    pub current_level: i64,
    pub level_started_at: Option<i64>,
    pub paused_at: Option<i64>,
    pub paused_duration_secs: i64,
    pub selected: i64,
    pub name: String,
    pub speed_steps: i64,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct BlindLevel {
    pub id: i64,
    pub game_id: i64,
    pub level_num: i64,
    pub small_blind: i64,
    pub big_blind: i64,
    pub duration_secs: i64,
    pub is_break: bool,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ChipType {
    pub id: i64,
    pub game_id: i64,
    pub color: String,
    pub value: i64,
    pub total_count: i64,
}

impl Game {
    pub fn seconds_elapsed(&self) -> i64 {
        match self.level_started_at {
            None => 0,
            Some(started) => {
                let effective_now = self.paused_at.unwrap_or_else(|| Utc::now().timestamp());
                (effective_now - started - self.paused_duration_secs).max(0)
            }
        }
    }

    pub fn seconds_remaining(&self, level: &BlindLevel) -> i64 {
        (level.duration_secs - self.seconds_elapsed()).max(0)
    }

}

pub async fn get_active_game(pool: &SqlitePool) -> sqlx::Result<Option<Game>> {
    sqlx::query_as::<_, Game>(
        "SELECT id, status, num_tables, num_players, current_level,
                level_started_at, paused_at, paused_duration_secs, selected, name, speed_steps
         FROM games WHERE selected = 1 AND status != 'ended' LIMIT 1",
    )
    .fetch_optional(pool)
    .await
}

pub async fn get_game_by_id(pool: &SqlitePool, id: i64) -> sqlx::Result<Option<Game>> {
    sqlx::query_as::<_, Game>(
        "SELECT id, status, num_tables, num_players, current_level,
                level_started_at, paused_at, paused_duration_secs, selected, name, speed_steps
         FROM games WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_all_games(pool: &SqlitePool) -> sqlx::Result<Vec<Game>> {
    sqlx::query_as::<_, Game>(
        "SELECT id, status, num_tables, num_players, current_level,
                level_started_at, paused_at, paused_duration_secs, selected, name, speed_steps
         FROM games ORDER BY id DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn get_blind_levels(pool: &SqlitePool, game_id: i64) -> sqlx::Result<Vec<BlindLevel>> {
    sqlx::query_as::<_, BlindLevel>(
        "SELECT id, game_id, level_num, small_blind, big_blind, duration_secs, is_break
         FROM blind_levels WHERE game_id = ? ORDER BY level_num",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
}

pub async fn get_chip_types(pool: &SqlitePool, game_id: i64) -> sqlx::Result<Vec<ChipType>> {
    sqlx::query_as::<_, ChipType>(
        "SELECT id, game_id, color, value, total_count
         FROM chip_types WHERE game_id = ? ORDER BY value",
    )
    .bind(game_id)
    .fetch_all(pool)
    .await
}

pub async fn create_game(
    pool: &SqlitePool,
    num_tables: i64,
    num_players: i64,
) -> sqlx::Result<i64> {
    // Deselect all others first
    sqlx::query("UPDATE games SET selected = 0")
        .execute(pool)
        .await?;
    let row = sqlx::query(
        "INSERT INTO games (status, num_tables, num_players, selected, name) VALUES ('pending', ?, ?, 1, '') RETURNING id",
    )
    .bind(num_tables)
    .bind(num_players)
    .fetch_one(pool)
    .await?;
    use sqlx::Row;
    Ok(row.get::<i64, _>("id"))
}

pub async fn select_game(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE games SET selected = 0")
        .execute(pool)
        .await?;
    sqlx::query("UPDATE games SET selected = 1 WHERE id = ?")
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_game(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM blind_levels WHERE game_id = ?")
        .bind(game_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM chip_types WHERE game_id = ?")
        .bind(game_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM games WHERE id = ?")
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn adjust_speed(pool: &SqlitePool, game_id: i64, delta: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE games SET speed_steps = speed_steps + ? WHERE id = ?")
        .bind(delta)
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_game_name(pool: &SqlitePool, game_id: i64, name: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE games SET name = ? WHERE id = ?")
        .bind(name)
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_blind_level(
    pool: &SqlitePool,
    game_id: i64,
    level_num: i64,
    small_blind: i64,
    big_blind: i64,
    duration_secs: i64,
    is_break: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO blind_levels (game_id, level_num, small_blind, big_blind, duration_secs, is_break)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(game_id)
    .bind(level_num)
    .bind(small_blind)
    .bind(big_blind)
    .bind(duration_secs)
    .bind(is_break)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn insert_chip_type(
    pool: &SqlitePool,
    game_id: i64,
    color: &str,
    value: i64,
    total_count: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO chip_types (game_id, color, value, total_count) VALUES (?, ?, ?, ?)",
    )
    .bind(game_id)
    .bind(color)
    .bind(value)
    .bind(total_count)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn end_all_games(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query("UPDATE games SET status = 'ended' WHERE status != 'ended'")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_game_players(
    pool: &SqlitePool,
    game_id: i64,
    num_tables: i64,
    num_players: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE games SET num_tables = ?, num_players = ? WHERE id = ?")
        .bind(num_tables)
        .bind(num_players)
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_chip_types(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM chip_types WHERE game_id = ?")
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete_blind_levels(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM blind_levels WHERE game_id = ?")
        .bind(game_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn start_game(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE games SET status = 'running', current_level = 0,
         level_started_at = ?, paused_at = NULL, paused_duration_secs = 0
         WHERE id = ?",
    )
    .bind(now)
    .bind(game_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn pause_game(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE games SET status = 'paused', paused_at = ? WHERE id = ? AND status = 'running'",
    )
    .bind(now)
    .bind(game_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn resume_game(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    let game = get_game_by_id(pool, game_id).await?;
    if let Some(game) = game {
        if let Some(paused_at) = game.paused_at {
            let now = Utc::now().timestamp();
            let new_paused_duration = game.paused_duration_secs + (now - paused_at);
            sqlx::query(
                "UPDATE games SET status = 'running', paused_at = NULL, paused_duration_secs = ?
                 WHERE id = ?",
            )
            .bind(new_paused_duration)
            .bind(game_id)
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

pub async fn set_level(pool: &SqlitePool, game_id: i64, level: i64) -> sqlx::Result<()> {
    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE games SET current_level = ?, level_started_at = ?,
         paused_at = NULL, paused_duration_secs = 0 WHERE id = ?",
    )
    .bind(level)
    .bind(now)
    .bind(game_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_game(pool: &SqlitePool, game_id: i64) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE games SET status = 'pending', current_level = 0,
         level_started_at = NULL, paused_at = NULL, paused_duration_secs = 0,
         speed_steps = 0
         WHERE id = ?",
    )
    .bind(game_id)
    .execute(pool)
    .await?;
    Ok(())
}
