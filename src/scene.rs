use sdl2::render::Canvas;
use sdl2::video::Window;

pub enum SceneResult {
    Continue,
    Next,
}

pub trait Scene {
    fn update(&mut self, elapsed: u128) -> SceneResult;
    fn render(&mut self, canvas: &mut Canvas<Window>, elapsed: u128);
}
