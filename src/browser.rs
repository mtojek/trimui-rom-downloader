use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::backend::RemoteGame;
use crate::config::Source;
use crate::download::{DownloadCommand, DownloadManager};
use crate::input::InputAction;
use crate::install_dir::InstallDirResolver;
use crate::library::MyGames;
use crate::scene::{Scene, SceneResult};
use crate::text::TextRenderer;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

const LETTERS: &[char] = &[
    '#', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const ROW1_LEN: usize = 14;

const MAX_VISIBLE: usize = 15;
const LETTER_BAR_FONT_SIZE: f32 = 24.0;
const GAME_FONT_SIZE: f32 = 22.0;
const LEGEND_FONT_SIZE: f32 = 28.0;
const LETTER_BAR_Y: i32 = 30;
const LETTER_ROW_SPACING: i32 = 28;
const GAME_LIST_GAP: i32 = 15;
const GAME_SPACING: i32 = 36;
const GAME_LEFT_MARGIN: i32 = 40;
const SIZE_RIGHT_MARGIN: i32 = 40;
const LEGEND_BOTTOM_MARGIN: i32 = 12;

const NORMAL_COLOR: Color = Color::RGBA(200, 200, 200, 255);
const SELECTED_COLOR: Color = Color::RGBA(255, 220, 80, 255);
const DIM_COLOR: Color = Color::RGBA(100, 100, 100, 255);
const INSTALLED_COLOR: Color = Color::RGBA(100, 200, 100, 255);
const DOWNLOADING_COLOR: Color = Color::RGBA(100, 180, 255, 255);
const FAILED_COLOR: Color = Color::RGBA(255, 80, 80, 255);
const LEGEND_COLOR: Color = Color::RGBA(0, 0, 0, 220);

pub enum BrowserOutcome {
    None,
    Back,
}

struct RenderedGame<'a> {
    name_texture: Texture<'a>,
    name_w: u32,
    name_h: u32,
    size_texture: Texture<'a>,
    size_w: u32,
    size_h: u32,
}

struct GameEntry {
    full_key: String,
    file_name: String,
    game_key: String,
    display_name: String,
    size_str: String,
    file_size: u64,
    installed: bool,
    downloading: bool,
    failed: bool,
}

fn file_stem(file_name: &str) -> &str {
    file_name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(file_name)
}

pub struct GameBrowser<'a> {
    games: Vec<RemoteGame>,
    source: Source,
    platform: String,
    pub source_idx: usize,
    letter_idx: usize,
    selected: usize,
    scroll_offset: usize,
    filtered: Vec<GameEntry>,
    rendered_games: Vec<RenderedGame<'a>>,
    rendered_selected: Vec<Texture<'a>>,
    rendered_offset: usize,
    letter_textures_normal: Vec<Texture<'a>>,
    letter_textures_active: Vec<Texture<'a>>,
    letter_textures_dim: Vec<Texture<'a>>,
    letter_widths: Vec<u32>,
    letter_heights: Vec<u32>,
    letter_has_games: Vec<bool>,
    texture_creator: &'a TextureCreator<WindowContext>,
}

impl<'a> GameBrowser<'a> {
    pub fn new(
        texture_creator: &'a TextureCreator<WindowContext>,
        all_games: Vec<RemoteGame>,
        source: Source,
        platform: String,
        source_idx: usize,
        my_games: &MyGames,
        download_mgr: &DownloadManager,
    ) -> Self {
        let text = TextRenderer::new();

        let mut letter_textures_normal = Vec::new();
        let mut letter_textures_active = Vec::new();
        let mut letter_textures_dim = Vec::new();
        let mut letter_widths = Vec::new();
        let mut letter_heights = Vec::new();
        let mut letter_has_games = Vec::new();

        for &ch in LETTERS {
            let label = format!(" {} ", ch);
            let has = has_games_for_letter(&all_games, ch);
            letter_has_games.push(has);

            let normal = text.render_text(texture_creator, &label, LETTER_BAR_FONT_SIZE,
                NORMAL_COLOR.r, NORMAL_COLOR.g, NORMAL_COLOR.b, NORMAL_COLOR.a);
            let active = text.render_text(texture_creator, &label, LETTER_BAR_FONT_SIZE,
                SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, SELECTED_COLOR.a);
            let dim = text.render_text(texture_creator, &label, LETTER_BAR_FONT_SIZE,
                DIM_COLOR.r, DIM_COLOR.g, DIM_COLOR.b, DIM_COLOR.a);

            let q = normal.query();
            letter_widths.push(q.width);
            letter_heights.push(q.height);
            letter_textures_normal.push(normal);
            letter_textures_active.push(active);
            letter_textures_dim.push(dim);
        }

        let mut browser = GameBrowser {
            games: all_games,
            source,
            platform,
            source_idx,
            letter_idx: letter_has_games.iter().position(|&h| h).unwrap_or(0),
            selected: 0,
            scroll_offset: 0,
            filtered: Vec::new(),
            rendered_games: Vec::new(),
            rendered_selected: Vec::new(),
            rendered_offset: 0,
            letter_textures_normal,
            letter_textures_active,
            letter_textures_dim,
            letter_widths,
            letter_heights,
            letter_has_games,
            texture_creator,
        };
        browser.rebuild_game_list(my_games, download_mgr);
        browser
    }

