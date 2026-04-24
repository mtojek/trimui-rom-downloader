use fontdue::{Font, FontSettings};
use sdl2::pixels::PixelFormatEnum;
use sdl2::render::{BlendMode, Texture, TextureCreator};
use sdl2::video::WindowContext;

const FONT_DATA: &[u8] = include_bytes!("../assets/SourceCodePro-SemiBold.ttf");

pub struct TextRenderer {
    font: Font,
}

impl TextRenderer {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, FontSettings::default()).unwrap();
        TextRenderer { font }
    }

    pub fn render_text<'a>(
        &self,
        creator: &'a TextureCreator<WindowContext>,
        text: &str,
        size: f32,
        r: u8,
        g: u8,
        b: u8,
        a: u8,
    ) -> Texture<'a> {
        let line_metrics = self.font.horizontal_line_metrics(size).unwrap();
        let ascent = line_metrics.ascent.ceil() as i32;
        let descent = (-line_metrics.descent).ceil() as i32;
        let total_height = (ascent + descent) as u32;

        let mut glyphs = Vec::new();
        let mut total_width = 0u32;

        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, size);
            total_width += metrics.advance_width.ceil() as u32;
            glyphs.push((metrics, bitmap));
        }

        let mut pixels = vec![0u8; (total_width * total_height * 4) as usize];
        let mut cursor_x = 0i32;

        for (metrics, bitmap) in &glyphs {
            let glyph_y = ascent - metrics.height as i32 - metrics.ymin;

            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let alpha_val = bitmap[row * metrics.width + col];
                    if alpha_val == 0 {
                        continue;
                    }
                    let px = cursor_x + metrics.xmin as i32 + col as i32;
                    let py = glyph_y + row as i32;
                    if px < 0 || py < 0 || px >= total_width as i32 || py >= total_height as i32 {
                        continue;
                    }
                    let idx = ((py as u32 * total_width + px as u32) * 4) as usize;
                    let blended_a = (alpha_val as u16 * a as u16 / 255) as u8;
                    pixels[idx] = r;
                    pixels[idx + 1] = g;
                    pixels[idx + 2] = b;
                    pixels[idx + 3] = blended_a;
                }
            }
            cursor_x += metrics.advance_width.ceil() as i32;
        }

        let mut texture = creator
            .create_texture_static(PixelFormatEnum::ABGR8888, total_width, total_height)
            .unwrap();
        texture.set_blend_mode(BlendMode::Blend);
        texture
            .update(None, &pixels, (total_width * 4) as usize)
            .unwrap();
        texture
    }
}
