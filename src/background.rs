use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::text::TextRenderer;
use crate::texture::load_texture;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

pub struct Background<'a> {
    texture: Texture<'a>,
    version_texture: Texture<'a>,
    version_rect: Rect,
}

impl<'a> Background<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        let texture = load_texture(texture_creator, include_bytes!("../assets/background.png"));

        let text_renderer = TextRenderer::new();
        let version_text = format!("ROM Downloader, v{}", env!("CARGO_PKG_VERSION"));
        let version_texture =
            text_renderer.render_text(texture_creator, &version_text, 18.0, 255, 255, 255, 180);
        let query = version_texture.query();
        let version_rect = Rect::new(10, 8, query.width, query.height);

        Background {
            texture,
            version_texture,
            version_rect,
        }
    }

    pub fn render(&mut self, canvas: &mut Canvas<Window>, alpha: u8) {
        self.texture.set_alpha_mod(alpha);
        canvas
            .copy(
                &self.texture,
                None,
                Rect::new(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT),
            )
            .unwrap();

        if alpha > 0 {
            canvas
                .copy(&self.version_texture, None, self.version_rect)
                .unwrap();
        }
    }
}
