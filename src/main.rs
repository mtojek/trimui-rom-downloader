mod background;
mod intro;
mod menu;
mod scene;
mod text;
mod texture;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;
use std::time::{Duration, Instant};

use crate::background::Background;
use crate::intro::IntroScene;
use crate::scene::{Scene, SceneResult};

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

enum ActiveScene<'a> {
    Intro(IntroScene<'a>),
    Menu,
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
    let mut background = Background::new(&texture_creator);
    let mut active_scene = ActiveScene::Intro(IntroScene::new(&texture_creator));
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

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        let bg_alpha = match &active_scene {
            ActiveScene::Intro(scene) => scene.bg_alpha(elapsed),
            ActiveScene::Menu => 255,
        };
        background.render(&mut canvas, bg_alpha);

        match &mut active_scene {
            ActiveScene::Intro(scene) => {
                let result = scene.update(elapsed);
                scene.render(&mut canvas, elapsed);
                if matches!(result, SceneResult::Next) {
                    active_scene = ActiveScene::Menu;
                }
            }
            ActiveScene::Menu => {
                // TODO: menu scene rendering
            }
        }

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
}
