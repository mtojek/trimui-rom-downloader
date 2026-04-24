use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::scene::{Scene, SceneResult};
use crate::text::TextRenderer;
use crate::WINDOW_WIDTH;

const ERROR_FONT_SIZE: f32 = 18.0;
const ERROR_COLOR: Color = Color::RGBA(255, 80, 80, 255);
const ERROR_MARGIN: i32 = 10;

pub struct ErrorScene<'a> {
    texture: Texture<'a>,
    width: u32,
    height: u32,
}

impl<'a> ErrorScene<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>, message: &str) -> Self {
        let text_renderer = TextRenderer::new();
        let texture = text_renderer.render_text(
            texture_creator,
            message,
            ERROR_FONT_SIZE,
            ERROR_COLOR.r,
            ERROR_COLOR.g,
            ERROR_COLOR.b,
            ERROR_COLOR.a,
        );
        let query = texture.query();
        ErrorScene {
            texture,
            width: query.width,
            height: query.height,
        }
    }
}

impl<'a> Scene for ErrorScene<'a> {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, _elapsed: u128) {
        let x = WINDOW_WIDTH as i32 - self.width as i32 - ERROR_MARGIN;
        let y = ERROR_MARGIN;
        canvas
            .copy(
                &self.texture,
                None,
                Rect::new(x, y, self.width, self.height),
            )
            .unwrap();
    }
}
