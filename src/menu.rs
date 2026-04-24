use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::input::InputAction;
use crate::scene::{Scene, SceneResult};
use crate::widget::{Menu, MenuAction, MenuItem};

pub struct MenuScene<'a> {
    menu_stack: Vec<Menu<'a>>,
    texture_creator: &'a TextureCreator<WindowContext>,
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

        MenuScene {
            menu_stack: vec![menu],
            texture_creator,
        }
    }

    pub fn handle_input(&mut self, action: InputAction) {
        let depth = self.menu_stack.len();
        let menu = self.menu_stack.last_mut().unwrap();
        match menu.handle_input(action) {
            MenuAction::Selected(index) => {
                if depth == 1 && index == 0 {
                    // Browse Sources
                    let items = vec![
                        MenuItem {
                            label: "Source 1".to_string(),
                        },
                        MenuItem {
                            label: "Source 2".to_string(),
                        },
                    ];
                    let sub = Menu::new(
                        self.texture_creator,
                        &items,
                        "B: Back       A: Confirm",
                    );
                    self.menu_stack.push(sub);
                }
            }
            MenuAction::Back => {
                if self.menu_stack.len() > 1 {
                    self.menu_stack.pop();
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
        self.menu_stack.last().unwrap().render(canvas);
    }
}
