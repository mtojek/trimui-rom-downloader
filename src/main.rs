use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::BlendMode;
use std::time::{Duration, Instant};

const WINDOW_WIDTH: u32 = 1280;
const WINDOW_HEIGHT: u32 = 720;
const CRAB_FADE_IN_MS: u128 = 300;    // krab: fade-in 300ms
const BG_FADE_START_MS: u128 = 1000;  // tło: starts 700ms after krab
const BG_FADE_IN_MS: u128 = 1000;     // tło: fade-in 1000ms
const CART_START_MS: u128 = 2200;     // cart: starts after bg is done (BG_FADE_START + BG_FADE_IN + 200ms pause)
const CART_SLIDE_MS: u128 = 600;      // cart: slide down duration
const EXIT_START_MS: u128 = 3600;    // exit: 800ms after cart arrives (2800 + 800)
const EXIT_SLIDE_MS: u128 = 600;     // exit: slide out duration

fn load_texture<'a>(
    creator: &'a sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    path: &str,
) -> sdl2::render::Texture<'a> {
    let img = image::open(path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {}", path, e))
        .into_rgba8();
    let (w, h) = img.dimensions();
    let mut texture = creator
        .create_texture_static(PixelFormatEnum::ABGR8888, w, h)
        .unwrap();
    texture.set_blend_mode(BlendMode::Blend);
    texture.update(None, &img, (w * 4) as usize).unwrap();
    texture
}

fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("ROM Downloader", WINDOW_WIDTH, WINDOW_HEIGHT)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    canvas.set_blend_mode(BlendMode::Blend);

    let texture_creator = canvas.texture_creator();
    let mut bg_texture = load_texture(&texture_creator, "assets/background.png");
    let mut crab_front_texture = load_texture(&texture_creator, "assets/sprite-crab-front.png");
    let mut crab_back_texture = load_texture(&texture_creator, "assets/sprite-crab-back.png");
    let cart_texture = load_texture(&texture_creator, "assets/sprite-cart.png");

    let crab_front_query = crab_front_texture.query();
    let crab_front_w = (crab_front_query.width as f32 * 0.9) as u32;
    let crab_front_h = (crab_front_query.height as f32 * 0.9) as u32;
    let crab_front_x = (WINDOW_WIDTH as i32 - crab_front_w as i32) / 2;
    let crab_front_y = (WINDOW_HEIGHT as i32 - crab_front_h as i32) / 2 + 50;
    let crab_front_rect = Rect::new(crab_front_x, crab_front_y, crab_front_w, crab_front_h);

    let crab_back_query = crab_back_texture.query();
    let crab_back_w = (crab_back_query.width as f32 * 0.9) as u32;
    let crab_back_h = (crab_back_query.height as f32 * 0.9) as u32;
    let crab_back_x = (WINDOW_WIDTH as i32 - crab_back_w as i32) / 2;
    let crab_back_y = (WINDOW_HEIGHT as i32 - crab_back_h as i32) / 2 - 50;
    let crab_back_rect = Rect::new(crab_back_x, crab_back_y, crab_back_w, crab_back_h);

    let cart_query = cart_texture.query();
    let cart_w = (cart_query.width as f32 * 0.45) as u32;
    let cart_h = (cart_query.height as f32 * 0.45) as u32;
    let cart_x = (WINDOW_WIDTH as i32 - cart_w as i32) / 2 + 7;
    // cart slides from above crab down to just above crab (in crab's claws)
    let cart_y_final = crab_front_y - cart_h as i32 + 20 + 75; // overlap slightly with crab, +75px down
    let cart_y_start = -(cart_h as i32); // starts off-screen top

    let mut event_pump = sdl_context.event_pump().unwrap();
    let start = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        let elapsed = start.elapsed().as_millis();

        // krab: fade-in 0–300ms
        let crab_alpha =
            ((elapsed.min(CRAB_FADE_IN_MS) as f32 / CRAB_FADE_IN_MS as f32) * 255.0) as u8;

        // background: fade-in starting at BG_FADE_START_MS
        let bg_alpha = if elapsed < BG_FADE_START_MS {
            0
        } else {
            let t =
                (elapsed - BG_FADE_START_MS).min(BG_FADE_IN_MS) as f32 / BG_FADE_IN_MS as f32;
            (t * 255.0) as u8
        };

        // exit offset: all sprites slide down together
        let exit_offset = if elapsed < EXIT_START_MS {
            0
        } else {
            let t = ((elapsed - EXIT_START_MS).min(EXIT_SLIDE_MS) as f32) / EXIT_SLIDE_MS as f32;
            let eased = t * t; // ease-in: accelerate as it leaves
            (WINDOW_HEIGHT as f32 * 1.5 * eased) as i32
        };

        // cart: slides down from top after CART_START_MS
        let cart_visible = elapsed >= CART_START_MS;
        let cart_y = if !cart_visible {
            cart_y_start
        } else {
            let t = ((elapsed - CART_START_MS).min(CART_SLIDE_MS) as f32) / CART_SLIDE_MS as f32;
            // ease-out: decelerate as it arrives
            let eased = 1.0 - (1.0 - t) * (1.0 - t);
            cart_y_start + ((cart_y_final - cart_y_start) as f32 * eased) as i32 + exit_offset
        };

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        // background
        bg_texture.set_alpha_mod(bg_alpha);
        canvas
            .copy(
                &bg_texture,
                None,
                Rect::new(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT),
            )
            .unwrap();

        // crab-back (behind cart)
        crab_back_texture.set_alpha_mod(crab_alpha);
        let crab_back_rect_anim = Rect::new(crab_back_rect.x(), crab_back_rect.y() + exit_offset, crab_back_rect.width(), crab_back_rect.height());
        canvas.copy(&crab_back_texture, None, crab_back_rect_anim).unwrap();

        // cart (between crab layers, slides down from top, rotated 7°)
        if cart_visible {
            canvas
                .copy_ex(
                    &cart_texture,
                    None,
                    Rect::new(cart_x, cart_y, cart_w, cart_h),
                    7.0,
                    None,
                    false,
                    false,
                )
                .unwrap();
        }

        // crab-front (on top of cart)
        crab_front_texture.set_alpha_mod(crab_alpha);
        let crab_front_rect_anim = Rect::new(crab_front_rect.x(), crab_front_rect.y() + exit_offset, crab_front_rect.width(), crab_front_rect.height());
        canvas.copy(&crab_front_texture, None, crab_front_rect_anim).unwrap();

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
}
