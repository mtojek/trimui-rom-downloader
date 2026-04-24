use sdl2::pixels::Color;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::time::{Duration, Instant};

use crate::background::Background;
use crate::config::Config;
use crate::error::ErrorScene;
use crate::input::{InputAction, InputHandler};
use crate::intro::IntroScene;
use crate::menu::MenuScene;
use crate::scene::{Scene, SceneResult};

const CONFIG_PATH: &str = "sources.yaml";

enum ActiveScene<'a> {
    Intro(IntroScene<'a>),
    Menu(MenuScene<'a>),
    Error(ErrorScene<'a>),
}

impl<'a> ActiveScene<'a> {
    fn as_scene(&mut self) -> &mut dyn Scene {
        match self {
            ActiveScene::Intro(s) => s,
            ActiveScene::Menu(s) => s,
            ActiveScene::Error(s) => s,
        }
    }
}

pub fn run(
    canvas: &mut Canvas<Window>,
    texture_creator: &TextureCreator<WindowContext>,
    input: &mut InputHandler,
    event_pump: &mut sdl2::EventPump,
) {
    let mut background = Background::new(texture_creator);
    let mut active_scene = ActiveScene::Intro(IntroScene::new(texture_creator));
    let start = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            let action = input.handle_event(&event);
            if action == InputAction::Quit {
                break 'running;
            }
            if action != InputAction::None {
                if let ActiveScene::Menu(scene) = &mut active_scene {
                    scene.handle_input(action);
                }
            }
        }

        let elapsed = start.elapsed().as_millis();

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        let bg_alpha = match &active_scene {
            ActiveScene::Intro(scene) => scene.bg_alpha(elapsed),
            _ => 255,
        };
        background.render(canvas, bg_alpha);

        match &mut active_scene {
            ActiveScene::Intro(scene) => {
                let result = scene.update(elapsed);
                scene.render(canvas, elapsed);
                if matches!(result, SceneResult::Next) {
                    active_scene = match Config::load(CONFIG_PATH) {
                        Ok(_config) => ActiveScene::Menu(MenuScene::new(texture_creator)),
                        Err(e) => {
                            eprintln!("{}", e);
                            ActiveScene::Error(ErrorScene::new(texture_creator, &e.to_string()))
                        }
                    };
                }
            }
            other => {
                let scene = other.as_scene();
                scene.update(elapsed);
                scene.render(canvas, elapsed);
            }
        }

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
}
