use super::state::{Rect, WORLD_HEIGHT, WORLD_WIDTH};

const ACTION_Y: f32 = 106.0;
const ACTION_WIDTH: f32 = 96.0;
const ACTION_HEIGHT: f32 = 18.0;
const ACTION_GAP: f32 = 4.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GameMenuMode {
    Paused,
    GameOver,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MenuAction {
    Primary,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MenuHitAreas {
    pub primary: Rect,
    pub exit: Option<Rect>,
}

impl GameMenuMode {
    pub(super) const fn title(self) -> &'static str {
        match self {
            Self::Paused => "PAUSED",
            Self::GameOver => "GAME OVER",
        }
    }

    pub(super) const fn primary_label(self) -> &'static str {
        match self {
            Self::Paused => "RESUME",
            Self::GameOver => "RESTART",
        }
    }
}

pub(super) fn menu_hit_areas(has_exit: bool) -> MenuHitAreas {
    if has_exit {
        let group_width = ACTION_WIDTH * 2.0 + ACTION_GAP;
        let primary_x = (WORLD_WIDTH - group_width) / 2.0;
        return MenuHitAreas {
            primary: action_rect(primary_x),
            exit: Some(action_rect(primary_x + ACTION_WIDTH + ACTION_GAP)),
        };
    }

    MenuHitAreas {
        primary: action_rect((WORLD_WIDTH - ACTION_WIDTH) / 2.0),
        exit: None,
    }
}

pub(super) fn menu_action_at(x: f32, y: f32, has_exit: bool) -> Option<MenuAction> {
    let areas = menu_hit_areas(has_exit);
    if rect_contains(areas.primary, x, y) {
        return Some(MenuAction::Primary);
    }
    if let Some(exit) = areas.exit
        && rect_contains(exit, x, y)
    {
        return Some(MenuAction::Exit);
    }
    None
}

pub(super) fn canvas_to_world_point(
    css_x: f64,
    css_y: f64,
    css_width: f64,
    css_height: f64,
) -> Option<(f32, f32)> {
    if css_width <= 0.0 || css_height <= 0.0 {
        return None;
    }

    let scale = (css_width / f64::from(WORLD_WIDTH)).min(css_height / f64::from(WORLD_HEIGHT));
    if scale <= 0.0 {
        return None;
    }

    let offset_x = (css_width - f64::from(WORLD_WIDTH) * scale) / 2.0;
    let offset_y = (css_height - f64::from(WORLD_HEIGHT) * scale) / 2.0;
    let world_x = ((css_x - offset_x) / scale) as f32;
    let world_y = ((css_y - offset_y) / scale) as f32;

    (0.0..=WORLD_WIDTH)
        .contains(&world_x)
        .then_some(())
        .and_then(|()| {
            (0.0..=WORLD_HEIGHT)
                .contains(&world_y)
                .then_some((world_x, world_y))
        })
}

fn action_rect(x: f32) -> Rect {
    Rect {
        x,
        y: ACTION_Y,
        width: ACTION_WIDTH,
        height: ACTION_HEIGHT,
    }
}

fn rect_contains(rect: Rect, x: f32, y: f32) -> bool {
    x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn menu_hitbox_maps_primary_and_exit_actions() {
        let areas = menu_hit_areas(true);

        assert_eq!(
            menu_action_at(
                areas.primary.x + areas.primary.width / 2.0,
                areas.primary.y + areas.primary.height / 2.0,
                true,
            ),
            Some(MenuAction::Primary)
        );

        let exit = areas.exit.expect("exit area");
        assert_eq!(
            menu_action_at(exit.x + exit.width / 2.0, exit.y + exit.height / 2.0, true),
            Some(MenuAction::Exit)
        );
    }

    #[wasm_bindgen_test]
    fn exit_hitbox_is_absent_without_exit_callback() {
        let areas = menu_hit_areas(true);
        let exit = areas.exit.expect("exit area");

        assert_eq!(
            menu_action_at(exit.x + exit.width / 2.0, exit.y + exit.height / 2.0, false),
            None
        );
    }

    #[wasm_bindgen_test]
    fn canvas_coordinates_convert_to_logical_world() {
        assert_eq!(
            canvas_to_world_point(300.0, 75.0, 600.0, 150.0),
            Some((300.0, 75.0))
        );

        assert_eq!(
            canvas_to_world_point(400.0, 200.0, 800.0, 400.0),
            Some((300.0, 75.0))
        );
    }
}
