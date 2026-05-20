use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leptos::{ev, prelude::*};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use super::input::{keydown_command, keyup_command};
use super::menu::{GameMenuMode, MenuAction, canvas_to_world_point, menu_action_at};
use super::render::{CanvasMenu, CanvasPalette, SpriteAssets, render_canvas};
use super::state::{DinoGameState, GameCommand, GamePhase};
use super::storage::{load_high_score, save_high_score};

stylance::import_crate_style!(css, "src/features/dino_game/dino_game.module.css");

type RafCallback = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

#[derive(Clone, Copy)]
struct GameSignals {
    score: RwSignal<u32>,
    high_score: RwSignal<u32>,
    phase: RwSignal<GamePhase>,
    menu_mode: RwSignal<Option<GameMenuMode>>,
}

#[component]
pub fn DinoGame(
    #[prop(optional)] autofocus: bool,
    #[prop(optional)] menu_request: Option<Signal<u64>>,
    #[prop(optional)] exit_label: Option<&'static str>,
    #[prop(optional)] on_exit: Option<Callback<()>>,
) -> impl IntoView {
    let root_ref = NodeRef::<leptos::html::Div>::new();
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let initial_high_score = load_high_score();
    let state = Rc::new(RefCell::new(DinoGameState::new_with_high_score(
        initial_high_score,
    )));
    let loop_state = state.clone();
    let installed = Cell::new(false);
    let score = RwSignal::new(0);
    let high_score = RwSignal::new(initial_high_score);
    let phase = RwSignal::new(GamePhase::Ready);
    let menu_mode = RwSignal::new(None::<GameMenuMode>);
    let signals = GameSignals {
        score,
        high_score,
        phase,
        menu_mode,
    };
    let exit_label = exit_label.unwrap_or("EXIT");
    let has_exit = on_exit.is_some();
    let last_menu_request = RwSignal::new(
        menu_request
            .as_ref()
            .map(Signal::get_untracked)
            .unwrap_or(0),
    );
    let sprites = Rc::new(SpriteAssets::new());
    let loop_sprites = sprites.clone();

    Effect::new(move |_| {
        if installed.get() {
            return;
        }
        let (Some(root), Some(canvas)) = (root_ref.get(), canvas_ref.get()) else {
            return;
        };
        installed.set(true);
        if autofocus {
            let _ = root.focus();
        }
        install_game_loop(
            root.into(),
            canvas,
            loop_state.clone(),
            loop_sprites.clone(),
            signals,
            has_exit,
            exit_label,
        );
    });

    Effect::new(move |_| {
        let Some(menu_request) = menu_request else {
            return;
        };
        let next_request = menu_request.get();
        if next_request == last_menu_request.get_untracked() {
            return;
        }
        last_menu_request.set(next_request);
        if next_request > 0 {
            open_menu_for_phase(phase, menu_mode);
            focus_root(root_ref);
        }
    });

    let apply_keydown = {
        let state = state.clone();
        move |command: GameCommand| {
            state.borrow_mut().command(command);
            sync_signals(&state, score, high_score, phase);
        }
    };
    let apply_keyup = {
        let state = state.clone();
        move |command: GameCommand| {
            state.borrow_mut().command(command);
            sync_signals(&state, score, high_score, phase);
        }
    };
    let apply_pointer = {
        let state = state.clone();
        move || {
            state.borrow_mut().command(GameCommand::Jump);
            sync_signals(&state, score, high_score, phase);
        }
    };
    let key_state = state.clone();
    let pointer_state = state.clone();

    view! {
        <div
            class=css::root
            node_ref=root_ref
            tabindex="0"
            role="application"
            aria-label="Dino game"
            on:keydown=move |ev: ev::KeyboardEvent| {
                let key = ev.key();
                if key == "Escape" {
                    ev.prevent_default();
                    if menu_mode.get_untracked() == Some(GameMenuMode::Paused) {
                        resume_menu(menu_mode, root_ref);
                    } else if menu_mode.get_untracked().is_none() {
                        open_menu_for_phase(phase, menu_mode);
                    }
                    return;
                }
                if menu_mode.get_untracked().is_some() {
                    if key == "Enter" || key == " " {
                        ev.prevent_default();
                        activate_menu_primary(&key_state, score, high_score, phase, menu_mode, root_ref);
                    } else if keydown_command(&key).is_some() {
                        ev.prevent_default();
                    }
                    return;
                }
                if let Some(command) = keydown_command(&key) {
                    ev.prevent_default();
                    apply_keydown(command);
                }
            }
            on:keyup=move |ev: ev::KeyboardEvent| {
                if menu_mode.get_untracked().is_some() {
                    if keyup_command(&ev.key()).is_some() || matches!(ev.key().as_str(), " " | "Enter") {
                        ev.prevent_default();
                    }
                    return;
                }
                if let Some(command) = keyup_command(&ev.key()) {
                    ev.prevent_default();
                    apply_keyup(command);
                }
            }
        >
            <canvas
                class=css::canvas
                node_ref=canvas_ref
                aria-label="Dino game canvas"
                on:pointerdown=move |ev: ev::PointerEvent| {
                    ev.prevent_default();
                    focus_root(root_ref);
                    if menu_mode.get_untracked().is_some() {
                        let Some(canvas) = canvas_ref.get() else {
                            return;
                        };
                        let bounds = canvas.get_bounding_client_rect();
                        let css_x = f64::from(ev.client_x()) - bounds.left();
                        let css_y = f64::from(ev.client_y()) - bounds.top();
                        let Some((world_x, world_y)) = canvas_to_world_point(
                            css_x,
                            css_y,
                            bounds.width(),
                            bounds.height(),
                        ) else {
                            return;
                        };
                        match menu_action_at(world_x, world_y, has_exit) {
                            Some(MenuAction::Primary) => {
                                activate_menu_primary(
                                    &pointer_state,
                                    score,
                                    high_score,
                                    phase,
                                    menu_mode,
                                    root_ref,
                                );
                            }
                            Some(MenuAction::Exit) => {
                                if let Some(on_exit) = on_exit {
                                    on_exit.run(());
                                }
                            }
                            None => {}
                        }
                        return;
                    }
                    apply_pointer();
                }
            ></canvas>
        </div>
    }
}

