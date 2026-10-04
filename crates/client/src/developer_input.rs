//! Held shortcut routing independent of winit's opaque platform KeyEvent payload.
use super::*;
#[derive(Default)]
pub(super) struct F3Chord {
    held: bool,
    used: bool,
}
impl F3Chord {
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
impl ClientApp {
    pub(super) fn console_input(&mut self, input: rustcraft_scripting_rhai::ConsoleInput) {
        if let Some(tools) = self.devtools.as_mut() {
            tools.input(input);
        } else {
            self.pending_console_input = Some(input);
        }
    }
    pub(super) fn developer_shortcut(
        &mut self,
        code: KeyCode,
        state: ElementState,
        repeat: bool,
    ) -> bool {
        if code == KeyCode::F4 && state == ElementState::Pressed && !repeat {
            self.console_input(rustcraft_scripting_rhai::ConsoleInput::Close);
            use rustcraft_control::diagnostics::DebugInput;
            let input = if self.control_state.selector.open {
                DebugInput::Close
            } else {
                DebugInput::Open
            };
            let _ = self.control_state.selector_input(input);
            self.dev_focus_transition();
            return true;
        }
        if code == KeyCode::Backquote && state == ElementState::Pressed && !repeat {
            self.control_state.selector.open = false;
            self.console_input(rustcraft_scripting_rhai::ConsoleInput::Toggle);
            self.dev_focus_transition();
            return true;
        }
        if code == KeyCode::F3 {
            if self.dev_focus() || self.inventory_open {
                self.f3_chord.clear();
                return true;
            }
            if state == ElementState::Pressed && !repeat {
                self.f3_chord.held = true;
                self.f3_chord.used = false;
            }
            if state == ElementState::Released {
                if self.f3_chord.held && !self.f3_chord.used {
                    self.control_state.page = "overview".into();
                    self.debug = !self.debug;
                    self.dx_text.clear();
                }
                self.f3_chord.clear();
            }
            return true;
        }
        if self.f3_chord.held {
            let digit = match code {
                KeyCode::Digit1 => Some(1),
                KeyCode::Digit2 => Some(2),
                KeyCode::Digit3 => Some(3),
                KeyCode::Digit4 => Some(4),
                KeyCode::Digit5 => Some(5),
                KeyCode::Digit6 => Some(6),
                KeyCode::Digit7 => Some(7),
                KeyCode::Digit8 => Some(8),
                KeyCode::Digit9 => Some(9),
                _ => None,
            };
            if let Some(digit) = digit {
                if state == ElementState::Pressed && !repeat && !self.f3_chord.used {
                    self.f3_chord.used = true;
                    if let Some(page) = self
                        .control_state
                        .diagnostics
                        .registry
                        .views(rustcraft_control::diagnostics::ViewKind::Page)
                        .find(|v| v.shortcut.as_deref() == Some(&format!("F3+{digit}")))
                        .map(|v| v.name.clone())
                    {
                        self.control_state.page = page;
                        self.debug = true;
                        self.dx_text.clear();
                    }
                }
                return true;
            }
            if code == KeyCode::Escape {
                self.f3_chord.clear();
                return true;
            }
        }
        if code == KeyCode::Slash
            && state == ElementState::Pressed
            && !repeat
            && !self.dev_focus()
            && !self.inventory_open
        {
            self.f3_chord.clear();
            self.console_input(rustcraft_scripting_rhai::ConsoleInput::OpenCommand);
            self.dev_focus_transition();
            return true;
        }
        false
    }
    pub(super) fn developer_automation_key(
        &mut self,
        key: &str,
        pressed: bool,
        repeat: bool,
    ) -> Result<(), String> {
        let code = match key {
            "F3" => KeyCode::F3,
            "F4" => KeyCode::F4,
            "Backquote" => KeyCode::Backquote,
            "Slash" => KeyCode::Slash,
            "Digit1" => KeyCode::Digit1,
            "Digit2" => KeyCode::Digit2,
            "Digit3" => KeyCode::Digit3,
            "Digit4" => KeyCode::Digit4,
            "Digit5" => KeyCode::Digit5,
            "Digit6" => KeyCode::Digit6,
            "Digit7" => KeyCode::Digit7,
            "Digit8" => KeyCode::Digit8,
            "Digit9" => KeyCode::Digit9,
            _ => return Err("unsupported developer shortcut key".into()),
        };
        let state = if pressed {
            ElementState::Pressed
        } else {
            ElementState::Released
        };
        if !self.developer_shortcut(code, state, repeat) && !self.dev_focus() {
            self.controller.key(code, state);
            if self.control_state.leased {
                self.control_state.intent.select_hotbar =
                    self.controller.next_intent().select_hotbar;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_chords_consume_digits_and_reset_focus() {
        let mut a = ClientApp::new(None, None);
        a.devtools = Some(
            rustcraft_scripting_rhai::DevTools::new(
                &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts"),
                rustcraft_control::engine_registry(),
            )
            .unwrap(),
        );
        for (i, k) in [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(!a.developer_shortcut(k, ElementState::Pressed, false));
            a.controller.key(k, ElementState::Pressed);
            assert_eq!(a.controller.next_intent().select_hotbar, Some(i as u8));
            a.developer_shortcut(KeyCode::F3, ElementState::Pressed, false);
            assert!(a.developer_shortcut(k, ElementState::Pressed, false));
            let page = a.control_state.page.clone();
            a.developer_shortcut(KeyCode::F3, ElementState::Pressed, true);
            a.developer_shortcut(k, ElementState::Pressed, true);
            assert_eq!(a.control_state.page, page);
            assert_eq!(a.controller.next_intent().select_hotbar, None);
            a.developer_shortcut(KeyCode::F3, ElementState::Released, false);
            assert_eq!(a.control_state.page, page);
        }
        a.control_state.page.clear();
        a.debug = false;
        a.developer_shortcut(KeyCode::F3, ElementState::Pressed, false);
        assert!(!a.debug);
        a.developer_shortcut(KeyCode::F3, ElementState::Released, false);
        assert!(a.debug);
        a.developer_shortcut(KeyCode::F3, ElementState::Pressed, false);
        a.f3_chord.clear();
        a.developer_shortcut(KeyCode::F3, ElementState::Released, false);
        assert!(a.debug);
        a.control_state.selector.open = true;
        a.developer_shortcut(KeyCode::F3, ElementState::Pressed, false);
        assert!(!a.f3_chord.held);
    }
}
