use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::scene::{Scene, SceneResult};
use crate::texture::load_texture;
use crate::WINDOW_HEIGHT;
use crate::WINDOW_WIDTH;

const CRAB_FADE_IN_MS: u128 = 300;
const BG_FADE_START_MS: u128 = 1000;
const BG_FADE_IN_MS: u128 = 1000;
const CART_START_MS: u128 = 2200;
const CART_SLIDE_MS: u128 = 600;
const EXIT_START_MS: u128 = 3600;
const EXIT_SLIDE_MS: u128 = 600;

pub struct IntroScene<'a> {
    crab_front_texture: Texture<'a>,
    crab_back_texture: Texture<'a>,
    cart_texture: Texture<'a>,
    crab_front_rect: Rect,
    crab_back_rect: Rect,
    cart_x: i32,
    cart_w: u32,
    cart_h: u32,
    cart_y_start: i32,
    cart_y_final: i32,
}

impl<'a> IntroScene<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>) -> Self {
        let crab_front_texture = load_texture(texture_creator, include_bytes!("../assets/sprite-crab-front.png"));
        let crab_back_texture = load_texture(texture_creator, include_bytes!("../assets/sprite-crab-back.png"));
        let cart_texture = load_texture(texture_creator, include_bytes!("../assets/sprite-cart.png"));

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
        let cart_y_final = crab_front_y - cart_h as i32 + 20 + 75;
        let cart_y_start = -(cart_h as i32);

        IntroScene {
            crab_front_texture,
            crab_back_texture,
            cart_texture,
            crab_front_rect,
            crab_back_rect,
            cart_x,
            cart_w,
            cart_h,
            cart_y_start,
            cart_y_final,
        }
    }

    pub fn bg_alpha(&self, elapsed: u128) -> u8 {
        if elapsed < BG_FADE_START_MS {
            0
        } else {
            let t =
                (elapsed - BG_FADE_START_MS).min(BG_FADE_IN_MS) as f32 / BG_FADE_IN_MS as f32;
            (t * 255.0) as u8
        }
    }

    fn crab_alpha(&self, elapsed: u128) -> u8 {
        ((elapsed.min(CRAB_FADE_IN_MS) as f32 / CRAB_FADE_IN_MS as f32) * 255.0) as u8
    }

    fn exit_offset(&self, elapsed: u128) -> i32 {
        if elapsed < EXIT_START_MS {
            0
        } else {
            let t =
                ((elapsed - EXIT_START_MS).min(EXIT_SLIDE_MS) as f32) / EXIT_SLIDE_MS as f32;
            let eased = t * t;
            (WINDOW_HEIGHT as f32 * 1.5 * eased) as i32
        }
    }

    fn cart_y(&self, elapsed: u128, exit_offset: i32) -> (bool, i32) {
        let visible = elapsed >= CART_START_MS;
        let y = if !visible {
            self.cart_y_start
        } else {
            let t = ((elapsed - CART_START_MS).min(CART_SLIDE_MS) as f32) / CART_SLIDE_MS as f32;
            let eased = 1.0 - (1.0 - t) * (1.0 - t);
            self.cart_y_start
                + ((self.cart_y_final - self.cart_y_start) as f32 * eased) as i32
                + exit_offset
        };
        (visible, y)
    }

    pub fn is_done(elapsed: u128) -> bool {
        elapsed >= EXIT_START_MS + EXIT_SLIDE_MS
    }
}

impl<'a> Scene for IntroScene<'a> {
    fn update(&mut self, elapsed: u128) -> SceneResult {
        if IntroScene::is_done(elapsed) {
            SceneResult::Next
        } else {
            SceneResult::Continue
        }
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, elapsed: u128) {
        let crab_alpha = self.crab_alpha(elapsed);
        let exit_offset = self.exit_offset(elapsed);
        let (cart_visible, cart_y) = self.cart_y(elapsed, exit_offset);

        // crab-back (behind cart)
        self.crab_back_texture.set_alpha_mod(crab_alpha);
        let crab_back_rect_anim = Rect::new(
            self.crab_back_rect.x(),
            self.crab_back_rect.y() + exit_offset,
            self.crab_back_rect.width(),
            self.crab_back_rect.height(),
        );
        canvas
            .copy(&self.crab_back_texture, None, crab_back_rect_anim)
            .unwrap();

        // cart (between crab layers, rotated 7°)
        if cart_visible {
            canvas
                .copy_ex(
                    &self.cart_texture,
                    None,
                    Rect::new(self.cart_x, cart_y, self.cart_w, self.cart_h),
                    7.0,
                    None,
                    false,
                    false,
                )
                .unwrap();
        }

        // crab-front (on top of cart)
        self.crab_front_texture.set_alpha_mod(crab_alpha);
        let crab_front_rect_anim = Rect::new(
            self.crab_front_rect.x(),
            self.crab_front_rect.y() + exit_offset,
            self.crab_front_rect.width(),
            self.crab_front_rect.height(),
        );
        canvas
            .copy(&self.crab_front_texture, None, crab_front_rect_anim)
            .unwrap();
    }
}
