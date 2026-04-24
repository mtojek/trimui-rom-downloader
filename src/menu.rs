use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::input::InputAction;
use crate::scene::{Scene, SceneResult};
use crate::widget::{Menu, MenuAction, MenuItem};

pub struct MenuScene<'a> {
    menu: Menu<'a>,
}

impl<'a> MenuScene<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        let items = vec![
            MenuItem {
                label: "Browse Sources".to_string(),
            },
            MenuItem {
                label: "My Games".to_string(),
            },
        ];

        let menu = Menu::new(texture_creator, &items, "Menu: Exit       A: Confirm");

        MenuScene { menu }
    }

    pub fn handle_input(&mut self, action: InputAction) {
        match self.menu.handle_input(action) {
            MenuAction::Selected(index) => {
                println!("Selected menu item: {}", index);
                // TODO: push sub-scene based on index
            }
            MenuAction::Back => {
                // At top-level menu, Back does nothing
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
