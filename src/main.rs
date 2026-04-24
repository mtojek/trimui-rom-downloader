mod background;
mod input;
mod intro;
mod menu;
mod scene;
mod text;
mod texture;
mod widget;

use sdl2::pixels::Color;
use sdl2::render::BlendMode;
use std::time::{Duration, Instant};

use crate::background::Background;
use crate::input::{InputAction, InputHandler};
use crate::intro::IntroScene;
use crate::menu::MenuScene;
use crate::scene::{Scene, SceneResult};

pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 720;

enum ActiveScene<'a> {
    Intro(IntroScene<'a>),
    Menu(MenuScene<'a>),
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
    let mut input = InputHandler::new(&sdl_context);
    let mut event_pump = sdl_context.event_pump().unwrap();
    let start = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            let action = input.handle_event(&event);
            if action == InputAction::Quit {
                break 'running;
            }
            if action != InputAction::None {
                match &mut active_scene {
                    ActiveScene::Menu(scene) => scene.handle_input(action),
                    _ => {}
                }
            }
        }

        let elapsed = start.elapsed().as_millis();

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        let bg_alpha = match &active_scene {
            ActiveScene::Intro(scene) => scene.bg_alpha(elapsed),
            ActiveScene::Menu(_) => 255,
        };
        background.render(&mut canvas, bg_alpha);

        match &mut active_scene {
            ActiveScene::Intro(scene) => {
                let result = scene.update(elapsed);
                scene.render(&mut canvas, elapsed);
                if matches!(result, SceneResult::Next) {
                    active_scene = ActiveScene::Menu(MenuScene::new(&texture_creator));
                }
            }
            ActiveScene::Menu(scene) => {
                scene.update(elapsed);
                scene.render(&mut canvas, elapsed);
            }
        }

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
}