fn install_game_loop(
    root: web_sys::HtmlElement,
    canvas: HtmlCanvasElement,
    state: Rc<RefCell<DinoGameState>>,
    sprites: Rc<SpriteAssets>,
    signals: GameSignals,
    has_exit: bool,
    exit_label: &'static str,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(ctx) = canvas
        .get_context("2d")
        .ok()
        .flatten()
        .and_then(|ctx| ctx.dyn_into::<CanvasRenderingContext2d>().ok())
    else {
        return;
    };

    let palette = Rc::new(palette_from_root(&root));
    let callback: RafCallback = Rc::new(RefCell::new(None));
    let frame_id = Rc::new(Cell::new(None::<i32>));
    let last_time = Rc::new(Cell::new(None::<f64>));

    let callback_ref = callback.clone();
    let frame_id_ref = frame_id.clone();
    let last_time_ref = last_time.clone();
    let window_ref = window.clone();
    let canvas_ref = canvas.clone();
    let ctx_ref = ctx.clone();
    let state_ref = state.clone();
    let palette_ref = palette.clone();
    let sprites_ref = sprites.clone();

    *callback.borrow_mut() = Some(Closure::wrap(Box::new(move |timestamp: f64| {
        let dt = last_time_ref
            .replace(Some(timestamp))
            .map(|previous| ((timestamp - previous) / 1000.0) as f32)
            .unwrap_or(0.0);
        let (next_score, next_high_score, next_phase, outcome) = {
            let mut state = state_ref.borrow_mut();
            let outcome = if signals.menu_mode.get_untracked().is_some() {
                Default::default()
            } else {
                state.update(dt)
            };
            let menu_mode = signals
                .menu_mode
                .get_untracked()
                .or_else(|| outcome.crashed.then_some(GameMenuMode::GameOver));
            let menu = menu_mode.map(|mode| CanvasMenu {
                mode,
                has_exit,
                exit_label,
            });
            render_canvas(
                &canvas_ref,
                &ctx_ref,
                &state,
                &palette_ref,
                &sprites_ref,
                menu,
                window_ref.device_pixel_ratio(),
            );
            (state.score(), state.high_score(), state.phase(), outcome)
        };
        if signals.score.get_untracked() != next_score {
            signals.score.set(next_score);
        }
        if signals.high_score.get_untracked() != next_high_score {
            signals.high_score.set(next_high_score);
        }
        if signals.phase.get_untracked() != next_phase {
            signals.phase.set(next_phase);
        }
        if outcome.high_score_changed {
            save_high_score(next_high_score);
        }
        if outcome.crashed {
            signals.menu_mode.set(Some(GameMenuMode::GameOver));
        }
        if let Some(callback) = callback_ref.borrow().as_ref()
            && let Ok(next_id) =
                window_ref.request_animation_frame(callback.as_ref().unchecked_ref())
        {
            frame_id_ref.set(Some(next_id));
        }
    }) as Box<dyn FnMut(f64)>));

    if let Some(callback) = callback.borrow().as_ref()
        && let Ok(id) = window.request_animation_frame(callback.as_ref().unchecked_ref())
    {
        frame_id.set(Some(id));
    }

    let cleanup = AnimationLoopCleanup {
        window,
        frame_id,
        callback,
    };
    on_cleanup(move || cleanup.cancel());
}

