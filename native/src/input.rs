//! Logical keyboard shortcuts and device-independent canvas navigation.

use eframe::egui::{Event, InputState, Key, KeyboardShortcut, Modifiers, MouseWheelUnit, Vec2};

const MAX_SCROLL_STEPS: f32 = 1.0;
const SCROLL_ZOOM_PER_STEP: f32 = 1.05;

pub struct CanvasNavigation {
    pub pan: Vec2,
    pub zoom: f32,
}

pub fn scroll_zoom_factor(steps: f32) -> f32 {
    SCROLL_ZOOM_PER_STEP.powf(steps.clamp(-MAX_SCROLL_STEPS, MAX_SCROLL_STEPS))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScrollMode {
    #[default]
    Auto,
    Pan,
    Zoom,
}

impl ScrollMode {
    pub fn translation_key(self) -> &'static str {
        match self {
            Self::Auto => "navigation.auto",
            Self::Pan => "navigation.pan",
            Self::Zoom => "navigation.zoom",
        }
    }
}

pub fn canvas_navigation(input: &InputState, mode: ScrollMode) -> CanvasNavigation {
    let mut pan = Vec2::ZERO;
    let mut steps = 0.0;
    for event in &input.raw.events {
        if let Event::MouseWheel {
            unit,
            delta,
            modifiers,
            ..
        } = event
        {
            // egui already accounts for Ctrl/Command scrolling in zoom_delta().
            if modifiers.ctrl || modifiers.command {
                continue;
            }
            if modifiers.alt {
                steps += if *unit == MouseWheelUnit::Point {
                    delta.y / 50.0
                } else {
                    delta.y
                };
                continue;
            }
            if modifiers.shift && mode != ScrollMode::Zoom {
                pan += Vec2::new(if delta.x == 0.0 { delta.y } else { delta.x }, 0.0)
                    * if *unit == MouseWheelUnit::Line {
                        24.0
                    } else {
                        1.0
                    };
                continue;
            }
            match (mode, unit) {
                (ScrollMode::Pan, _) | (ScrollMode::Auto, MouseWheelUnit::Point) => {
                    pan += *delta
                        * if *unit == MouseWheelUnit::Line {
                            24.0
                        } else {
                            1.0
                        };
                }
                (_, MouseWheelUnit::Point) => {
                    pan.x += delta.x;
                    steps += delta.y / 50.0;
                }
                _ => {
                    pan.x += delta.x * 24.0;
                    steps += delta.y;
                }
            }
        }
    }
    CanvasNavigation {
        pan,
        zoom: scroll_zoom_factor(steps) * input.zoom_delta(),
    }
}

pub struct ViewActions {
    pub fit: bool,
    pub zoom: f32,
}

pub fn view_actions(input: &mut InputState) -> ViewActions {
    let fit = input.key_pressed(Key::Home)
        || input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0));
    let zoom_in = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Plus))
        || input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Equals));
    let zoom_out = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Minus));
    ViewActions {
        fit,
        zoom: if zoom_in {
            1.1
        } else if zoom_out {
            1.0 / 1.1
        } else {
            1.0
        },
    }
}

#[derive(Default)]
pub struct KeyboardActions {
    pub new: bool,
    pub open: bool,
    pub save: bool,
    pub save_as: bool,
    pub undo: bool,
    pub redo: bool,
    pub select_all: bool,
    pub duplicate: bool,
    pub delete: bool,
    pub escape: bool,
    pub tool_key: Option<Key>,
    pub nudge: Vec2,
}

