use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::scene::{Scene, SceneResult};

#[allow(dead_code)]
pub struct MenuScene {
    // TODO: menu textures and state
}

impl MenuScene {
    #[allow(dead_code)]
    pub fn new() -> Self {
        MenuScene {}
    }
}

impl Scene for MenuScene {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, _canvas: &mut Canvas<Window>, _elapsed: u128) {
        // TODO: render menu elements (background stays from intro)
    }
}