fn palette_from_root(root: &web_sys::HtmlElement) -> CanvasPalette {
    let Some(style) =
        web_sys::window().and_then(|window| window.get_computed_style(root).ok().flatten())
    else {
        return CanvasPalette::default();
    };

    CanvasPalette {
        background: css_prop(&style, "--dino-game-bg", "#f7f7f7"),
        ink: css_prop(&style, "--dino-game-ink", "#535353"),
    }
}

fn css_prop(style: &web_sys::CssStyleDeclaration, name: &str, fallback: &str) -> String {
    style
        .get_property_value(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

struct AnimationLoopCleanup {
    window: web_sys::Window,
    frame_id: Rc<Cell<Option<i32>>>,
    callback: RafCallback,
}

impl AnimationLoopCleanup {
    fn cancel(&self) {
        if let Some(id) = self.frame_id.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
        self.callback.borrow_mut().take();
    }
}

// SAFETY: the web app runs on wasm32's single JS thread. The cleanup is only
// moved through Leptos's generic cleanup bound and is not sent across threads.
unsafe impl Send for AnimationLoopCleanup {}
unsafe impl Sync for AnimationLoopCleanup {}

fn focus_root(root_ref: NodeRef<leptos::html::Div>) {
    if let Some(root) = root_ref.get() {
        let _ = root.focus();
    }
}

fn sync_signals(
    state: &Rc<RefCell<DinoGameState>>,
    score: RwSignal<u32>,
    high_score: RwSignal<u32>,
    phase: RwSignal<GamePhase>,
) {
    let state = state.borrow();
    let next_score = state.score();
    let next_high_score = state.high_score();
    let next_phase = state.phase();
    if score.get_untracked() != next_score {
        score.set(next_score);
    }
    if high_score.get_untracked() != next_high_score {
        high_score.set(next_high_score);
    }
    if phase.get_untracked() != next_phase {
        phase.set(next_phase);
    }
}

fn open_menu_for_phase(phase: RwSignal<GamePhase>, menu_mode: RwSignal<Option<GameMenuMode>>) {
    let mode = if phase.get_untracked() == GamePhase::Crashed {
        GameMenuMode::GameOver
    } else {
        GameMenuMode::Paused
    };
    menu_mode.set(Some(mode));
}

fn resume_menu(menu_mode: RwSignal<Option<GameMenuMode>>, root_ref: NodeRef<leptos::html::Div>) {
    if menu_mode.get_untracked() == Some(GameMenuMode::Paused) {
        menu_mode.set(None);
        focus_root(root_ref);
    }
}

fn activate_menu_primary(
    state: &Rc<RefCell<DinoGameState>>,
    score: RwSignal<u32>,
    high_score: RwSignal<u32>,
    phase: RwSignal<GamePhase>,
    menu_mode: RwSignal<Option<GameMenuMode>>,
    root_ref: NodeRef<leptos::html::Div>,
) {
    if menu_mode.get_untracked() == Some(GameMenuMode::GameOver) {
        state.borrow_mut().command(GameCommand::Restart);
        sync_signals(state, score, high_score, phase);
        menu_mode.set(None);
        focus_root(root_ref);
    } else {
        resume_menu(menu_mode, root_ref);
    }
}
