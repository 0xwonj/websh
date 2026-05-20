pub const WORLD_WIDTH: f32 = 600.0;
pub const WORLD_HEIGHT: f32 = 150.0;
pub const GROUND_Y: f32 = 140.0;

const DINO_X: f32 = 50.0;
const DINO_WIDTH: f32 = 44.0;
const DINO_HEIGHT: f32 = 47.0;
const DINO_DUCK_WIDTH: f32 = 59.0;
const DINO_DUCK_HEIGHT: f32 = 25.0;
const INITIAL_SPEED: f32 = 360.0;
const MAX_SPEED: f32 = 780.0;
const SPEED_RAMP: f32 = 0.0045;
const SCORE_COEFFICIENT: f32 = 0.025;
const GRAVITY: f32 = 2160.0;
const JUMP_VELOCITY: f32 = -600.0;
const FAST_DROP_IMPULSE: f32 = 280.0;
const MIN_SPAWN_DISTANCE: f32 = 155.0;
const MAX_SPAWN_DISTANCE: f32 = 255.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GamePhase {
    Ready,
    Running,
    Crashed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameCommand {
    Jump,
    DuckStart,
    DuckEnd,
    Restart,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ObstacleKind {
    CactusSmall,
    CactusLarge,
    Flyer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    fn shrink(self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - amount * 2.0).max(0.0),
            height: (self.height - amount * 2.0).max(0.0),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Obstacle {
    pub kind: ObstacleKind,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DinoUpdateOutcome {
    pub crashed: bool,
    pub high_score_changed: bool,
}

#[derive(Clone, Copy, Debug)]
struct LocalRng {
    state: u32,
}

impl LocalRng {
    const fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    fn next_f32(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        ((self.state >> 8) as f32) / ((u32::MAX >> 8) as f32)
    }
}

#[derive(Clone, Debug)]
pub struct DinoGameState {
    phase: GamePhase,
    dino_y: f32,
    velocity_y: f32,
    ducking: bool,
    distance: f32,
    speed: f32,
    spawn_distance: f32,
    obstacles: Vec<Obstacle>,
    rng: LocalRng,
    high_score: u32,
}

impl Default for DinoGameState {
    fn default() -> Self {
        Self::new()
    }
}

impl DinoGameState {
    pub fn new() -> Self {
        Self::new_with_high_score(0)
    }

    pub fn new_with_high_score(high_score: u32) -> Self {
        Self {
            phase: GamePhase::Ready,
            dino_y: GROUND_Y - DINO_HEIGHT,
            velocity_y: 0.0,
            ducking: false,
            distance: 0.0,
            speed: INITIAL_SPEED,
            spawn_distance: 96.0,
            obstacles: Vec::new(),
            rng: LocalRng::new(0xD1A0_600D),
            high_score,
        }
    }

    pub fn phase(&self) -> GamePhase {
        self.phase
    }

    pub fn score(&self) -> u32 {
        (self.distance * SCORE_COEFFICIENT).round() as u32
    }

    pub fn high_score(&self) -> u32 {
        self.high_score
    }

    pub fn distance(&self) -> f32 {
        self.distance
    }

    pub fn dino_rect(&self) -> Rect {
        let (width, height) = if self.ducking && self.grounded() {
            (DINO_DUCK_WIDTH, DINO_DUCK_HEIGHT)
        } else {
            (DINO_WIDTH, DINO_HEIGHT)
        };
        Rect {
            x: DINO_X,
            y: GROUND_Y - height - self.airborne_offset(),
            width,
            height,
        }
    }

    pub fn dino_visual_rect(&self) -> Rect {
        let width = if self.ducking && self.grounded() {
            DINO_DUCK_WIDTH
        } else {
            DINO_WIDTH
        };
        Rect {
            x: DINO_X,
            y: self.dino_y,
            width,
            height: DINO_HEIGHT,
        }
    }

    pub fn dino_ducking(&self) -> bool {
        self.ducking && self.grounded()
    }

    pub fn grounded(&self) -> bool {
        self.dino_y >= GROUND_Y - DINO_HEIGHT
    }

    pub fn obstacles(&self) -> &[Obstacle] {
        &self.obstacles
    }

    pub fn command(&mut self, command: GameCommand) {
        match command {
            GameCommand::Jump => self.jump_or_start(),
            GameCommand::Restart => {
                if self.phase == GamePhase::Crashed {
                    self.restart();
                } else if self.phase == GamePhase::Ready {
                    self.phase = GamePhase::Running;
                }
            }
            GameCommand::DuckStart => {
                if self.phase == GamePhase::Running {
                    self.ducking = true;
                    if !self.grounded() {
                        self.velocity_y += FAST_DROP_IMPULSE;
                    }
                }
            }
            GameCommand::DuckEnd => {
                self.ducking = false;
            }
        }
    }

    pub fn update(&mut self, dt_seconds: f32) -> DinoUpdateOutcome {
        if self.phase != GamePhase::Running {
            return DinoUpdateOutcome::default();
        }

        let dt = dt_seconds.clamp(0.0, 0.05);
        self.distance += self.speed * dt;
        self.speed = (INITIAL_SPEED + self.distance * SPEED_RAMP).min(MAX_SPEED);
        self.spawn_distance -= self.speed * dt;

        self.update_dino(dt);
        self.update_obstacles(dt);

        if self.spawn_distance <= 0.0 {
            self.spawn_obstacle();
            self.spawn_distance = self.next_spawn_distance();
        }

        if self.collides() {
            self.phase = GamePhase::Crashed;
            self.velocity_y = 0.0;
            self.ducking = false;
            return DinoUpdateOutcome {
                crashed: true,
                high_score_changed: self.record_high_score(),
            };
        }

        DinoUpdateOutcome::default()
    }

    fn jump_or_start(&mut self) {
        match self.phase {
            GamePhase::Ready => {
                self.phase = GamePhase::Running;
                self.velocity_y = JUMP_VELOCITY;
                self.dino_y = GROUND_Y - DINO_HEIGHT - 0.1;
            }
            GamePhase::Running if self.grounded() => {
                self.ducking = false;
                self.velocity_y = JUMP_VELOCITY;
                self.dino_y = GROUND_Y - DINO_HEIGHT - 0.1;
            }
            GamePhase::Crashed => self.restart(),
            GamePhase::Running => {}
        }
    }

    fn restart(&mut self) {
        let high_score = self.high_score;
        *self = Self::new_with_high_score(high_score);
        self.phase = GamePhase::Running;
    }

    fn update_dino(&mut self, dt: f32) {
        if self.grounded() && self.velocity_y >= 0.0 {
            self.dino_y = GROUND_Y - DINO_HEIGHT;
            self.velocity_y = 0.0;
            return;
        }

        self.velocity_y += GRAVITY * dt;
        self.dino_y += self.velocity_y * dt;

        let ground_top = GROUND_Y - DINO_HEIGHT;
        if self.dino_y >= ground_top {
            self.dino_y = ground_top;
            self.velocity_y = 0.0;
        }
    }

    fn update_obstacles(&mut self, dt: f32) {
        let travel = self.speed * dt;
        for obstacle in &mut self.obstacles {
            obstacle.rect.x -= travel;
        }
        self.obstacles
            .retain(|obstacle| obstacle.rect.x + obstacle.rect.width > -24.0);
    }

    fn spawn_obstacle(&mut self) {
        let score = self.score();
        let roll = self.rng.next_f32();
        let kind = if score > 120 && roll > 0.72 {
            ObstacleKind::Flyer
        } else if roll > 0.52 {
            ObstacleKind::CactusLarge
        } else {
            ObstacleKind::CactusSmall
        };
        let (width, height, y) = match kind {
            ObstacleKind::CactusSmall => (17.0, 35.0, GROUND_Y - 35.0),
            ObstacleKind::CactusLarge => (25.0, 50.0, GROUND_Y - 50.0),
            ObstacleKind::Flyer => (46.0, 40.0, 75.0),
        };
        self.obstacles.push(Obstacle {
            kind,
            rect: Rect {
                x: WORLD_WIDTH + 16.0,
                y,
                width,
                height,
            },
        });
    }

    fn next_spawn_distance(&mut self) -> f32 {
        let spread = MAX_SPAWN_DISTANCE - MIN_SPAWN_DISTANCE;
        let speed_bonus = ((self.speed - INITIAL_SPEED) / (MAX_SPEED - INITIAL_SPEED)).max(0.0);
        MIN_SPAWN_DISTANCE + self.rng.next_f32() * spread + speed_bonus * 34.0
    }

    fn collides(&self) -> bool {
        let dino = self.dino_rect().shrink(3.0);
        self.obstacles
            .iter()
            .any(|obstacle| dino.intersects(obstacle.rect.shrink(3.0)))
    }

    fn airborne_offset(&self) -> f32 {
        (GROUND_Y - DINO_HEIGHT - self.dino_y).max(0.0)
    }

    fn record_high_score(&mut self) -> bool {
        let score = self.score();
        if score > self.high_score {
            self.high_score = score;
            true
        } else {
            false
        }
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn jump_only_leaves_ground_from_running_grounded_state() {
        let mut state = DinoGameState::new();
        state.command(GameCommand::Jump);
        assert_eq!(state.phase(), GamePhase::Running);

        let _ = state.update(0.016);
        assert!(!state.grounded());
    }

    #[wasm_bindgen_test]
    fn duck_changes_grounded_hitbox() {
        let mut state = DinoGameState::new();
        state.command(GameCommand::Restart);
        state.command(GameCommand::DuckStart);
        let duck = state.dino_rect();
        state.command(GameCommand::DuckEnd);
        let normal = state.dino_rect();

        assert!(duck.width > normal.width);
        assert!(duck.height < normal.height);
    }

    #[wasm_bindgen_test]
    fn score_increases_while_running() {
        let mut state = DinoGameState::new();
        state.command(GameCommand::Restart);
        for _ in 0..20 {
            let _ = state.update(0.016);
        }
        assert!(state.score() > 0);
    }

    #[wasm_bindgen_test]
    fn collision_crashes_game() {
        let mut state = DinoGameState::new();
        state.phase = GamePhase::Running;
        state.obstacles.push(Obstacle {
            kind: ObstacleKind::CactusSmall,
            rect: state.dino_rect(),
        });
        let outcome = state.update(0.016);
        assert_eq!(state.phase(), GamePhase::Crashed);
        assert!(outcome.crashed);
    }

    #[wasm_bindgen_test]
    fn restart_resets_runtime_state() {
        let mut state = DinoGameState::new();
        state.phase = GamePhase::Crashed;
        state.distance = 500.0;
        state.obstacles.push(Obstacle {
            kind: ObstacleKind::CactusLarge,
            rect: Rect {
                x: 10.0,
                y: 10.0,
                width: 10.0,
                height: 10.0,
            },
        });
        state.command(GameCommand::Restart);

        assert_eq!(state.phase(), GamePhase::Running);
        assert_eq!(state.score(), 0);
        assert!(state.obstacles().is_empty());
    }

    #[wasm_bindgen_test]
    fn high_score_survives_restart() {
        let mut state = DinoGameState::new_with_high_score(12);
        state.phase = GamePhase::Crashed;
        state.distance = 800.0;
        state.command(GameCommand::Restart);

        assert_eq!(state.phase(), GamePhase::Running);
        assert_eq!(state.high_score(), 12);
    }

    #[wasm_bindgen_test]
    fn collision_updates_high_score_once() {
        let mut state = DinoGameState::new_with_high_score(1);
        state.phase = GamePhase::Running;
        state.distance = 800.0;
        state.obstacles.push(Obstacle {
            kind: ObstacleKind::CactusSmall,
            rect: state.dino_rect(),
        });

        let outcome = state.update(0.016);

        assert!(outcome.high_score_changed);
        assert_eq!(state.high_score(), state.score());
    }
}
