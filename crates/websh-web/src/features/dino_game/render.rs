use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

use super::menu::{GameMenuMode, menu_hit_areas};
use super::state::{DinoGameState, GamePhase, ObstacleKind, Rect, WORLD_HEIGHT, WORLD_WIDTH};

const SPRITE_1X_SRC: &str = "assets/vendor/chromium-dino/100-offline-sprite.png";
const SPRITE_2X_SRC: &str = "assets/vendor/chromium-dino/200-offline-sprite.png";

const HORIZON_Y: f32 = 127.0;
const CLOUD_WIDTH: f32 = 46.0;
const CLOUD_HEIGHT: f32 = 14.0;
const TREX_HEIGHT: f32 = 47.0;
const TEXT_WIDTH: f32 = 10.0;
const TEXT_HEIGHT: f32 = 13.0;
const TEXT_DEST_WIDTH: f32 = 11.0;
const SCORE_DIGITS: usize = 5;
const HUD_SCORE_TOP_CSS: f32 = 8.0;
const HUD_SCORE_RIGHT_CSS: f32 = 6.0;
const BITMAP_WIDTH: f32 = 5.0;
const BITMAP_HEIGHT: f32 = 7.0;
const BITMAP_SPACING: f32 = 1.0;
const MENU_TITLE_Y: f32 = 12.0;
const MENU_STATS_Y: f32 = 68.0;
const MENU_SCORE_CENTER_X: f32 = 227.0;
const MENU_HIGH_SCORE_CENTER_X: f32 = 375.0;

#[derive(Clone, Debug)]
pub struct CanvasPalette {
    pub background: String,
    pub ink: String,
}

impl Default for CanvasPalette {
    fn default() -> Self {
        Self {
            background: "#f7f7f7".to_string(),
            ink: "#535353".to_string(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct CanvasMenu<'a> {
    pub mode: GameMenuMode,
    pub has_exit: bool,
    pub exit_label: &'a str,
}

#[derive(Clone)]
pub struct SpriteAssets {
    one_x: Option<HtmlImageElement>,
    two_x: Option<HtmlImageElement>,
}

impl SpriteAssets {
    pub fn new() -> Self {
        Self {
            one_x: load_image(SPRITE_1X_SRC),
            two_x: load_image(SPRITE_2X_SRC),
        }
    }

    fn sheet(&self, device_pixel_ratio: f64) -> Option<SpriteSheet<'_>> {
        if device_pixel_ratio > 1.0
            && let Some(image) = self.two_x.as_ref()
            && image.complete()
            && image.natural_width() > 0
        {
            return Some(SpriteSheet {
                image,
                scale: SpriteScale::TwoX,
            });
        }

        if let Some(image) = self.one_x.as_ref()
            && image.complete()
            && image.natural_width() > 0
        {
            return Some(SpriteSheet {
                image,
                scale: SpriteScale::OneX,
            });
        }

        None
    }
}

#[derive(Clone, Copy)]
enum SpriteScale {
    OneX,
    TwoX,
}

impl SpriteScale {
    const fn factor(self) -> f64 {
        match self {
            Self::OneX => 1.0,
            Self::TwoX => 2.0,
        }
    }
}

struct SpriteSheet<'a> {
    image: &'a HtmlImageElement,
    scale: SpriteScale,
}

#[derive(Clone, Copy)]
struct SpriteOrigin {
    ldpi_x: f64,
    ldpi_y: f64,
    hdpi_x: f64,
    hdpi_y: f64,
}

impl SpriteOrigin {
    const fn new(ldpi_x: f64, ldpi_y: f64, hdpi_x: f64, hdpi_y: f64) -> Self {
        Self {
            ldpi_x,
            ldpi_y,
            hdpi_x,
            hdpi_y,
        }
    }

    const fn x(self, scale: SpriteScale) -> f64 {
        match scale {
            SpriteScale::OneX => self.ldpi_x,
            SpriteScale::TwoX => self.hdpi_x,
        }
    }

