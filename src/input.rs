use sdl2::controller::GameController;
use sdl2::event::Event;
use sdl2::joystick::Joystick;
use sdl2::keyboard::Keycode;
use sdl2::{GameControllerSubsystem, JoystickSubsystem};
use std::time::Instant;

const REPEAT_DELAY_MS: u128 = 300;
const REPEAT_INTERVAL_MS: u128 = 50;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputAction {
    None,
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
    Action,
    Refresh,
    Quit,
}

pub struct InputHandler {
    game_controller_subsystem: GameControllerSubsystem,
    joystick_subsystem: JoystickSubsystem,
    controllers: Vec<GameController>,
    joysticks: Vec<Joystick>,
    held_action: InputAction,
    held_since: Instant,
    last_repeat: Instant,
}

impl InputHandler {
    pub fn new(sdl_context: &sdl2::Sdl) -> Self {
        let game_controller_subsystem = sdl_context.game_controller().unwrap();
        let joystick_subsystem = sdl_context.joystick().unwrap();

        let mut handler = InputHandler {
            game_controller_subsystem,
            joystick_subsystem,
            controllers: Vec::new(),
            joysticks: Vec::new(),
            held_action: InputAction::None,
            held_since: Instant::now(),
            last_repeat: Instant::now(),
        };
        handler.open_devices();
        handler
    }

    fn open_devices(&mut self) {
        self.controllers.clear();
        self.joysticks.clear();

        let num = self.joystick_subsystem.num_joysticks().unwrap_or(0);
        for i in 0..num {
            if self.game_controller_subsystem.is_game_controller(i) {
                if let Ok(c) = self.game_controller_subsystem.open(i) {
                    let name = self
                        .game_controller_subsystem
                        .name_for_index(i)
                        .unwrap_or_else(|_| "Unknown".to_string());
                    println!("Controller opened: idx={} name={}", i, name);
                    self.controllers.push(c);
                }
                continue;
            }
            if let Ok(j) = self.joystick_subsystem.open(i) {
                let name = self
                    .joystick_subsystem
                    .name_for_index(i)
                    .unwrap_or_else(|_| "Unknown".to_string());
                println!("Joystick opened: idx={} name={}", i, name);
                self.joysticks.push(j);
            }
        }
    }

    fn is_repeatable(action: InputAction) -> bool {
        matches!(action, InputAction::Up | InputAction::Down | InputAction::Left | InputAction::Right)
    }

    pub fn poll_repeat(&mut self) -> InputAction {
        if self.held_action == InputAction::None {
            return InputAction::None;
        }
        let now = Instant::now();
        let held_ms = now.duration_since(self.held_since).as_millis();
        if held_ms < REPEAT_DELAY_MS {
            return InputAction::None;
        }
        let since_last = now.duration_since(self.last_repeat).as_millis();
        if since_last >= REPEAT_INTERVAL_MS {
            self.last_repeat = now;
            return self.held_action;
        }
        InputAction::None
    }

    pub fn handle_event(&mut self, event: &Event) -> InputAction {
        match event {
            Event::Quit { .. } => InputAction::Quit,

            Event::KeyDown {
                keycode: Some(key), repeat, ..
            } => {
                if *repeat { return InputAction::None; }
                let action = match *key {
                    Keycode::Escape => InputAction::Quit,
                    Keycode::Up => InputAction::Up,
                    Keycode::Down => InputAction::Down,
                    Keycode::O => InputAction::Left,
                    Keycode::P => InputAction::Right,
                    Keycode::Return => InputAction::Confirm,
                    Keycode::Backspace => InputAction::Back,
                    Keycode::X => InputAction::Action,
                    Keycode::Y => InputAction::Refresh,
                    _ => InputAction::None,
                };
                if Self::is_repeatable(action) {
                    self.held_action = action;
                    self.held_since = Instant::now();
                    self.last_repeat = Instant::now();
                }
                action
            }

            Event::KeyUp { keycode: Some(key), .. } => {
                let released = match *key {
                    Keycode::Up => InputAction::Up,
                    Keycode::Down => InputAction::Down,
                    Keycode::O => InputAction::Left,
                    Keycode::P => InputAction::Right,
                    _ => InputAction::None,
                };
                if released == self.held_action {
                    self.held_action = InputAction::None;
                }
                InputAction::None
            }

            Event::ControllerButtonDown {
                which, button, ..
            } => {
                println!("Controller button: which={} button={:?}", which, button);
                use sdl2::controller::Button;
                let action = match button {
                    _ if *which == 0 && *button as i32 == 5 => InputAction::Quit,
                    Button::DPadUp => InputAction::Up,
                    Button::DPadDown => InputAction::Down,
                    Button::LeftShoulder => InputAction::Left,
                    Button::RightShoulder => InputAction::Right,
                    Button::A => InputAction::Back,
                    Button::B => InputAction::Confirm,
                    Button::X => InputAction::Refresh,
                    Button::Y => InputAction::Action,
                    _ => InputAction::None,
                };
                if Self::is_repeatable(action) {
                    self.held_action = action;
                    self.held_since = Instant::now();
                    self.last_repeat = Instant::now();
                }
                action
            }

            Event::ControllerButtonUp { button, .. } => {
                use sdl2::controller::Button;
                let released = match button {
                    Button::DPadUp => InputAction::Up,
                    Button::DPadDown => InputAction::Down,
                    Button::LeftShoulder => InputAction::Left,
                    Button::RightShoulder => InputAction::Right,
                    _ => InputAction::None,
                };
                if released == self.held_action {
                    self.held_action = InputAction::None;
                }
                InputAction::None
            }

            Event::JoyButtonDown {
                which, button_idx, ..
            } => {
                println!("Joy button: which={} button={}", which, button_idx);
                InputAction::None
            }

            Event::ControllerAxisMotion { axis, value, .. } => {
                use sdl2::controller::Axis;
                match axis {
                    Axis::TriggerLeft if *value > 16000 => InputAction::Left,
                    Axis::TriggerRight if *value > 16000 => InputAction::Right,
                    _ => InputAction::None,
                }
            }

            Event::JoyDeviceAdded { .. } | Event::ControllerDeviceAdded { .. } => {
                self.open_devices();
                InputAction::None
            }

            _ => InputAction::None,
        }
    }
}
