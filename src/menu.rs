use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::input::InputAction;
use crate::scene::{Scene, SceneResult};
use crate::widget::{Menu, MenuAction, MenuItem};

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Main,
    BrowseSources,
}

impl State {
    fn items(&self) -> Vec<MenuItem<State>> {
        match self {
            State::Main => vec![
                MenuItem { label: "Browse Sources".to_string(), target: Some(State::BrowseSources) },
                MenuItem { label: "My Games".to_string(), target: None },
            ],
            State::BrowseSources => vec![
                MenuItem { label: "Source 1".to_string(), target: None },
                MenuItem { label: "Source 2".to_string(), target: None },
            ],
        }
    }

    fn legend(&self) -> &str {
        match self {
            State::Main => "Menu: Exit       A: Confirm",
            State::BrowseSources => "Menu: Exit       B: Back       A: Confirm",
        }
    }

    fn parent(&self) -> Option<State> {
        match self {
            State::BrowseSources => Some(State::Main),
            _ => None,
        }
    }
}

pub struct MenuScene<'a> {
    state: State,
    menu: Menu<'a, State>,
    texture_creator: &'a TextureCreator<WindowContext>,
}

impl<'a> MenuScene<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        let state = State::Main;
        let menu = Menu::new(texture_creator, &state.items(), state.legend());
        MenuScene { state, menu, texture_creator }
    }

    fn transition(&mut self, new_state: State) {
        self.state = new_state;
        self.menu = Menu::new(self.texture_creator, &new_state.items(), new_state.legend());
    }

    pub fn handle_input(&mut self, action: InputAction) {
        match self.menu.handle_input(action) {
            MenuAction::Selected(next) => {
                self.transition(next);
            }
            MenuAction::Back => {
                if let Some(parent) = self.state.parent() {
                    self.transition(parent);
                }
            }
            MenuAction::None => {}
        }
    }
}

impl<'a> Scene for MenuScene<'a> {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, _elapsed: u128) {
        self.menu.render(canvas);
    }
}
