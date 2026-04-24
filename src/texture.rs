use sdl2::pixels::PixelFormatEnum;
use sdl2::render::BlendMode;

pub fn load_texture<'a>(
    creator: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    data: &[u8],
) -> sdl2::render::Texture<'a> {
    let img = image::load_from_memory(data)
        .unwrap_or_else(|e| panic!("Failed to decode image: {}", e))
        .into_rgba8();
    let (w, h) = img.dimensions();
    let mut texture = creator
        .create_texture_static(PixelFormatEnum::ABGR8888, w, h)
        .unwrap();
    texture.set_blend_mode(BlendMode::Blend);
    texture.update(None, &img, (w * 4) as usize).unwrap();
    texture
}
