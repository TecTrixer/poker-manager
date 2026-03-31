use crate::models::{BlindLevel, ChipType, Game};
use serde::Serialize;

#[derive(Serialize)]
pub struct TimerView {
    pub status: String,
    pub current_level: Option<LevelView>,
    pub next_level: Option<LevelView>,
    pub seconds_remaining: i64,
    pub time_display: String,
    pub level_num: i64,
    pub total_levels: usize,
}

#[derive(Serialize)]
pub struct LevelView {
    pub small_blind: i64,
    pub big_blind: i64,
    pub duration_secs: i64,
    pub duration_mins: i64,
    pub is_break: bool,
    pub label: String,
}

#[derive(Serialize)]
pub struct ChipView {
    pub color: String,
    pub value: i64,
    pub chips_per_player: i64,
}

#[derive(Serialize)]
pub struct LevelAdminView {
    pub level_num: i64,
    pub small_blind: i64,
    pub big_blind: i64,
    pub duration_secs: i64,
    pub duration_mins: i64,
    pub is_break: bool,
    pub label: String,
    pub is_current: bool,
    /// Adjusted blind values for future levels when speed_steps != 0.
    pub adjusted_small_blind: i64,
    pub adjusted_big_blind: i64,
    pub is_adjusted: bool,
}

pub fn format_time(secs: i64) -> String {
    let m = secs / 60;
    let s = secs % 60;
    format!("{:02}:{:02}", m, s)
}

pub fn level_label(level: &BlindLevel) -> String {
    if level.is_break {
        "BREAK".to_string()
    } else {
        format!("{} / {}", level.small_blind, level.big_blind)
    }
}

pub fn build_timer_view(
    game: &Game,
    levels: &[BlindLevel],
) -> TimerView {
    let current_idx = game.current_level as usize;
    let current = levels.get(current_idx);
    let next = if game.status == "pending" {
        levels.get(current_idx)
    } else {
        levels.get(current_idx + 1)
    };

    let (seconds_remaining, time_display) = match current {
        Some(lvl) => {
            let secs = game.seconds_remaining(lvl);
            (secs, format_time(secs))
        }
        None => (0, "00:00".to_string()),
    };

    TimerView {
        status: game.status.clone(),
        current_level: current.map(|l| LevelView {
            small_blind: l.small_blind,
            big_blind: l.big_blind,
            duration_secs: l.duration_secs,
            duration_mins: l.duration_secs / 60,
            is_break: l.is_break,
            label: level_label(l),
        }),
        next_level: next.map(|l| LevelView {
            small_blind: l.small_blind,
            big_blind: l.big_blind,
            duration_secs: l.duration_secs,
            duration_mins: l.duration_secs / 60,
            is_break: l.is_break,
            label: level_label(l),
        }),
        seconds_remaining,
        time_display,
        level_num: game.current_level + 1,
        total_levels: levels.len(),
    }
}

pub fn build_chip_distribution(chips: &[ChipType], num_players: i64) -> Vec<ChipView> {
    chips
        .iter()
        .map(|c| {
            let chips_per_player = if num_players > 0 {
                c.total_count / num_players
            } else {
                0
            };
            ChipView {
                color: c.color.clone(),
                value: c.value,
                chips_per_player,
            }
        })
        .collect()
}
