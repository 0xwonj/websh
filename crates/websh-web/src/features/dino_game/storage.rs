use crate::runtime::state::{self, DINO_SCORE_KEY};

pub fn load_high_score() -> u32 {
    state::read(DINO_SCORE_KEY)
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(0)
}

pub fn save_high_score(score: u32) {
    let _ = state::write(DINO_SCORE_KEY, &score.to_string());
}
