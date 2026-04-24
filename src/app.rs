use sdl2::pixels::Color;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::time::{Duration, Instant};

use crate::backend;
use crate::background::Background;
use crate::browser::{BrowserOutcome, GameBrowser};
use crate::cache::CatalogCache;
use crate::config::Config;
use crate::error::ErrorScene;
use crate::input::{InputAction, InputHandler};
use crate::intro::IntroScene;
use crate::menu::{MenuOutcome, MenuScene};
use crate::scene::{Scene, SceneResult};

const CONFIG_PATH: &str = "sources.yaml";

enum ActiveScene<'a> {
    Intro(IntroScene<'a>),
    Menu(MenuScene<'a>),
    Browser(GameBrowser<'a>),
    Error(ErrorScene<'a>),
}

impl<'a> ActiveScene<'a> {
    fn as_scene(&mut self) -> &mut dyn Scene {
        match self {
            ActiveScene::Intro(s) => s,
            ActiveScene::Menu(s) => s,
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
    let cache = CatalogCache::new();
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
                                    let source = &cfg.sources[source_idx];
                                    let catalog = &source.catalogs[catalog_idx];

                                    println!("Loading catalog: source='{}' path='{}' platform='{}'",
                                        source.name, catalog.path, catalog.platform);

                                    let games = if !cache.is_stale(&source.name, catalog) {
                                        println!("Cache hit, loading from disk");
                                        let cached = cache.load(&source.name, catalog).unwrap_or_default();
                                        println!("Cached games: {}", cached.len());
                                        cached
                                    } else {
                                        println!("Cache miss/stale, fetching from S3...");
                                        println!("Endpoint: {}", source.endpoint);
                                        match backend::create_backend(source) {
                                            Ok(be) => {
                                                match be.list_all_objects(catalog) {
                                                    Ok(all) => {
                                                        println!("Fetched {} games from S3", all.len());
                                                        if let Err(e) = cache.save(&source.name, catalog, &all) {
                                                            eprintln!("Cache save error: {}", e);
                                                        }
                                                        all
                                                    }
                                                    Err(e) => {
                                                        eprintln!("List error: {}", e);
                                                        Vec::new()
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("Backend error: {}", e);
                                                Vec::new()
                                            }
                                        }
                                    };

                                    active_scene = ActiveScene::Browser(
                                        GameBrowser::new(texture_creator, games, catalog.platform.clone()),
                                    );
                                }
                            }
                            MenuOutcome::None => {}
                        }
                    }
                    ActiveScene::Browser(scene) => {
                        match scene.handle_input(action) {
                            BrowserOutcome::Back => {
                                if let Some(cfg) = &config {
                                    active_scene = ActiveScene::Menu(MenuScene::new(texture_creator, cfg.clone()));
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