    fn current_letter(&self) -> char {
        LETTERS[self.letter_idx]
    }

    fn rebuild_game_list(&mut self, my_games: &MyGames, download_mgr: &DownloadManager) {
        let letter = self.current_letter();
        let mut filtered: Vec<(&RemoteGame,)> = self.games.iter().filter(|g| {
            let name = g.key.rsplit('/').next().unwrap_or(&g.key);
            if let Some(first) = name.chars().next() {
                if letter == '#' { !first.is_ascii_alphabetic() } else { first.to_ascii_uppercase() == letter }
            } else { false }
        }).map(|g| (g,)).collect();
        filtered.sort_by(|a, b| {
            let na = a.0.key.rsplit('/').next().unwrap_or(&a.0.key);
            let nb = b.0.key.rsplit('/').next().unwrap_or(&b.0.key);
            na.to_ascii_lowercase().cmp(&nb.to_ascii_lowercase())
        });

        self.filtered = filtered.into_iter().map(|(g,)| {
            let file_name = g.key.rsplit('/').next().unwrap_or(&g.key).to_string();
            let game_key = file_stem(&file_name).to_string();
            let installed = my_games.is_installed(&self.source.name, &self.platform, &game_key);
            let downloading = download_mgr.is_queued_or_active(&self.source.name, &self.platform, &game_key);
            let failed = download_mgr.is_failed(&self.source.name, &self.platform, &game_key);
            let display = truncate_name(&file_name, 68);
            GameEntry {
                full_key: g.key.clone(),
                file_name,
                game_key,
                display_name: display,
                size_str: format_size(g.file_size),
                file_size: g.file_size,
                installed,
                downloading,
                failed,
            }
        }).collect();

        self.selected = 0;
        self.scroll_offset = 0;
        self.render_visible();
    }

    pub fn refresh_statuses(&mut self, my_games: &MyGames, download_mgr: &DownloadManager) {
        let mut changed = false;
        for entry in &mut self.filtered {
            let installed = my_games.is_installed(&self.source.name, &self.platform, &entry.game_key);
            let downloading = download_mgr.is_queued_or_active(&self.source.name, &self.platform, &entry.game_key);
            let failed = download_mgr.is_failed(&self.source.name, &self.platform, &entry.game_key);
            if installed != entry.installed || downloading != entry.downloading || failed != entry.failed {
                entry.installed = installed;
                entry.downloading = downloading;
                entry.failed = failed;
                entry.display_name = truncate_name(&entry.file_name, 68);
                changed = true;
            }
        }
        if changed {
            self.render_visible();
        }
    }

