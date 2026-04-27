use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use crate::backend::{self, RemoteGame};
use crate::cache::CatalogCache;
use crate::config::{Catalog, Config, Source};
use crate::input::InputAction;
use crate::scene::{Scene, SceneResult};
use crate::text::TextRenderer;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

const LOG_FONT_SIZE: f32 = 22.0;
const LEGEND_FONT_SIZE: f32 = 28.0;
const LOG_COLOR: Color = Color::RGBA(180, 220, 180, 255);
const BOX_COLOR: Color = Color::RGBA(0, 0, 0, 200);
const LEGEND_COLOR: Color = Color::RGBA(0, 0, 0, 220);
const MAX_LOG_LINES: usize = 15;
const LOG_LINE_HEIGHT: i32 = 30;
const BOX_MARGIN: i32 = 80;
const BOX_PADDING: i32 = 15;
const LEGEND_BOTTOM_MARGIN: i32 = 12;

pub enum LoadingOutcome {
    None,
    Done {
        games: Vec<RemoteGame>,
        source_idx: usize,
    },
    RefreshDone,
    Cancelled,
}

enum LoadingResult {
    Single(Option<Vec<RemoteGame>>),
    RefreshAll(bool),
}

pub struct LoadingScene<'a> {
    log_lines: Vec<String>,
    log_rx: Receiver<String>,
    cancel: Arc<AtomicBool>,
    handle: Option<JoinHandle<LoadingResult>>,
    pub source_idx: usize,
    pub refresh_all: bool,
    rendered_lines: Vec<(Texture<'a>, u32, u32)>,
    legend_texture: Texture<'a>,
    legend_w: u32,
    legend_h: u32,
    texture_creator: &'a TextureCreator<WindowContext>,
    dirty: bool,
}

impl<'a> LoadingScene<'a> {
    pub fn new(
        texture_creator: &'a TextureCreator<WindowContext>,
        source: Source,
        catalog: Catalog,
        cache: CatalogCache,
        source_idx: usize,
    ) -> Self {
        let (log_tx, log_rx) = std::sync::mpsc::channel::<String>();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        let handle = std::thread::spawn(move || {
            LoadingResult::Single(Self::fetch_games(source, catalog, cache, log_tx, cancel_clone))
        });

        let text = TextRenderer::new();
        let legend = text.render_text(
            texture_creator,
            "B: Cancel",
            LEGEND_FONT_SIZE,
            LEGEND_COLOR.r, LEGEND_COLOR.g, LEGEND_COLOR.b, LEGEND_COLOR.a,
        );
        let lq = legend.query();

        LoadingScene {
            log_lines: Vec::new(),
            log_rx,
            cancel,
            handle: Some(handle),
            source_idx,
            refresh_all: false,
            rendered_lines: Vec::new(),
            legend_texture: legend,
            legend_w: lq.width,
            legend_h: lq.height,
            texture_creator,
            dirty: false,
        }
    }

    pub fn new_refresh_all(
        texture_creator: &'a TextureCreator<WindowContext>,
        config: Config,
    ) -> Self {
        let (log_tx, log_rx) = std::sync::mpsc::channel::<String>();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        let handle = std::thread::spawn(move || {
            LoadingResult::RefreshAll(Self::refresh_all_sources(config, log_tx, cancel_clone))
        });

        let text = TextRenderer::new();
        let legend = text.render_text(
            texture_creator,
            "B: Cancel",
            LEGEND_FONT_SIZE,
            LEGEND_COLOR.r, LEGEND_COLOR.g, LEGEND_COLOR.b, LEGEND_COLOR.a,
        );
        let lq = legend.query();

        LoadingScene {
            log_lines: Vec::new(),
            log_rx,
            cancel,
            handle: Some(handle),
            source_idx: 0,
            refresh_all: true,
            rendered_lines: Vec::new(),
            legend_texture: legend,
            legend_w: lq.width,
            legend_h: lq.height,
            texture_creator,
            dirty: false,
        }
    }

    fn refresh_all_sources(
        config: Config,
        log: Sender<String>,
        cancel: Arc<AtomicBool>,
    ) -> bool {
        let cache = CatalogCache::new();
        for source in &config.sources {
            if cancel.load(Ordering::Relaxed) {
                let _ = log.send("Cancelled".to_string());
                return false;
            }
            let _ = log.send(format!("--- {} ---", source.name));
            for catalog in &source.catalogs {
                if cancel.load(Ordering::Relaxed) {
                    let _ = log.send("Cancelled".to_string());
                    return false;
                }
                let _ = log.send(format!("Refreshing: {}", catalog.path));
                let _ = cache.invalidate(&source.name, catalog);
                let be = match backend::create_backend(source) {
                    Ok(be) => be,
                    Err(e) => {
                        let _ = log.send(format!("ERROR: {}", e));
                        continue;
                    }
                };
                match be.list_all_objects(catalog, &log, &cancel) {
                    Ok(games) => {
                        let _ = log.send(format!("Fetched {} games", games.len()));
                        if let Err(e) = cache.save(&source.name, catalog, &games) {
                            let _ = log.send(format!("Cache save error: {}", e));
                        }
                    }
                    Err(e) => {
                        let _ = log.send(format!("ERROR: {}", e));
                    }
                }
            }
        }
        let _ = log.send("All sources refreshed!".to_string());
        true
    }

