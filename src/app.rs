use sdl2::pixels::Color;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::time::{Duration, Instant};

use crate::background::Background;
use crate::browser::{BrowserOutcome, GameBrowser};
use crate::cache::CatalogCache;
use crate::config::Config;
use crate::download::DownloadManager;
use crate::error::ErrorScene;
use crate::input::{InputAction, InputHandler};
use crate::install_dir::InstallDirResolver;
use crate::intro::IntroScene;
use crate::library::MyGames;
use crate::loading::{LoadingOutcome, LoadingScene};
use crate::menu::{MenuOutcome, MenuScene};
use crate::mygames::{MyGamesOutcome, MyGamesScene};
use crate::scene::{Scene, SceneResult};

const CONFIG_PATH: &str = "sources.yaml";

enum ActiveScene<'a> {
    Intro(IntroScene<'a>),
    Menu(MenuScene<'a>),
    Loading(LoadingScene<'a>),
    Browser(GameBrowser<'a>),
    MyGames(MyGamesScene<'a>),
    Error(ErrorScene<'a>),
}

impl<'a> ActiveScene<'a> {
    fn as_scene(&mut self) -> &mut dyn Scene {
        match self {
            ActiveScene::Intro(s) => s,
            ActiveScene::Menu(s) => s,
            ActiveScene::Loading(s) => s,
            ActiveScene::Browser(s) => s,
            ActiveScene::MyGames(s) => s,
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
    let mut download_mgr: Option<DownloadManager> = None;
    let mut my_games = MyGames::new();
    let install_resolver = InstallDirResolver::new();
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
                            MenuOutcome::OpenGameBrowser { source_idx } => {
                                if let Some(cfg) = &config {
                                    let source = cfg.sources[source_idx].clone();
                                    active_scene = ActiveScene::Loading(
                                        LoadingScene::new(texture_creator, source, cfg.clone(), CatalogCache::new(), source_idx),
                                    );
                                }
                            }
                            MenuOutcome::OpenMyGames => {
                                if let Some(dm) = &download_mgr {
                                    active_scene = ActiveScene::MyGames(
                                        MyGamesScene::new(texture_creator, &my_games, dm),
                                    );
                                }
                            }
                            MenuOutcome::RefreshAll => {
                                if let Some(cfg) = &config {
                                    active_scene = ActiveScene::Loading(
                                        LoadingScene::new_refresh_all(texture_creator, cfg.clone()),
                                    );
                                }
                            }
                            MenuOutcome::None => {}
                        }
                    }
                    ActiveScene::Loading(scene) => {
                        let is_refresh_all = scene.refresh_all;
                        let si = scene.source_idx;
                        match scene.handle_input(action) {
                            LoadingOutcome::Cancelled => {
                                if let Some(cfg) = &config {
                                    if is_refresh_all {
                                        let mut menu = MenuScene::new(texture_creator, cfg.clone());
                                        menu.go_to_browse_sources();
                                        active_scene = ActiveScene::Menu(menu);
                                    } else {
                                        active_scene = ActiveScene::Menu(
                                            MenuScene::new_at_source(texture_creator, cfg.clone(), si),
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    ActiveScene::Browser(scene) => {
                        if let Some(dm) = &download_mgr {
                            match scene.handle_input(action, &mut my_games, dm, &install_resolver) {
                                BrowserOutcome::Back => {
                                    if let Some(cfg) = &config {
                                        active_scene = ActiveScene::Menu(
                                            MenuScene::new_at_source(texture_creator, cfg.clone(), scene.source_idx),
                                        );
                                    }
                                }
                                BrowserOutcome::None => {}
                            }
                        }
                    }
                    ActiveScene::MyGames(scene) => {
                        if let Some(dm) = &download_mgr {
                            match scene.handle_input(action, &mut my_games, dm, &install_resolver) {
                                MyGamesOutcome::Back => {
                                    if let Some(cfg) = &config {
                                        active_scene = ActiveScene::Menu(
                                            MenuScene::new(texture_creator, cfg.clone()),
                                        );
                                    }
                                }
                                MyGamesOutcome::None => {}
                            }
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
                            download_mgr = Some(DownloadManager::new(cfg.clone(), &install_resolver));
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
                    LoadingOutcome::Done { games, source, platform, source_idx, .. } => {
                        if let Some(dm) = &download_mgr {
                            active_scene = ActiveScene::Browser(
                                GameBrowser::new(texture_creator, games, source, platform, source_idx, &my_games, dm),
                            );
                        }
                    }
                    LoadingOutcome::RefreshDone => {
                        if let Some(cfg) = &config {
                            let mut menu = MenuScene::new(texture_creator, cfg.clone());
                            menu.go_to_browse_sources();
                            active_scene = ActiveScene::Menu(menu);
                        }
                    }
                    LoadingOutcome::Cancelled => {
                        if let Some(cfg) = &config {
                            if scene.refresh_all {
                                let mut menu = MenuScene::new(texture_creator, cfg.clone());
                                menu.go_to_browse_sources();
                                active_scene = ActiveScene::Menu(menu);
                            } else {
                                active_scene = ActiveScene::Menu(
                                    MenuScene::new_at_source(texture_creator, cfg.clone(), si),
                                );
                            }
                        }
                    }
                    LoadingOutcome::None => {}
                }
            }
            ActiveScene::Browser(scene) => {
                if let Some(dm) = &download_mgr {
                    scene.refresh_statuses(&my_games, dm);
                }
                scene.update(elapsed);
                scene.render(canvas, elapsed);
            }
            ActiveScene::MyGames(scene) => {
                if let Some(dm) = &download_mgr {
                    scene.refresh(&my_games, dm);
                }
                scene.update(elapsed);
                scene.render(canvas, elapsed);
            }
            other => {
                let scene = other.as_scene();
                scene.update(elapsed);
                scene.render(canvas, elapsed);
            }
        }

        // Poll download events — add completed downloads to MyGames
        if let Some(dm) = &download_mgr {
            for event in dm.poll_events() {
                match &event {
                    crate::download::DownloadEvent::Completed { id } => {
                        for entry in dm.statuses() {
                            if entry.id == *id {
                                eprintln!(
                                    "[APP] Download #{} completed, adding '{}' to MyGames (platform={})",
                                    id, entry.game_key, entry.platform
                                );
                                let result = my_games.add(crate::library::GameEntry {
                                    key: entry.game_key.clone(),
                                    source: entry.source_name.clone(),
                                    platform: entry.platform.clone(),
                                });
                                match result {
                                    Ok(()) => eprintln!("[APP] '{}' added to MyGames successfully", entry.file_name),
                                    Err(e) => eprintln!("[APP] Failed to add '{}' to MyGames: {}", entry.file_name, e),
                                }
                                break;
                            }
                        }
                    }
                    crate::download::DownloadEvent::Failed { id, error } => {
                        eprintln!("[APP] Download #{} failed: {}", id, error);
                    }
                }
            }
        }

        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
}