impl KeyboardActions {
    pub fn read(input: &mut InputState, editing_text: bool) -> Self {
        // Consume more specific shortcuts first: egui permits extra Shift modifiers.
        let save_as = input.consume_shortcut(&KeyboardShortcut::new(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::S,
        ));
        let save = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S));
        let new = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::N));
        let open = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::O));
        let mut actions = Self {
            new,
            open,
            save,
            save_as,
            ..Self::default()
        };
        // Leave text editing, including undo/select-all and IME composition, to egui.
        if editing_text {
            return actions;
        }
        actions.redo = input.consume_shortcut(&KeyboardShortcut::new(
            Modifiers::COMMAND | Modifiers::SHIFT,
            Key::Z,
        )) || input
            .consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Y));
        actions.undo = input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::Z));
        actions.select_all =
            input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::A));
        actions.duplicate =
            input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::D));
        actions.escape = input.key_pressed(Key::Escape);
        if !input.modifiers.ctrl && !input.modifiers.command && !input.modifiers.alt {
            actions.delete = input.key_pressed(Key::Delete) || input.key_pressed(Key::Backspace);
            // These are logical letters, not physical US key positions (AZERTY/QWERTY).
            actions.tool_key = [
                Key::V,
                Key::M,
                Key::L,
                Key::R,
                Key::T,
                Key::P,
                Key::F,
                Key::O,
                Key::A,
                Key::B,
                Key::N,
                Key::Y,
            ]
            .into_iter()
            .find(|key| input.key_pressed(*key));
            let step = if input.modifiers.shift { 10.0 } else { 1.0 };
            actions.nudge = Vec2::new(
                if input.key_pressed(Key::ArrowLeft) {
                    -step
                } else if input.key_pressed(Key::ArrowRight) {
                    step
                } else {
                    0.0
                },
                if input.key_pressed(Key::ArrowUp) {
                    -step
                } else if input.key_pressed(Key::ArrowDown) {
                    step
                } else {
                    0.0
                },
            );
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{
        Context, Event, Key, Modifiers, MouseWheelUnit, RawInput, TouchPhase, Vec2,
    };

    use super::{KeyboardActions, canvas_navigation, scroll_zoom_factor};

    fn key_event(key: Key, physical_key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: Some(physical_key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn actions(
        key: Key,
        physical_key: Key,
        modifiers: Modifiers,
        editing_text: bool,
    ) -> KeyboardActions {
        let context = Context::default();
        let mut result = KeyboardActions::default();
        let _ = context.run_ui(
            RawInput {
                modifiers,
                events: vec![key_event(key, physical_key, modifiers)],
                ..RawInput::default()
            },
            |ui| {
                result = ui
                    .ctx()
                    .input_mut(|input| KeyboardActions::read(input, editing_text));
            },
        );
        result
    }

    #[test]
    fn shortcuts_use_logical_letters_for_azerty_and_qwerty() {
        for physical_key in [Key::A, Key::Q] {
            assert_eq!(
                actions(Key::A, physical_key, Modifiers::NONE, false).tool_key,
                Some(Key::A)
            );
            assert!(actions(Key::A, physical_key, Modifiers::COMMAND, false).select_all);
        }
        for physical_key in [Key::Z, Key::W] {
            assert!(actions(Key::Z, physical_key, Modifiers::COMMAND, false).undo);
        }
    }

    #[test]
    fn save_as_and_redo_do_not_trigger_less_specific_commands() {
        let modifiers = Modifiers::COMMAND | Modifiers::SHIFT;
        let save = actions(Key::S, Key::S, modifiers, false);
        assert!(save.save_as);
        assert!(!save.save);
        let undo = actions(Key::Z, Key::Z, modifiers, false);
        assert!(undo.redo);
        assert!(!undo.undo);
    }

    #[test]
    fn text_fields_keep_their_shortcuts_and_tools_ignore_modifiers() {
        assert!(!actions(Key::Z, Key::Z, Modifiers::COMMAND, true).undo);
        assert!(!actions(Key::A, Key::A, Modifiers::COMMAND, true).select_all);
        assert_eq!(
            actions(Key::M, Key::M, Modifiers::NONE, true).tool_key,
            None
        );
        assert_eq!(
            actions(Key::M, Key::M, Modifiers::ALT, false).tool_key,
            None
        );
        assert_eq!(
            actions(Key::M, Key::M, Modifiers::COMMAND, false).tool_key,
            None
        );
        assert!(!actions(Key::Backspace, Key::Backspace, Modifiers::NONE, true).delete);
        assert!(actions(Key::O, Key::O, Modifiers::COMMAND, false).open);
    }

    #[test]
    fn trackpad_pans_wheel_zooms_and_pinch_is_not_double_counted() {
        let context = Context::default();
        let _ = context.run_ui(
            RawInput {
                events: vec![
                    Event::MouseWheel {
                        unit: MouseWheelUnit::Point,
                        delta: Vec2::new(12.0, -8.0),
                        modifiers: Modifiers::NONE,
                        phase: TouchPhase::Move,
                    },
                    Event::Zoom(1.5),
                ],
                ..RawInput::default()
            },
            |ui| {
                let navigation = ui
                    .ctx()
                    .input(|input| canvas_navigation(input, super::ScrollMode::Auto));
                assert_eq!(navigation.pan, Vec2::new(12.0, -8.0));
                assert!((navigation.zoom - 1.5).abs() < 0.0001);
            },
        );
        let context = Context::default();
        let _ = context.run_ui(
            RawInput {
                events: vec![Event::MouseWheel {
                    unit: MouseWheelUnit::Line,
                    delta: Vec2::new(0.0, 1.0),
                    modifiers: Modifiers::NONE,
                    phase: TouchPhase::Move,
                }],
                ..RawInput::default()
            },
            |ui| {
                let navigation = ui
                    .ctx()
                    .input(|input| canvas_navigation(input, super::ScrollMode::Auto));
                assert_eq!(navigation.pan, Vec2::ZERO);
                assert_eq!(navigation.zoom, scroll_zoom_factor(1.0));
            },
        );
    }

    #[test]
    fn ctrl_and_command_view_shortcuts_match_platform_conventions() {
        for modifiers in [
            Modifiers {
                ctrl: true,
                command: true,
                ..Modifiers::NONE
            },
            Modifiers {
                mac_cmd: true,
                command: true,
                ..Modifiers::NONE
            },
        ] {
            assert!(actions(Key::S, Key::S, modifiers, false).save);
            for key in [Key::Plus, Key::Equals, Key::Minus, Key::Num0] {
                let context = Context::default();
                let _ = context.run_ui(
                    RawInput {
                        modifiers,
                        events: vec![key_event(key, key, modifiers)],
                        ..RawInput::default()
                    },
                    |ui| {
                        let view = ui.ctx().input_mut(super::view_actions);
                        if key == Key::Num0 {
                            assert!(view.fit);
                        } else if key == Key::Minus {
                            assert!(view.zoom < 1.0);
                        } else {
                            assert!(view.zoom > 1.0);
                        }
                    },
                );
            }
        }
    }

    #[test]
    fn scrolling_modes_and_modifiers_cover_precise_mice_and_trackpads() {
        for (mode, modifiers, unit, delta, expected_pan, should_zoom) in [
            (
                super::ScrollMode::Zoom,
                Modifiers::NONE,
                MouseWheelUnit::Point,
                Vec2::new(0.0, 50.0),
                Vec2::ZERO,
                true,
            ),
            (
                super::ScrollMode::Pan,
                Modifiers::NONE,
                MouseWheelUnit::Line,
                Vec2::new(0.0, 1.0),
                Vec2::new(0.0, 24.0),
                false,
            ),
            (
                super::ScrollMode::Auto,
                Modifiers::SHIFT,
                MouseWheelUnit::Line,
                Vec2::new(0.0, 1.0),
                Vec2::new(24.0, 0.0),
                false,
            ),
            (
                super::ScrollMode::Auto,
                Modifiers::ALT,
                MouseWheelUnit::Point,
                Vec2::new(0.0, 50.0),
                Vec2::ZERO,
                true,
            ),
        ] {
            let context = Context::default();
            let _ = context.run_ui(
                RawInput {
                    modifiers,
                    events: vec![Event::MouseWheel {
                        unit,
                        delta,
                        modifiers,
                        phase: TouchPhase::Move,
                    }],
                    ..RawInput::default()
                },
                |ui| {
                    let navigation = ui.ctx().input(|input| canvas_navigation(input, mode));
                    assert_eq!(navigation.pan, expected_pan);
                    assert_eq!(navigation.zoom > 1.0, should_zoom);
                },
            );
        }
    }

    #[test]
    fn ctrl_scroll_uses_egui_zoom_once_without_panning() {
        let modifiers = Modifiers {
            ctrl: true,
            command: true,
            ..Modifiers::NONE
        };
        let context = Context::default();
        let _ = context.run_ui(
            RawInput {
                modifiers,
                events: vec![Event::MouseWheel {
                    unit: MouseWheelUnit::Point,
                    delta: Vec2::new(0.0, 50.0),
                    modifiers,
                    phase: TouchPhase::Move,
                }],
                ..RawInput::default()
            },
            |ui| {
                ui.ctx().input(|input| {
                    let navigation = canvas_navigation(input, super::ScrollMode::Auto);
                    assert_eq!(navigation.pan, Vec2::ZERO);
                    assert_eq!(navigation.zoom, input.zoom_delta());
                    assert!(navigation.zoom > 1.0);
                });
            },
        );
    }

    #[test]
    fn wheel_zoom_is_gentle_and_bounded() {
        assert_eq!(scroll_zoom_factor(0.0), 1.0);
        assert!((scroll_zoom_factor(1.0) - 1.05).abs() < f32::EPSILON);
        assert!((scroll_zoom_factor(-1.0) - (1.0 / 1.05)).abs() < f32::EPSILON);
        assert_eq!(scroll_zoom_factor(5000.0), scroll_zoom_factor(1.0));
    }
}