    const fn y(self, scale: SpriteScale) -> f64 {
        match scale {
            SpriteScale::OneX => self.ldpi_y,
            SpriteScale::TwoX => self.hdpi_y,
        }
    }
}

const CLOUD: SpriteOrigin = SpriteOrigin::new(86.0, 2.0, 166.0, 2.0);
const CACTUS_SMALL: SpriteOrigin = SpriteOrigin::new(228.0, 2.0, 446.0, 2.0);
const CACTUS_LARGE: SpriteOrigin = SpriteOrigin::new(332.0, 2.0, 652.0, 2.0);
const HORIZON: SpriteOrigin = SpriteOrigin::new(2.0, 54.0, 2.0, 104.0);
const PTERODACTYL: SpriteOrigin = SpriteOrigin::new(134.0, 2.0, 260.0, 2.0);
const TEXT: SpriteOrigin = SpriteOrigin::new(655.0, 2.0, 1294.0, 2.0);
const TREX: SpriteOrigin = SpriteOrigin::new(848.0, 2.0, 1678.0, 2.0);

pub fn render_canvas(
    canvas: &HtmlCanvasElement,
    ctx: &CanvasRenderingContext2d,
    state: &DinoGameState,
    palette: &CanvasPalette,
    sprites: &SpriteAssets,
    menu: Option<CanvasMenu<'_>>,
    device_pixel_ratio: f64,
) {
    let css_width = f64::from(canvas.client_width().max(1));
    let css_height = f64::from(canvas.client_height().max(1));
    let backing_width = (css_width * device_pixel_ratio).round().max(1.0) as u32;
    let backing_height = (css_height * device_pixel_ratio).round().max(1.0) as u32;

    if canvas.width() != backing_width {
        canvas.set_width(backing_width);
    }
    if canvas.height() != backing_height {
        canvas.set_height(backing_height);
    }

    let _ = ctx.set_transform(device_pixel_ratio, 0.0, 0.0, device_pixel_ratio, 0.0, 0.0);
    ctx.set_fill_style_str(&palette.background);
    ctx.fill_rect(0.0, 0.0, css_width, css_height);

    let scale = (css_width / f64::from(WORLD_WIDTH)).min(css_height / f64::from(WORLD_HEIGHT));
    let offset_x = (css_width - f64::from(WORLD_WIDTH) * scale) / 2.0;
    let offset_y = (css_height - f64::from(WORLD_HEIGHT) * scale) / 2.0;

    let sheet = sprites.sheet(device_pixel_ratio);

    ctx.save();
    let _ = ctx.translate(offset_x, offset_y);
    let _ = ctx.scale(scale, scale);

    if let Some(sheet) = sheet.as_ref() {
        draw_clouds(ctx, state, sheet);
        draw_horizon(ctx, state, sheet);
        draw_obstacles(ctx, state, sheet);
        draw_trex(ctx, state, sheet);
    }

    if let Some(menu) = menu {
        draw_menu(
            ctx,
            state,
            palette,
            menu.mode,
            menu.has_exit,
            menu.exit_label,
        );
    }

    ctx.restore();

    if menu.is_none()
        && let Some(sheet) = sheet.as_ref()
    {
        draw_hud_score(ctx, state, sheet, css_width, scale);
    }
}

fn draw_clouds(ctx: &CanvasRenderingContext2d, state: &DinoGameState, sheet: &SpriteSheet<'_>) {
    let drift = state.distance() * 0.22;
    draw_cloud(ctx, sheet, 530.0 - drift.rem_euclid(760.0), 27.0);
    draw_cloud(ctx, sheet, 190.0 - (drift + 280.0).rem_euclid(820.0), 48.0);
}

fn draw_cloud(ctx: &CanvasRenderingContext2d, sheet: &SpriteSheet<'_>, x: f32, y: f32) {
    let mut draw_x = x;
    if draw_x < -CLOUD_WIDTH {
        draw_x += WORLD_WIDTH + CLOUD_WIDTH;
    }

    draw_sprite(
        ctx,
        sheet,
        CLOUD,
        Rect {
            x: 0.0,
            y: 0.0,
            width: CLOUD_WIDTH,
            height: CLOUD_HEIGHT,
        },
        Rect {
            x: draw_x,
            y,
            width: CLOUD_WIDTH,
            height: CLOUD_HEIGHT,
        },
    );
}

fn draw_horizon(ctx: &CanvasRenderingContext2d, state: &DinoGameState, sheet: &SpriteSheet<'_>) {
    let offset = -(state.distance().rem_euclid(WORLD_WIDTH));
    for index in 0..3 {
        draw_sprite(
            ctx,
            sheet,
            HORIZON,
            Rect {
                x: 0.0,
                y: 0.0,
                width: WORLD_WIDTH,
                height: 12.0,
            },
            Rect {
                x: offset + index as f32 * WORLD_WIDTH,
                y: HORIZON_Y,
                width: WORLD_WIDTH,
                height: 12.0,
            },
        );
    }
}

fn draw_obstacles(ctx: &CanvasRenderingContext2d, state: &DinoGameState, sheet: &SpriteSheet<'_>) {
    for obstacle in state.obstacles() {
        match obstacle.kind {
            ObstacleKind::CactusSmall => draw_sprite(
                ctx,
                sheet,
                CACTUS_SMALL,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 17.0,
                    height: 35.0,
                },
                obstacle.rect,
            ),
            ObstacleKind::CactusLarge => draw_sprite(
                ctx,
                sheet,
                CACTUS_LARGE,
                Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 25.0,
                    height: 50.0,
                },
                obstacle.rect,
            ),
            ObstacleKind::Flyer => {
                let frame = if ((state.distance() / 36.0).floor() as u32).is_multiple_of(2) {
                    0.0
                } else {
                    46.0
                };
                draw_sprite(
                    ctx,
                    sheet,
                    PTERODACTYL,
                    Rect {
                        x: frame,
                        y: 0.0,
                        width: 46.0,
                        height: 40.0,
                    },
                    obstacle.rect,
                );
            }
        }
    }
}

