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
    pub(super) fn input_denied(&mut self, error: &str) {
        self.dx_text = error.to_owned();
        if let Some(tools) = self.devtools.as_mut() {
            tools.print(error);
        }
    }
    fn input_granted(&mut self, capability: &str) -> bool {
        match self.session.context.require(capability) {
            Ok(()) => true,
            Err(error) => {
                self.input_denied(&error);
                false
            }
        }
    }
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
        if code == KeyCode::F4 {
            if state != ElementState::Pressed || repeat || !self.input_granted("debug.configure") {
                return true;
            }
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
        if code == KeyCode::Backquote {
            if state != ElementState::Pressed || repeat || !self.input_granted("script.load") {
                return true;
            }
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
                if self.f3_chord.held
                    && !self.f3_chord.used
                    && self.input_granted("debug.configure")
                {
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
                    if self.input_granted("debug.configure")
                        && let Some(page) = self
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
            if !self.input_granted("debug.inspect") {
                return true;
            }
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
                self.leased_hotbar = self.controller.next_intent().game.select_hotbar;
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
            assert_eq!(a.controller.next_intent().game.select_hotbar, Some(i as u8));
            a.developer_shortcut(KeyCode::F3, ElementState::Pressed, false);
            assert!(a.developer_shortcut(k, ElementState::Pressed, false));
            let page = a.control_state.page.clone();
            a.developer_shortcut(KeyCode::F3, ElementState::Pressed, true);
            a.developer_shortcut(k, ElementState::Pressed, true);
            assert_eq!(a.control_state.page, page);
            assert_eq!(a.controller.next_intent().game.select_hotbar, None);
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

/// Safe payload copied at the winit boundary. KeyEvent itself has private platform fields;
/// production and deterministic tests enter this same complete client routing function.
pub(super) struct ClientKeyEvent<'a> {
    pub physical_key: PhysicalKey,
    pub state: ElementState,
    pub repeat: bool,
    pub text: Option<&'a str>,
}
impl<'a> From<&'a winit::event::KeyEvent> for ClientKeyEvent<'a> {
    fn from(event: &'a winit::event::KeyEvent) -> Self {
        Self {
            physical_key: event.physical_key,
            state: event.state,
            repeat: event.repeat,
            text: event.text.as_deref(),
        }
    }
}
pub(super) enum ClientInput<'a> {
    Key(ClientKeyEvent<'a>),
    Focus(bool),
}
#[derive(PartialEq)]
pub(super) enum InputEffect {
    Continue,
    Exit,
}
impl ClientApp {
    pub(super) fn route_window_input(&mut self, event: &WindowEvent) -> Option<InputEffect> {
        let input = match event {
            WindowEvent::KeyboardInput { event, .. } => ClientInput::Key(event.into()),
            WindowEvent::Focused(focused) => ClientInput::Focus(*focused),
            _ => return None,
        };
        Some(self.route_client_input(input))
    }
    pub(super) fn route_client_input(&mut self, event: ClientInput<'_>) -> InputEffect {
        let key = match event {
            ClientInput::Focus(focused) => {
                self.rebase_presentation();
                if !focused {
                    self.f3_chord.clear();
                    if let Some(window) = &self.window {
                        window.set_ime_allowed(false);
                        self.controller.release(window);
                    }
                    if let Some(tools) = self.devtools.as_mut() {
                        tools.input(rustcraft_scripting_rhai::ConsoleInput::Preedit(
                            String::new(),
                        ));
                    }
                    self.controller = LocalHumanController::default();
                    self.leased_hotbar = None;
                } else {
                    self.last_frame = Instant::now();
                    self.clock = Default::default();
                    if let Some(window) = &self.window {
                        window.set_ime_allowed(
                            self.devtools.as_ref().is_some_and(|d| d.console_open),
                        );
                    }
                }
                return InputEffect::Continue;
            }
            ClientInput::Key(key) => key,
        };
        if self.dev_key(&key) {
            return InputEffect::Continue;
        }
        let PhysicalKey::Code(code) = key.physical_key else {
            return InputEffect::Continue;
        };
        if code == KeyCode::KeyE && key.state == ElementState::Pressed && !key.repeat {
            self.inventory_open = !self.inventory_open;
            if self.inventory_open {
                self.f3_chord.clear();
            }
            if let Some(window) = &self.window {
                if self.inventory_open {
                    self.controller.release(window);
                } else {
                    self.controller.capture(window);
                }
            }
        } else if code == KeyCode::Escape && key.state == ElementState::Pressed {
            if self.controller.captured {
                if let Some(window) = &self.window {
                    self.controller.release(window);
                }
                self.controller.captured = false;
            } else {
                return InputEffect::Exit;
            }
        } else if code == KeyCode::KeyC
            && key.state == ElementState::Pressed
            && !key.repeat
            && self.inventory_open
        {
            if let Some(sim) = self.simulation.as_mut() {
                let _ = sim.craft_first_available("log_to_planks");
            }
        } else {
            self.controller.key(code, key.state);
        }
        InputEffect::Continue
    }
}

#[cfg(test)]
mod window_route_tests {
    use super::*;
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    fn key(a: &mut ClientApp, code: KeyCode, state: ElementState, repeat: bool) {
        assert!(
            a.route_client_input(ClientInput::Key(ClientKeyEvent {
                physical_key: PhysicalKey::Code(code),
                state,
                repeat,
                text: None
            })) == InputEffect::Continue
        );
    }
    fn tap(a: &mut ClientApp, code: KeyCode) {
        key(a, code, ElementState::Pressed, false);
        key(a, code, ElementState::Released, false);
    }
    #[test]
    fn normal_window_event_routing_granted_and_denied() {
        for flags in [
            vec![],
            vec!["--survival"],
            vec!["--player"],
            vec!["--devtools"],
        ] {
            let mut a = ClientApp::new(None, None);
            a.session = session::LocalSession::from_args(
                &flags.into_iter().map(str::to_owned).collect::<Vec<_>>(),
            );
            let granted = a.session.context.require("debug.configure").is_ok();
            // No interpreter object exists: event routing and gameplay are not chosen by it.
            assert!(a.devtools.is_none());
            a.debug = false;
            for (slot, code) in DIGITS.into_iter().enumerate() {
                tap(&mut a, code);
                assert_eq!(
                    a.controller.next_intent().game.select_hotbar,
                    Some(slot as u8)
                );
                let old_page = a.control_state.page.clone();
                key(&mut a, KeyCode::F3, ElementState::Pressed, false);
                key(&mut a, code, ElementState::Pressed, false);
                key(&mut a, KeyCode::F3, ElementState::Pressed, true);
                key(&mut a, code, ElementState::Pressed, true);
                key(&mut a, code, ElementState::Released, false);
                key(&mut a, KeyCode::F3, ElementState::Released, false);
                assert_eq!(a.controller.next_intent().game.select_hotbar, None);
                if granted {
                    let expected = a
                        .control_state
                        .diagnostics
                        .registry
                        .views(rustcraft_control::diagnostics::ViewKind::Page)
                        .find(|v| v.shortcut.as_deref() == Some(&format!("F3+{}", slot + 1)))
                        .unwrap()
                        .name
                        .clone();
                    assert_eq!(a.control_state.page, expected);
                } else {
                    assert_eq!(a.control_state.page, old_page);
                    assert!(!a.debug);
                }
            }
            a.debug = false;
            key(&mut a, KeyCode::F3, ElementState::Pressed, false);
            assert!(!a.debug);
            key(&mut a, KeyCode::F3, ElementState::Pressed, true);
            assert!(!a.debug);
            key(&mut a, KeyCode::F3, ElementState::Released, false);
            assert_eq!(a.debug, granted);
            tap(&mut a, KeyCode::F4);
            assert_eq!(a.control_state.selector.open, granted);
            key(&mut a, KeyCode::F4, ElementState::Pressed, true);
            assert_eq!(a.control_state.selector.open, granted);
            tap(&mut a, KeyCode::F4);
            assert!(!a.control_state.selector.open);
            a.pending_console_input = None;
            tap(&mut a, KeyCode::Backquote);
            assert_eq!(
                a.pending_console_input.is_some(),
                a.session.context.require("script.load").is_ok()
            );
        }
    }
    #[test]
    fn real_focus_window_event_clears_chord_movement_and_selection() {
        let mut a = ClientApp::new(None, None);
        key(&mut a, KeyCode::KeyW, ElementState::Pressed, false);
        key(&mut a, KeyCode::F3, ElementState::Pressed, false);
        a.controller.break_held = true;
        a.controller.place_pressed = true;
        a.controller.look = Vec3::new(2., 3., 0.);
        assert!(a.route_window_input(&WindowEvent::Focused(false)).is_some());
        assert_eq!(a.controller.next_intent(), AgentIntent::default());
        a.route_window_input(&WindowEvent::Focused(true));
        tap(&mut a, KeyCode::Digit4);
        assert_eq!(a.controller.next_intent().game.select_hotbar, Some(3));
        a.debug = false;
        key(&mut a, KeyCode::F3, ElementState::Released, false);
        assert!(!a.debug);
        assert!(
            a.route_window_input(&WindowEvent::RedrawRequested)
                .is_none()
        );
    }
    #[test]
    fn inventory_and_escape_transitions_use_the_same_router() {
        let mut a = ClientApp::new(None, None);
        key(&mut a, KeyCode::F3, ElementState::Pressed, false);
        tap(&mut a, KeyCode::KeyE);
        assert!(a.inventory_open);
        key(&mut a, KeyCode::KeyE, ElementState::Pressed, true);
        assert!(a.inventory_open);
        key(&mut a, KeyCode::F3, ElementState::Released, false);
        assert!(!a.f3_chord.held);
        tap(&mut a, KeyCode::KeyE);
        assert!(!a.inventory_open);
    }
}
