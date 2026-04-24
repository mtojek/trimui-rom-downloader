use sdl2::pixels::Color;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::time::{Duration, Instant};

use crate::background::Background;
use crate::browser::{BrowserOutcome, GameBrowser};
use crate::cache::CatalogCache;
use crate::config::Config;
use crate::error::ErrorScene;
use crate::input::{InputAction, InputHandler};
use crate::intro::IntroScene;
use crate::loading::{LoadingOutcome, LoadingScene};
use crate::menu::{MenuOutcome, MenuScene};
use crate::scene::{Scene, SceneResult};

const CONFIG_PATH: &str = "sources.yaml";

enum ActiveScene<'a> {
    Intro(IntroScene<'a>),
    Menu(MenuScene<'a>),
    Loading(LoadingScene<'a>),
    Browser(GameBrowser<'a>),
    Error(ErrorScene<'a>),
}

impl<'a> ActiveScene<'a> {
    fn as_scene(&mut self) -> &mut dyn Scene {
        match self {
            ActiveScene::Intro(s) => s,
            ActiveScene::Menu(s) => s,
            ActiveScene::Loading(s) => s,
            ActiveScene::Browser(s) => s,
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
    let mut config: Option<Config> = None;
    let start = Instant::now();

    'running: loop {
        for event in event_pump.poll_iter() {
            let action = input.handle_event(&event);
            if action == InputAction::Quit {
                break 'running;
            }
            if action != InputAction::None {
                match &mut active_scene {
                    ActiveScene::Menu(scene) => {
                        match scene.handle_input(action) {
                            MenuOutcome::OpenGameBrowser { source_idx, catalog_idx } => {
                                if let Some(cfg) = &config {
                                    let source = cfg.sources[source_idx].clone();
                                    let catalog = source.catalogs[catalog_idx].clone();
                                    active_scene = ActiveScene::Loading(
                                        LoadingScene::new(texture_creator, source, catalog, CatalogCache::new(), source_idx, catalog_idx),
                                    );
                                }
                            }
                            MenuOutcome::None => {}
                        }
                    }
                    ActiveScene::Loading(scene) => {
                        match scene.handle_input(action) {
                            LoadingOutcome::Cancelled => {
                                if let Some(cfg) = &config {
                                    active_scene = ActiveScene::Menu(
                                        MenuScene::new_at_source(texture_creator, cfg.clone(), scene.source_idx),
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                    ActiveScene::Browser(scene) => {
                        match scene.handle_input(action) {
                            BrowserOutcome::Back => {
                                if let Some(cfg) = &config {
                                    active_scene = ActiveScene::Menu(
                                        MenuScene::new_at_source(texture_creator, cfg.clone(), scene.source_idx),
                                    );
                                }
                            }
                            BrowserOutcome::Refresh => {
                                if let Some(cfg) = &config {
                                    let si = scene.source_idx;
                                    let ci = scene.catalog_idx;
                                    let source = cfg.sources[si].clone();
                                    let catalog = source.catalogs[ci].clone();
                                    let cache = CatalogCache::new();
                                    let _ = cache.invalidate(&source.name, &catalog);
                                    active_scene = ActiveScene::Loading(
                                        LoadingScene::new(texture_creator, source, catalog, cache, si, ci),
                                    );
                                }
                            }
                            BrowserOutcome::None => {}
                        }
                    }
                    _ => {}
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
                        Ok(cfg) => {
                            let scene = ActiveScene::Menu(MenuScene::new(texture_creator, cfg.clone()));
                            config = Some(cfg);
                            scene
                        }
                        Err(e) => {
                            eprintln!("{}", e);
                            ActiveScene::Error(ErrorScene::new(texture_creator, &e.to_string()))
                        }
                    };
                }
            }
            ActiveScene::Loading(scene) => {
                let si = scene.source_idx;
                scene.update(elapsed);
                scene.render(canvas, elapsed);
                match scene.check_result() {
                    LoadingOutcome::Done { games, platform, source_idx, catalog_idx } => {
                        active_scene = ActiveScene::Browser(
                            GameBrowser::new(texture_creator, games, platform, source_idx, catalog_idx),
                        );
                    }
                    LoadingOutcome::Cancelled => {
                        if let Some(cfg) = &config {
                            active_scene = ActiveScene::Menu(
                                MenuScene::new_at_source(texture_creator, cfg.clone(), si),
                            );
                        }
                    }
                    LoadingOutcome::None => {}
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