    fn render_visible(&mut self) {
        let end = (self.scroll_offset + MAX_VISIBLE).min(self.filtered.len());
        let text = TextRenderer::new();
        self.rendered_games.clear();
        self.rendered_selected.clear();
        self.rendered_offset = self.scroll_offset;

        for entry in &self.filtered[self.scroll_offset..end] {
            let name_color = if entry.failed {
                FAILED_COLOR
            } else if entry.installed {
                INSTALLED_COLOR
            } else if entry.downloading {
                DOWNLOADING_COLOR
            } else {
                NORMAL_COLOR
            };
            let name_tex = text.render_text(self.texture_creator, &entry.display_name, GAME_FONT_SIZE,
                name_color.r, name_color.g, name_color.b, name_color.a);
            let name_sel = text.render_text(self.texture_creator, &entry.display_name, GAME_FONT_SIZE,
                SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, SELECTED_COLOR.a);
            let size_tex = text.render_text(self.texture_creator, &entry.size_str, GAME_FONT_SIZE,
                NORMAL_COLOR.r, NORMAL_COLOR.g, NORMAL_COLOR.b, NORMAL_COLOR.a);

            let nq = name_tex.query();
            let sq = size_tex.query();

            self.rendered_games.push(RenderedGame {
                name_texture: name_tex,
                name_w: nq.width,
                name_h: nq.height,
                size_texture: size_tex,
                size_w: sq.width,
                size_h: sq.height,
            });
            self.rendered_selected.push(name_sel);
        }
    }

    pub fn handle_input(
        &mut self,
        action: InputAction,
        my_games: &mut MyGames,
        download_mgr: &DownloadManager,
        install_resolver: &InstallDirResolver,
    ) -> BrowserOutcome {
        match action {
            InputAction::Left => {
                if let Some(idx) = (0..self.letter_idx).rev().find(|&i| self.letter_has_games[i]) {
                    self.letter_idx = idx;
                    self.rebuild_game_list(my_games, download_mgr);
                }
                BrowserOutcome::None
            }
            InputAction::Right => {
                if let Some(idx) = (self.letter_idx + 1..LETTERS.len()).find(|&i| self.letter_has_games[i]) {
                    self.letter_idx = idx;
                    self.rebuild_game_list(my_games, download_mgr);
                }
                BrowserOutcome::None
            }
            InputAction::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    if self.selected < self.scroll_offset {
                        self.scroll_offset = self.selected;
                        self.render_visible();
                    }
                }
                BrowserOutcome::None
            }
            InputAction::Down => {
                if self.selected + 1 < self.filtered.len() {
                    self.selected += 1;
                    if self.selected >= self.scroll_offset + MAX_VISIBLE {
                        self.scroll_offset = self.selected - MAX_VISIBLE + 1;
                        self.render_visible();
                    }
                }
                BrowserOutcome::None
            }
            InputAction::Action => {
                if let Some(entry) = self.filtered.get(self.selected) {
                    if entry.installed || entry.downloading || entry.failed {
                        // Already installed/downloading/failed — no action
                    } else {
                        // Enqueue download — dest is platform_dir/game_key/file_name
                        let game_dir = install_resolver
                            .game_dir(&self.platform, &entry.game_key)
                            .unwrap_or_else(|| {
                                std::path::PathBuf::from("/mnt/SDCARD/Roms")
                                    .join(&self.platform)
                                    .join(&entry.game_key)
                            });
                        let dest = game_dir.join(&entry.file_name);
                        download_mgr.send_command(DownloadCommand::Enqueue {
                            source: self.source.clone(),
                            platform: self.platform.clone(),
                            key: entry.full_key.clone(),
                            file_name: entry.file_name.clone(),
                            game_key: entry.game_key.clone(),
                            dest_path: dest,
                            total_bytes: entry.file_size,
                        });
                        self.refresh_statuses(my_games, download_mgr);
                    }
                }
                BrowserOutcome::None
            }
            InputAction::Back => BrowserOutcome::Back,
            _ => BrowserOutcome::None,
        }
    }
}

