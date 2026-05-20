use super::state::GameCommand;

pub fn keydown_command(key: &str) -> Option<GameCommand> {
    match key {
        " " | "ArrowUp" | "w" | "W" => Some(GameCommand::Jump),
        "ArrowDown" | "s" | "S" => Some(GameCommand::DuckStart),
        "Enter" => Some(GameCommand::Restart),
        _ => None,
    }
}

pub fn keyup_command(key: &str) -> Option<GameCommand> {
    match key {
        "ArrowDown" | "s" | "S" => Some(GameCommand::DuckEnd),
        _ => None,
    }
}
