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
    pub rounds_before_break: usize,
}

const DENOMS: &[i64] = &[
    1, 2, 5, 10, 25, 50, 100, 200, 500, 1_000, 2_000, 5_000, 10_000, 25_000, 50_000, 100_000,
];

/// Round `n` down to the nearest multiple of `unit`. Returns at least `unit`.
pub fn floor_to_unit(n: i64, unit: i64) -> i64 {
    if unit <= 1 {
        return n.max(1);
    }
    ((n / unit) * unit).max(unit)
}

/// Round `n` to the nearest multiple of `unit` (half-up). Returns at least `unit`.
pub fn round_to_unit(n: i64, unit: i64) -> i64 {
    if unit <= 1 {
        return n.max(1);
    }
    (((n + unit / 2) / unit) * unit).max(unit)
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

    // Minimum chip denomination — all blinds must be multiples of this.
    let min_chip = input
        .chips
        .iter()
        .filter(|c| c.value > 0 && c.total > 0)
        .map(|c| c.value)
        .min()
        .unwrap_or(1);

    let rounds_before_break = input.rounds_before_break.max(1);
    let starting_stack = total_value / input.num_players;
    let total_slots = (input.total_duration_mins / input.level_duration_mins) as usize;
    if total_slots == 0 {
        return vec![];
    }

    // Count playing slots in the schedule.
    let mut play_slot_count = 0usize;
    {
        let mut playing_count = 0usize;
        for _ in 0..total_slots {
            if playing_count == rounds_before_break {
                playing_count = 0;
            } else {
                play_slot_count += 1;
                playing_count += 1;
            }
        }
    }
    if play_slot_count == 0 {
        return vec![];
    }

    // Blind range:
    // BB_start = starting_stack / 50  (~50 big blinds per player at start)
    // BB_end   = starting_stack / 2   (~2 big blinds heads-up → game ends quickly)
    //            capped at total_value / 4 (blinds can never exceed chips in play)
    // All values rounded to multiples of min_chip.
    let bb_start = floor_to_unit((starting_stack / 50).max(min_chip), min_chip);
    let bb_end_raw = floor_to_unit((starting_stack / 2).max(bb_start), min_chip);
    let bb_end = bb_end_raw.min(total_value / 4).max(bb_start);

    // Build denomination sequence: only DENOMS that are multiples of min_chip,
    // spanning from bb_start to bb_end.
    let denom_sequence: Vec<i64> = {
        let mut seq: Vec<i64> = DENOMS
            .iter()
            .copied()
            .filter(|&d| d % min_chip == 0 && d >= bb_start && d <= bb_end)
            .collect();
        if seq.is_empty() {
            seq.push(bb_start);
        }
        seq
    };

    let n_denoms = denom_sequence.len();

    // Distribute play slots across the denomination sequence.
    // More slots than denominations → repeat earlier (lower) blinds.
    // Fewer slots than denominations → subsample evenly.
    let play_bigs: Vec<i64> = if play_slot_count <= n_denoms {
        (0..play_slot_count)
            .map(|i| {
                let idx = if play_slot_count == 1 {
                    0
                } else {
                    (i * (n_denoms - 1)) / (play_slot_count - 1)
                };
                denom_sequence[idx]
            })
            .collect()
    } else {
        let base = play_slot_count / n_denoms;
        let extra = play_slot_count % n_denoms;
        let mut bigs = Vec::with_capacity(play_slot_count);
        for (di, &big) in denom_sequence.iter().enumerate() {
            // Extra repetitions go to the lowest levels (blinds stay low longer early on).
            let reps = base + if di < extra { 1 } else { 0 };
            for _ in 0..reps {
                bigs.push(big);
            }
        }
        bigs
    };

    // Build the full schedule with breaks interspersed.
    let mut levels = Vec::with_capacity(total_slots);
    let mut playing_count = 0usize;
    let mut play_idx = 0usize;

    for index in 0..total_slots {
        if playing_count == rounds_before_break {
            levels.push(SuggestedLevel {
                index,
                small: 0,
                big: 0,
                duration: input.level_duration_mins,
                is_break: true,
            });
            playing_count = 0;
        } else {
            let big = play_bigs.get(play_idx).copied().unwrap_or(*denom_sequence.last().unwrap());
            // Small blind: half of big, rounded DOWN to nearest chip multiple.
            let small = floor_to_unit(big / 2, min_chip);
            levels.push(SuggestedLevel {
                index,
                small,
                big,
                duration: input.level_duration_mins,
                is_break: false,
            });
            play_idx += 1;
            playing_count += 1;
        }
    }

    levels
}