impl<'a> Scene for GameBrowser<'a> {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, _elapsed: u128) {
        // Letter bar background
        let row1_width: u32 = self.letter_widths[..ROW1_LEN].iter().sum();
        let row2_width: u32 = self.letter_widths[ROW1_LEN..].iter().sum();
        let max_row_width = row1_width.max(row2_width);
        let letter_bar_height = LETTER_ROW_SPACING + self.letter_heights[0] as i32 + 12;
        let letter_bg_x = (WINDOW_WIDTH as i32 - max_row_width as i32) / 2 - 8;
        let letter_bg = Rect::new(letter_bg_x, LETTER_BAR_Y - 6, max_row_width + 16, letter_bar_height as u32);
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 200));
        canvas.fill_rect(letter_bg).unwrap();

        // Letter bar — row 1
        let mut lx = (WINDOW_WIDTH as i32 - row1_width as i32) / 2;
        for i in 0..ROW1_LEN {
            let tex = if i == self.letter_idx {
                &self.letter_textures_active[i]
            } else if self.letter_has_games[i] {
                &self.letter_textures_normal[i]
            } else {
                &self.letter_textures_dim[i]
            };
            canvas.copy(tex, None, Rect::new(lx, LETTER_BAR_Y, self.letter_widths[i], self.letter_heights[i])).unwrap();
            lx += self.letter_widths[i] as i32;
        }

        // Letter bar — row 2
        let mut lx = (WINDOW_WIDTH as i32 - row2_width as i32) / 2;
        let row2_y = LETTER_BAR_Y + LETTER_ROW_SPACING;
        for i in ROW1_LEN..LETTERS.len() {
            let tex = if i == self.letter_idx {
                &self.letter_textures_active[i]
            } else if self.letter_has_games[i] {
                &self.letter_textures_normal[i]
            } else {
                &self.letter_textures_dim[i]
            };
            canvas.copy(tex, None, Rect::new(lx, row2_y, self.letter_widths[i], self.letter_heights[i])).unwrap();
            lx += self.letter_widths[i] as i32;
        }

        // Game list background
        let letter_bar_bottom = LETTER_BAR_Y - 6 + letter_bar_height;
        let game_list_top = letter_bar_bottom + GAME_LIST_GAP;
        {
            let list_height = MAX_VISIBLE as i32 * GAME_SPACING + 12;
            let bg_rect = Rect::new(
                GAME_LEFT_MARGIN - 10,
                game_list_top,
                (WINDOW_WIDTH as i32 - 2 * (GAME_LEFT_MARGIN - 10)) as u32,
                list_height as u32,
            );
            canvas.set_draw_color(Color::RGBA(0, 0, 0, 200));
            canvas.fill_rect(bg_rect).unwrap();
        }

        // Game list
        for (vi, game) in self.rendered_games.iter().enumerate() {
            let gi = self.rendered_offset + vi;
            let y = game_list_top + 6 + (vi as i32 * GAME_SPACING);

            let name_tex = if gi == self.selected {
                &self.rendered_selected[vi]
            } else {
                &game.name_texture
            };

            canvas.copy(name_tex, None, Rect::new(GAME_LEFT_MARGIN, y, game.name_w, game.name_h)).unwrap();

            let size_x = WINDOW_WIDTH as i32 - game.size_w as i32 - SIZE_RIGHT_MARGIN;
            canvas.copy(&game.size_texture, None, Rect::new(size_x, y, game.size_w, game.size_h)).unwrap();
        }

        // Legend — contextual based on selected entry
        let legend_str = match self.filtered.get(self.selected) {
            Some(entry) if entry.installed || entry.downloading || entry.failed => {
                "Menu: Exit    L1/R1: Letter    B: Back"
            }
            _ => "Menu: Exit    L1/R1: Letter    X: Download    B: Back",
        };
        let text_r = TextRenderer::new();
        let legend_tex = text_r.render_text(
            self.texture_creator, legend_str, LEGEND_FONT_SIZE,
            LEGEND_COLOR.r, LEGEND_COLOR.g, LEGEND_COLOR.b, LEGEND_COLOR.a,
        );
        let lq = legend_tex.query();
        let legend_x = (WINDOW_WIDTH as i32 - lq.width as i32) / 2;
        let legend_y = WINDOW_HEIGHT as i32 - lq.height as i32 - LEGEND_BOTTOM_MARGIN;
        canvas.copy(&legend_tex, None, Rect::new(legend_x, legend_y, lq.width, lq.height)).unwrap();
    }
}

fn has_games_for_letter(games: &[RemoteGame], letter: char) -> bool {
    games.iter().any(|g| {
        let name = g.key.rsplit('/').next().unwrap_or(&g.key);
        if let Some(first) = name.chars().next() {
            if letter == '#' {
                !first.is_ascii_alphabetic()
            } else {
                first.to_ascii_uppercase() == letter
            }
        } else {
            false
        }
    })
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        return name.to_string();
    }
    let ext = match name.rfind('.') {
        Some(pos) => &name[pos..],
        None => "",
    };
    let avail = max_len.saturating_sub(ext.len() + 3);
    format!("{}...{}", &name[..avail], ext)
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.0} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}
