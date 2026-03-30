use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct SuggestedLevel {
    pub index: usize,
    pub small: i64,
    pub big: i64,
    pub duration: i64,
    pub is_break: bool,
}

pub struct ChipInput {
    pub value: i64,
    pub total: i64,
}

pub struct ScheduleInput {
    pub chips: Vec<ChipInput>,
    pub num_players: i64,
    pub total_duration_mins: i64,
    pub level_duration_mins: i64,
}

const DENOMS: &[i64] = &[
    1, 2, 5, 10, 25, 50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 25_000, 50_000, 100_000,
];

/// Round up to the nearest clean poker denomination (ceiling).
fn round_to_clean(n: i64) -> i64 {
    if n <= 0 {
        return 1;
    }
    DENOMS
        .iter()
        .copied()
        .find(|&d| d >= n)
        .unwrap_or(*DENOMS.last().unwrap())
}

/// Advance to the next denomination strictly above `current`.
fn next_denomination(current: i64) -> i64 {
    DENOMS
        .iter()
        .copied()
        .find(|&d| d > current)
        .unwrap_or(current * 2)
}

pub fn suggest_schedule(input: &ScheduleInput) -> Vec<SuggestedLevel> {
    if input.num_players <= 0 || input.total_duration_mins <= 0 || input.level_duration_mins <= 0 {
        return vec![];
    }

    let total_value: i64 = input
        .chips
        .iter()
        .filter(|c| c.value > 0 && c.total > 0)
        .map(|c| c.value * c.total)
        .sum();
    if total_value == 0 {
        return vec![];
    }

    let starting_stack = total_value / input.num_players;
    let total_slots = (input.total_duration_mins / input.level_duration_mins) as usize;

    let mut levels = Vec::with_capacity(total_slots);
    let mut big = round_to_clean(starting_stack / 50);
    let mut playing_count = 0usize;

    for index in 0..total_slots {
        if playing_count == 4 {
            levels.push(SuggestedLevel {
                index,
                small: 0,
                big: 0,
                duration: input.level_duration_mins,
                is_break: true,
            });
            playing_count = 0;
        } else {
            let small = big / 2;
            levels.push(SuggestedLevel {
                index,
                small,
                big,
                duration: input.level_duration_mins,
                is_break: false,
            });
            big = next_denomination(big);
            playing_count += 1;
        }
    }
    levels
}
