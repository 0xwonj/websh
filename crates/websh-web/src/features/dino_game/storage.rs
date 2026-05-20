const HIGH_SCORE_KEY: &str = "websh.dino_game.high_score.v1";

pub fn load_high_score() -> u32 {
    local_storage()
        .and_then(|storage| storage.get_item(HIGH_SCORE_KEY).ok().flatten())
        .and_then(|value| parse_high_score(&value))
        .unwrap_or(0)
}

pub fn save_high_score(score: u32) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(HIGH_SCORE_KEY, &format_high_score(score));
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn parse_high_score(value: &str) -> Option<u32> {
    value.trim().parse::<u32>().ok()
}

fn format_high_score(score: u32) -> String {
    score.to_string()
}

#[cfg(all(test, target_arch = "wasm32"))]
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
