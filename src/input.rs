use sdl2::controller::GameController;
use sdl2::event::Event;
use sdl2::joystick::Joystick;
use sdl2::keyboard::Keycode;
use sdl2::{GameControllerSubsystem, JoystickSubsystem};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputAction {
    None,
    Up,
    Down,
    Confirm,
    Back,
    Quit,
}

pub struct InputHandler {
    game_controller_subsystem: GameControllerSubsystem,
    joystick_subsystem: JoystickSubsystem,
    controllers: Vec<GameController>,
    joysticks: Vec<Joystick>,
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

    pub fn handle_event(&mut self, event: &Event) -> InputAction {
        match event {
            Event::Quit { .. } => InputAction::Quit,

            Event::KeyDown {
                keycode: Some(key), ..
            } => match *key {
                Keycode::Escape => InputAction::Quit,
                Keycode::Up => InputAction::Up,
                Keycode::Down => InputAction::Down,
                Keycode::Return => InputAction::Confirm,
                Keycode::Backspace => InputAction::Back,
                _ => InputAction::None,
            },

            Event::ControllerButtonDown {
                which, button, ..
            } => {
                println!("Controller button: which={} button={:?}", which, button);
                use sdl2::controller::Button;
                match button {
                    // Menu button (TSP: controller 0, button 5)
                    _ if *which == 0 && *button as i32 == 5 => InputAction::Quit,
                    Button::DPadUp => InputAction::Up,
                    Button::DPadDown => InputAction::Down,
                    Button::A => InputAction::Confirm,
                    Button::B => InputAction::Back,
                    _ => InputAction::None,
                }
            }

            Event::JoyButtonDown {
                which, button_idx, ..
            } => {
                println!("Joy button: which={} button={}", which, button_idx);
                InputAction::None
            }

            Event::JoyDeviceAdded { .. } | Event::ControllerDeviceAdded { .. } => {
                self.open_devices();
                InputAction::None
            }

            _ => InputAction::None,
        }
    }
}