fn draw_trex(ctx: &CanvasRenderingContext2d, state: &DinoGameState, sheet: &SpriteSheet<'_>) {
    let frame = match state.phase() {
        GamePhase::Crashed => 220.0,
        GamePhase::Ready => 44.0,
        GamePhase::Running if state.dino_ducking() => {
            if ((state.distance() / 42.0).floor() as u32).is_multiple_of(2) {
                264.0
            } else {
                323.0
            }
        }
        GamePhase::Running if state.grounded() => {
            if ((state.distance() / 42.0).floor() as u32).is_multiple_of(2) {
                88.0
            } else {
                132.0
            }
        }
        GamePhase::Running => 0.0,
    };
    let source_width = if state.dino_ducking() { 59.0 } else { 44.0 };
    let visual_rect = state.dino_visual_rect();

    draw_sprite(
        ctx,
        sheet,
        TREX,
        Rect {
            x: frame,
            y: 0.0,
            width: source_width,
            height: TREX_HEIGHT,
        },
        visual_rect,
    );
}

fn draw_hud_score(
    ctx: &CanvasRenderingContext2d,
    state: &DinoGameState,
    sheet: &SpriteSheet<'_>,
    css_width: f64,
    world_scale: f64,
) {
    if world_scale <= 0.0 {
        return;
    }

    let local_right = ((css_width - f64::from(HUD_SCORE_RIGHT_CSS)) / world_scale) as f32;
    let score_x = local_right - TEXT_DEST_WIDTH * SCORE_DIGITS as f32;
    let score_y = (f64::from(HUD_SCORE_TOP_CSS) / world_scale) as f32;

    ctx.save();
    let _ = ctx.scale(world_scale, world_scale);
    draw_number(ctx, sheet, state.score(), score_x, score_y, SCORE_DIGITS);

    if state.high_score() > 0 {
        let high_score_x = score_x - (SCORE_DIGITS as f32 * 2.0) * TEXT_WIDTH;
        draw_hi_score(ctx, sheet, state.high_score(), high_score_x, score_y);
    }
    ctx.restore();
}

