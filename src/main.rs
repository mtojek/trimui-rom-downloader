mod app;
mod backend;
mod background;
mod browser;
mod cache;
mod config;
mod error;
mod input;
mod intro;
mod menu;
mod scene;
mod text;
mod texture;
mod widget;

use sdl2::render::BlendMode;

use crate::input::InputHandler;

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

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
    let mut input = InputHandler::new(&sdl_context);
    let mut event_pump = sdl_context.event_pump().unwrap();

    app::run(&mut canvas, &texture_creator, &mut input, &mut event_pump);
}
