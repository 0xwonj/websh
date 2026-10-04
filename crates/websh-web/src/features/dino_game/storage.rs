use crate::runtime::state::{self, DINO_SCORE_KEY};

pub fn load_high_score() -> u32 {
    state::read(DINO_SCORE_KEY)
        .and_then(|value| parse_high_score(&value))
        .unwrap_or(0)
}

pub fn save_high_score(score: u32) {
    let _ = state::write(DINO_SCORE_KEY, &format_high_score(score));
}

fn parse_high_score(value: &str) -> Option<u32> {
    value.trim().parse::<u32>().ok()
}

fn format_high_score(score: u32) -> String {
    score.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn parses_high_score_values() {
        assert_eq!(parse_high_score("42"), Some(42));
        assert_eq!(parse_high_score(" 00042 "), Some(42));
    }

    #[wasm_bindgen_test]
    fn rejects_invalid_high_score_values() {
        assert_eq!(parse_high_score(""), None);
        assert_eq!(parse_high_score("abc"), None);
        assert_eq!(parse_high_score("-1"), None);
    }

    #[wasm_bindgen_test]
    fn formats_high_score_values() {
        assert_eq!(format_high_score(42), "42");
    }
}