fn draw_hi_score(
    ctx: &CanvasRenderingContext2d,
    sheet: &SpriteSheet<'_>,
    score: u32,
    x: f32,
    y: f32,
) {
    draw_text_glyph(ctx, sheet, 10, x, y);
    draw_text_glyph(ctx, sheet, 11, x + TEXT_DEST_WIDTH, y);
    draw_number(
        ctx,
        sheet,
        score,
        x + TEXT_DEST_WIDTH * 3.0,
        y,
        SCORE_DIGITS,
    );
}

fn draw_number(
    ctx: &CanvasRenderingContext2d,
    sheet: &SpriteSheet<'_>,
    value: u32,
    x: f32,
    y: f32,
    digits: usize,
) {
    let modulus = 10_u32.pow(digits as u32);
    let text = format!("{:0width$}", value % modulus, width = digits);
    for (index, digit) in text.bytes().enumerate() {
        let digit = usize::from(digit.saturating_sub(b'0'));
        draw_text_glyph(ctx, sheet, digit, x + index as f32 * TEXT_DEST_WIDTH, y);
    }
}

fn draw_text_glyph(
    ctx: &CanvasRenderingContext2d,
    sheet: &SpriteSheet<'_>,
    glyph: usize,
    x: f32,
    y: f32,
) {
    draw_sprite(
        ctx,
        sheet,
        TEXT,
        Rect {
            x: glyph as f32 * TEXT_WIDTH,
            y: 0.0,
            width: TEXT_WIDTH,
            height: TEXT_HEIGHT,
        },
        Rect {
            x,
            y,
            width: TEXT_WIDTH,
            height: TEXT_HEIGHT,
        },
    );
}

fn draw_menu(
    ctx: &CanvasRenderingContext2d,
    state: &DinoGameState,
    palette: &CanvasPalette,
    mode: GameMenuMode,
    has_exit: bool,
    exit_label: &str,
) {
    ctx.save();
    ctx.set_global_alpha(0.88);
    ctx.set_fill_style_str(&palette.background);
    ctx.fill_rect(0.0, 0.0, f64::from(WORLD_WIDTH), f64::from(WORLD_HEIGHT));
    ctx.restore();

    draw_bitmap_text_centered(
        ctx,
        mode.title(),
        WORLD_WIDTH / 2.0,
        MENU_TITLE_Y,
        3.0,
        palette,
    );
    draw_bitmap_text_centered(
        ctx,
        &format!("SCORE {:05}", state.score() % 100_000),
        MENU_SCORE_CENTER_X,
        MENU_STATS_Y,
        2.0,
        palette,
    );
    draw_bitmap_text_centered(
        ctx,
        &format!("HI {:05}", state.high_score() % 100_000),
        MENU_HIGH_SCORE_CENTER_X,
        MENU_STATS_Y,
        2.0,
        palette,
    );

    let hit_areas = menu_hit_areas(has_exit);
    draw_action_label(ctx, mode.primary_label(), hit_areas.primary, palette);
    if let Some(exit) = hit_areas.exit {
        draw_action_label(ctx, exit_label, exit, palette);
    }
}

fn draw_action_label(
    ctx: &CanvasRenderingContext2d,
    label: &str,
    hit_area: Rect,
    palette: &CanvasPalette,
) {
    let scale = 2.0;
    let y = hit_area.y + (hit_area.height - BITMAP_HEIGHT * scale) / 2.0;
    draw_bitmap_text_centered(
        ctx,
        label,
        hit_area.x + hit_area.width / 2.0,
        y,
        scale,
        palette,
    );
}