    fn fetch_games(
        source: Source,
        catalog: Catalog,
        cache: CatalogCache,
        log: Sender<String>,
        cancel: Arc<AtomicBool>,
    ) -> Option<Vec<RemoteGame>> {
        let _ = log.send(format!("Source: {}", source.name));
        let _ = log.send(format!("Catalog: {} ({})", catalog.path, catalog.platform));

        if !cache.is_stale(&source.name, &catalog) {
            let _ = log.send("Loading from cache...".to_string());
            let games = cache.load(&source.name, &catalog).unwrap_or_default();
            let _ = log.send(format!("Loaded {} games from cache", games.len()));
            return Some(games);
        }

        let _ = log.send("Cache miss, fetching from S3...".to_string());

        let be = match backend::create_backend(&source) {
            Ok(be) => be,
            Err(e) => {
                let _ = log.send(format!("ERROR: {}", e));
                return None;
            }
        };

        match be.list_all_objects(&catalog, &log, &cancel) {
            Ok(games) => {
                if cancel.load(Ordering::Relaxed) {
                    let _ = log.send("Cancelled".to_string());
                    return None;
                }
                let _ = log.send(format!("Fetched {} games", games.len()));
                let _ = log.send("Saving to cache...".to_string());
                if let Err(e) = cache.save(&source.name, &catalog, &games) {
                    let _ = log.send(format!("Cache save error: {}", e));
                }
                let _ = log.send("Done!".to_string());
                Some(games)
            }
            Err(e) => {
                let _ = log.send(format!("ERROR: {}", e));
                None
            }
        }
    }

    fn render_log_textures(&mut self) {
        let text = TextRenderer::new();
        self.rendered_lines.clear();

        let start = if self.log_lines.len() > MAX_LOG_LINES {
            self.log_lines.len() - MAX_LOG_LINES
        } else {
            0
        };

        for line in &self.log_lines[start..] {
            let tex = text.render_text(
                self.texture_creator, line, LOG_FONT_SIZE,
                LOG_COLOR.r, LOG_COLOR.g, LOG_COLOR.b, LOG_COLOR.a,
            );
            let q = tex.query();
            self.rendered_lines.push((tex, q.width, q.height));
        }
    }

    pub fn check_result(&mut self) -> LoadingOutcome {
        // Drain log messages
        while let Ok(msg) = self.log_rx.try_recv() {
            self.log_lines.push(msg);
            self.dirty = true;
        }

        if self.dirty {
            self.render_log_textures();
            self.dirty = false;
        }

        // Check if thread finished
        let finished = self.handle.as_ref().is_some_and(|h| h.is_finished());
        if finished {
            if let Some(handle) = self.handle.take() {
                if self.cancel.load(Ordering::Relaxed) {
                    return LoadingOutcome::Cancelled;
                }
                match handle.join() {
                    Ok(LoadingResult::Single(Some(games))) => {
                        return LoadingOutcome::Done {
                            games,
                            source_idx: self.source_idx,
                        };
                    }
                    Ok(LoadingResult::RefreshAll(true)) => {
                        return LoadingOutcome::RefreshDone;
                    }
                    _ => return LoadingOutcome::Cancelled,
                }
            }
        }

        LoadingOutcome::None
    }

    pub fn handle_input(&mut self, action: InputAction) -> LoadingOutcome {
        if action == InputAction::Back {
            self.cancel.store(true, Ordering::Relaxed);
            return LoadingOutcome::Cancelled;
        }
        LoadingOutcome::None
    }
}

impl<'a> Scene for LoadingScene<'a> {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, _elapsed: u128) {
        // Box
        let box_x = BOX_MARGIN;
        let box_y = BOX_MARGIN;
        let box_w = (WINDOW_WIDTH as i32 - 2 * BOX_MARGIN) as u32;
        let box_h = (MAX_LOG_LINES as i32 * LOG_LINE_HEIGHT + 2 * BOX_PADDING) as u32;
        canvas.set_draw_color(BOX_COLOR);
        canvas.fill_rect(Rect::new(box_x, box_y, box_w, box_h)).unwrap();

        // Log lines
        for (i, (tex, w, h)) in self.rendered_lines.iter().enumerate() {
            let x = box_x + BOX_PADDING;
            let y = box_y + BOX_PADDING + (i as i32 * LOG_LINE_HEIGHT);
            canvas.copy(tex, None, Rect::new(x, y, *w, *h)).unwrap();
        }

        // Legend
        let legend_x = (WINDOW_WIDTH as i32 - self.legend_w as i32) / 2;
        let legend_y = WINDOW_HEIGHT as i32 - self.legend_h as i32 - LEGEND_BOTTOM_MARGIN;
        canvas.copy(&self.legend_texture, None, Rect::new(legend_x, legend_y, self.legend_w, self.legend_h)).unwrap();
    }
}