fn draw_bitmap_text_centered(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    center_x: f32,
    y: f32,
    scale: f32,
    palette: &CanvasPalette,
) {
    let width = bitmap_text_width(text, scale);
    draw_bitmap_text(ctx, text, center_x - width / 2.0, y, scale, palette);
}

fn draw_bitmap_text(
    ctx: &CanvasRenderingContext2d,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    palette: &CanvasPalette,
) {
    ctx.set_fill_style_str(&palette.ink);
    let mut cursor_x = x;
    for byte in text.bytes() {
        if let Some(rows) = bitmap_glyph(byte.to_ascii_uppercase()) {
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..5 {
                    if bits & (1 << (4 - col)) != 0 {
                        ctx.fill_rect(
                            f64::from(cursor_x + col as f32 * scale),
                            f64::from(y + row as f32 * scale),
                            f64::from(scale),
                            f64::from(scale),
                        );
                    }
                }
            }
        }
        cursor_x += (BITMAP_WIDTH + BITMAP_SPACING) * scale;
    }
}

fn bitmap_text_width(text: &str, scale: f32) -> f32 {
    let chars = text.len();
    if chars == 0 {
        return 0.0;
    }
    (chars as f32 * BITMAP_WIDTH + (chars - 1) as f32 * BITMAP_SPACING) * scale
}

fn bitmap_glyph(byte: u8) -> Option<[u8; 7]> {
    match byte {
        b' ' => Some([0, 0, 0, 0, 0, 0, 0]),
        b'0' => Some([
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ]),
        b'1' => Some([
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        b'2' => Some([
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ]),
        b'3' => Some([
            0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        b'4' => Some([
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ]),
        b'5' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110,
        ]),
        b'6' => Some([
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ]),
        b'7' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ]),
        b'8' => Some([
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ]),
        b'9' => Some([
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100,
        ]),
        b'A' => Some([
            0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        b'B' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110,
        ]),
        b'C' => Some([
            0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110,
        ]),
        b'D' => Some([
            0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110,
        ]),
        b'E' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111,
        ]),
        b'F' => Some([
            0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        b'G' => Some([
            0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110,
        ]),
        b'H' => Some([
            0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001,
        ]),
        b'I' => Some([
            0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ]),
        b'J' => Some([
            0b00111, 0b00010, 0b00010, 0b00010, 0b10010, 0b10010, 0b01100,
        ]),
        b'K' => Some([
            0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001,
        ]),
        b'L' => Some([
            0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111,
        ]),
        b'M' => Some([
            0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001,
        ]),
        b'N' => Some([
            0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001,
        ]),
        b'O' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        b'P' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000,
        ]),
        b'Q' => Some([
            0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101,
        ]),
        b'R' => Some([
            0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001,
        ]),
        b'S' => Some([
            0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110,
        ]),
        b'T' => Some([
            0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        b'U' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110,
        ]),
        b'V' => Some([
            0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100,
        ]),
        b'W' => Some([
            0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b10101, 0b01010,
        ]),
        b'X' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001,
        ]),
        b'Y' => Some([
            0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100,
        ]),
        b'Z' => Some([
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111,
        ]),
        _ => None,
    }
}

fn draw_sprite(
    ctx: &CanvasRenderingContext2d,
    sheet: &SpriteSheet<'_>,
    origin: SpriteOrigin,
    source: Rect,
    dest: Rect,
) {
    let scale = sheet.scale.factor();
    let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
        sheet.image,
        origin.x(sheet.scale) + f64::from(source.x) * scale,
        origin.y(sheet.scale) + f64::from(source.y) * scale,
        f64::from(source.width) * scale,
        f64::from(source.height) * scale,
        f64::from(dest.x),
        f64::from(dest.y),
        f64::from(dest.width),
        f64::from(dest.height),
    );
}

fn load_image(src: &str) -> Option<HtmlImageElement> {
    let image = HtmlImageElement::new().ok()?;
    image.set_src(src);
    Some(image)
}
