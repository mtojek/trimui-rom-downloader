use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::download::{DownloadCommand, DownloadEntry, DownloadManager, DownloadState};
use crate::input::InputAction;
use crate::install_dir::InstallDirResolver;
use crate::library::{GameEntry, MyGames};
use crate::scene::{Scene, SceneResult};
use crate::text::TextRenderer;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

const MAX_VISIBLE: usize = 15;
const FONT_SIZE: f32 = 22.0;
const TITLE_FONT_SIZE: f32 = 28.0;
const LEGEND_FONT_SIZE: f32 = 28.0;
const ROW_HEIGHT: i32 = 36;
const LEFT_MARGIN: i32 = 40;
const RIGHT_MARGIN: i32 = 40;
const TOP_Y: i32 = 70;
const PROGRESS_BAR_HEIGHT: i32 = 8;
const PROGRESS_BAR_Y_OFFSET: i32 = 30;
const BOX_PADDING_X: i32 = 20;
const BOX_PADDING_Y: i32 = 12;

const NORMAL_COLOR: Color = Color::RGBA(200, 200, 200, 255);
const SELECTED_COLOR: Color = Color::RGBA(255, 220, 80, 255);
const ACTIVE_COLOR: Color = Color::RGBA(100, 180, 255, 255);
const PAUSED_COLOR: Color = Color::RGBA(255, 180, 50, 255);
const FAILED_COLOR: Color = Color::RGBA(255, 80, 80, 255);
const UNPACKING_COLOR: Color = Color::RGBA(200, 140, 255, 255);
const QUEUED_COLOR: Color = Color::RGBA(160, 160, 160, 255);
const INSTALLED_COLOR: Color = Color::RGBA(100, 200, 100, 255);
const SEPARATOR_COLOR: Color = Color::RGBA(120, 120, 120, 180);
const BAR_BG_COLOR: Color = Color::RGBA(60, 60, 60, 200);
const BAR_FG_COLOR: Color = Color::RGBA(100, 180, 255, 255);
const LEGEND_COLOR: Color = Color::RGBA(0, 0, 0, 220);
const BOX_COLOR: Color = Color::RGBA(0, 0, 0, 200);

pub enum MyGamesOutcome {
    None,
    Back,
}

enum Row {
    Download(DownloadEntry),
    Separator,
    Installed(GameEntry),
}

struct RenderedRow<'a> {
    left_texture: Texture<'a>,
    left_w: u32,
    left_h: u32,
    right_texture: Texture<'a>,
    right_w: u32,
    right_h: u32,
    selected_texture: Texture<'a>,
    sel_w: u32,
    sel_h: u32,
}

pub struct MyGamesScene<'a> {
    rows: Vec<Row>,
    rendered: Vec<Option<RenderedRow<'a>>>,
    selected: usize,
    scroll_offset: usize,
    texture_creator: &'a TextureCreator<WindowContext>,
    title_texture: Texture<'a>,
    title_w: u32,
    title_h: u32,
    confirm_delete: bool,
    confirm_selected: usize, // 0=No, 1=Yes
}

impl<'a> MyGamesScene<'a> {
    pub fn new(
        texture_creator: &'a TextureCreator<WindowContext>,
        my_games: &MyGames,
        download_mgr: &DownloadManager,
    ) -> Self {
        let text = TextRenderer::new();
        let title_texture = text.render_text(
            texture_creator, "My Games", TITLE_FONT_SIZE,
            SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, SELECTED_COLOR.a,
        );
        let tq = title_texture.query();

        let mut scene = MyGamesScene {
            rows: Vec::new(),
            rendered: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            texture_creator,
            title_texture,
            title_w: tq.width,
            title_h: tq.height,
            confirm_delete: false,
            confirm_selected: 0,
        };
        scene.rebuild(my_games, download_mgr);
        scene
    }

    pub fn refresh(&mut self, my_games: &MyGames, download_mgr: &DownloadManager) {
        let old_selected = self.selected;
        self.rebuild(my_games, download_mgr);
        if old_selected < self.rows.len() {
            self.selected = old_selected;
        } else if !self.rows.is_empty() {
            self.selected = self.rows.len() - 1;
        }
        self.clamp_scroll();
    }

    fn rebuild(&mut self, my_games: &MyGames, download_mgr: &DownloadManager) {
        let statuses = download_mgr.statuses();
        let text = TextRenderer::new();

        self.rows.clear();
        self.rendered.clear();

        let mut downloads: Vec<&DownloadEntry> = statuses.iter()
            .filter(|e| matches!(e.state, DownloadState::Active | DownloadState::Queued | DownloadState::Paused | DownloadState::Unpacking | DownloadState::Failed))
            .collect();
        downloads.sort_by(|a, b| {
            let order = |s: &DownloadState| match s {
                DownloadState::Active => 0,
                DownloadState::Unpacking => 1,
                DownloadState::Queued => 2,
                DownloadState::Paused => 3,
                DownloadState::Failed => 4,
                _ => 5,
            };
            order(&a.state).cmp(&order(&b.state))
                .then_with(|| a.file_name.to_lowercase().cmp(&b.file_name.to_lowercase()))
        });

        let has_downloads = !downloads.is_empty();
        let installed = my_games.list();
        let has_installed = !installed.is_empty();

        for dl in &downloads {
            let rendered = self.render_download_row(&text, dl);
            self.rows.push(Row::Download((*dl).clone()));
            self.rendered.push(Some(rendered));
        }

        if has_downloads && has_installed {
            self.rows.push(Row::Separator);
            self.rendered.push(None);
        }

        let mut installed_sorted: Vec<&GameEntry> = installed.iter().collect();
        installed_sorted.sort_by(|a, b| a.key.to_lowercase().cmp(&b.key.to_lowercase()));

        for game in installed_sorted {
            let rendered = self.render_installed_row(&text, game);
            self.rows.push(Row::Installed(game.clone()));
            self.rendered.push(Some(rendered));
        }

        if !self.rows.is_empty() && matches!(self.rows.get(self.selected), Some(Row::Separator)) {
            if self.selected + 1 < self.rows.len() {
                self.selected += 1;
            } else if self.selected > 0 {
                self.selected -= 1;
            }
        }
    }

    fn render_download_row(&self, text: &TextRenderer, dl: &DownloadEntry) -> RenderedRow<'a> {
        let (color, state_str) = match dl.state {
            DownloadState::Active => {
                let pct = if dl.total_bytes > 0 {
                    (dl.downloaded_bytes as f64 / dl.total_bytes as f64 * 100.0) as u32
                } else { 0 };
                (ACTIVE_COLOR, format!("{}%  {}/s", pct, format_bytes(dl.speed as u64)))
            }
            DownloadState::Unpacking => {
                let pct = if dl.total_bytes > 0 {
                    (dl.downloaded_bytes as f64 / dl.total_bytes as f64 * 100.0) as u32
                } else { 0 };
                (UNPACKING_COLOR, format!("{}% Unpacking", pct))
            }
            DownloadState::Queued => (QUEUED_COLOR, "Queued".to_string()),
            DownloadState::Paused => {
                let pct = if dl.total_bytes > 0 {
                    (dl.downloaded_bytes as f64 / dl.total_bytes as f64 * 100.0) as u32
                } else { 0 };
                (PAUSED_COLOR, format!("{}% Paused", pct))
            }
            DownloadState::Failed => (FAILED_COLOR, "Failed".to_string()),
            _ => (NORMAL_COLOR, String::new()),
        };

        let name = truncate(&dl.file_name, 45);
        let right = format!("{}  {}", format_bytes(dl.total_bytes), state_str);

        let left = text.render_text(self.texture_creator, &name, FONT_SIZE, color.r, color.g, color.b, color.a);
        let lq = left.query();
        let right_tex = text.render_text(self.texture_creator, &right, FONT_SIZE, color.r, color.g, color.b, color.a);
        let rq = right_tex.query();
        let selected = text.render_text(self.texture_creator, &name, FONT_SIZE, SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, SELECTED_COLOR.a);
        let sq = selected.query();

        RenderedRow {
            left_texture: left, left_w: lq.width, left_h: lq.height,
            right_texture: right_tex, right_w: rq.width, right_h: rq.height,
            selected_texture: selected, sel_w: sq.width, sel_h: sq.height,
        }
    }

    fn render_installed_row(&self, text: &TextRenderer, game: &GameEntry) -> RenderedRow<'a> {
        let name = truncate(&game.key, 50);
        let right = platform_display(&game.platform);

        let left = text.render_text(self.texture_creator, &name, FONT_SIZE, INSTALLED_COLOR.r, INSTALLED_COLOR.g, INSTALLED_COLOR.b, INSTALLED_COLOR.a);
        let lq = left.query();
        let right_tex = text.render_text(self.texture_creator, &right, FONT_SIZE, INSTALLED_COLOR.r, INSTALLED_COLOR.g, INSTALLED_COLOR.b, INSTALLED_COLOR.a);
        let rq = right_tex.query();
        let selected = text.render_text(self.texture_creator, &name, FONT_SIZE, SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, SELECTED_COLOR.a);
        let sq = selected.query();

        RenderedRow {
            left_texture: left, left_w: lq.width, left_h: lq.height,
            right_texture: right_tex, right_w: rq.width, right_h: rq.height,
            selected_texture: selected, sel_w: sq.width, sel_h: sq.height,
        }
    }

    fn clamp_scroll(&mut self) {
        if self.rows.is_empty() {
            self.scroll_offset = 0;
            self.selected = 0;
            return;
        }
        if self.selected >= self.rows.len() {
            self.selected = self.rows.len() - 1;
        }
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        if self.selected >= self.scroll_offset + MAX_VISIBLE {
            self.scroll_offset = self.selected + 1 - MAX_VISIBLE;
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() { return; }
        let new = (self.selected as i32 + delta).clamp(0, self.rows.len() as i32 - 1) as usize;
        self.selected = new;
        if matches!(self.rows.get(self.selected), Some(Row::Separator)) {
            let next = (self.selected as i32 + delta.signum()).clamp(0, self.rows.len() as i32 - 1) as usize;
            if !matches!(self.rows.get(next), Some(Row::Separator)) {
                self.selected = next;
            }
        }
        self.clamp_scroll();
    }

    pub fn handle_input(
        &mut self,
        action: InputAction,
        my_games: &mut MyGames,
        download_mgr: &DownloadManager,
        install_resolver: &InstallDirResolver,
    ) -> MyGamesOutcome {
        if self.confirm_delete {
            return self.handle_delete_confirm(action, my_games, download_mgr, install_resolver);
        }

        match action {
            InputAction::Up => { self.move_selection(-1); }
            InputAction::Down => { self.move_selection(1); }
            InputAction::Left => { self.move_selection(-(MAX_VISIBLE as i32)); }
            InputAction::Right => { self.move_selection(MAX_VISIBLE as i32); }
            InputAction::Back => { return MyGamesOutcome::Back; }
            InputAction::Action => {
                // X = Pause/Resume/Retry for downloads
                if let Some(Row::Download(dl)) = self.rows.get(self.selected) {
                    match dl.state {
                        DownloadState::Active => {
                            download_mgr.send_command(DownloadCommand::Pause(dl.id));
                        }
                        DownloadState::Paused | DownloadState::Failed => {
                            download_mgr.send_command(DownloadCommand::Resume(dl.id));
                        }
                        _ => {}
                    }
                }
            }
            InputAction::Refresh => {
                // Y = Delete (show confirmation)
                if self.selected < self.rows.len() && !matches!(self.rows[self.selected], Row::Separator) {
                    self.confirm_delete = true;
                    self.confirm_selected = 0; // default to No
                }
            }
            _ => {}
        }
        MyGamesOutcome::None
    }

    fn handle_delete_confirm(
        &mut self,
        action: InputAction,
        my_games: &mut MyGames,
        download_mgr: &DownloadManager,
        install_resolver: &InstallDirResolver,
    ) -> MyGamesOutcome {
        match action {
            InputAction::Up | InputAction::Down => {
                self.confirm_selected = 1 - self.confirm_selected;
            }
            InputAction::Confirm => {
                self.confirm_delete = false;
                if self.confirm_selected == 1 {
                    // Yes — delete
                    if self.selected < self.rows.len() {
                        match &self.rows[self.selected] {
                            Row::Download(dl) => {
                                download_mgr.send_command(DownloadCommand::Cancel(dl.id));
                            }
                            Row::Installed(game) => {
                                if let Some(game_dir) = install_resolver.game_dir(&game.platform, &game.key) {
                                    if game_dir.exists() && game_dir.is_dir() {
                                        let _ = std::fs::remove_dir_all(&game_dir);
                                        eprintln!("[MyGames] Deleted directory: {}", game_dir.display());
                                    }
                                }
                                // Also remove flat files (non-subdirectory installs)
                                if let Some(platform_dir) = install_resolver.resolve(&game.platform) {
                                    if let Ok(entries) = std::fs::read_dir(platform_dir) {
                                        for entry in entries.flatten() {
                                            let name = entry.file_name().to_string_lossy().to_string();
                                            if let Some(stem) = name.rsplit_once('.').map(|(s, _)| s) {
                                                if stem == game.key && entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                                                    let path = entry.path();
                                                    let _ = std::fs::remove_file(&path);
                                                    eprintln!("[MyGames] Deleted file: {}", path.display());
                                                }
                                            }
                                        }
                                    }
                                }
                                let _ = my_games.remove(&game.source, &game.platform, &game.key);
                            }
                            Row::Separator => {}
                        }
                        self.refresh(my_games, download_mgr);
                    }
                }
            }
            InputAction::Back => {
                self.confirm_delete = false;
            }
            _ => {}
        }
        MyGamesOutcome::None
    }
}

impl<'a> Scene for MyGamesScene<'a> {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, _elapsed: u128) {
        let list_top = TOP_Y + self.title_h as i32 + 15;
        let content_height = MAX_VISIBLE as i32 * ROW_HEIGHT;

        // Dark background box
        let box_x = LEFT_MARGIN - BOX_PADDING_X;
        let box_y = TOP_Y - BOX_PADDING_Y;
        let box_w = (WINDOW_WIDTH as i32 - 2 * (LEFT_MARGIN - BOX_PADDING_X)) as u32;
        let box_h = (self.title_h as i32 + 15 + content_height + 2 * BOX_PADDING_Y) as u32;
        canvas.set_draw_color(BOX_COLOR);
        canvas.fill_rect(Rect::new(box_x, box_y, box_w, box_h)).unwrap();

        // Title
        let title_x = (WINDOW_WIDTH as i32 - self.title_w as i32) / 2;
        canvas.copy(&self.title_texture, None, Rect::new(title_x, TOP_Y, self.title_w, self.title_h)).unwrap();

        if self.rows.is_empty() {
            let text = TextRenderer::new();
            let empty = text.render_text(
                self.texture_creator, "No games yet", FONT_SIZE,
                NORMAL_COLOR.r, NORMAL_COLOR.g, NORMAL_COLOR.b, NORMAL_COLOR.a,
            );
            let eq = empty.query();
            let x = (WINDOW_WIDTH as i32 - eq.width as i32) / 2;
            canvas.copy(&empty, None, Rect::new(x, list_top + 20, eq.width, eq.height)).unwrap();
        } else {
            let end = (self.scroll_offset + MAX_VISIBLE).min(self.rows.len());
            for (vi, ri) in (self.scroll_offset..end).enumerate() {
                let y = list_top + vi as i32 * ROW_HEIGHT;
                let is_selected = ri == self.selected;

                match &self.rows[ri] {
                    Row::Separator => {
                        let sep_y = y + ROW_HEIGHT / 2;
                        let bar_w = WINDOW_WIDTH - LEFT_MARGIN as u32 - RIGHT_MARGIN as u32;
                        canvas.set_draw_color(SEPARATOR_COLOR);
                        canvas.fill_rect(Rect::new(LEFT_MARGIN, sep_y, bar_w, 2)).unwrap();
                    }
                    Row::Download(dl) => {
                        if let Some(Some(rendered)) = self.rendered.get(ri) {
                            let (tex, w, h) = if is_selected {
                                (&rendered.selected_texture, rendered.sel_w, rendered.sel_h)
                            } else {
                                (&rendered.left_texture, rendered.left_w, rendered.left_h)
                            };
                            canvas.copy(tex, None, Rect::new(LEFT_MARGIN, y, w, h)).unwrap();

                            let rx = WINDOW_WIDTH as i32 - RIGHT_MARGIN - rendered.right_w as i32;
                            canvas.copy(&rendered.right_texture, None, Rect::new(rx, y, rendered.right_w, rendered.right_h)).unwrap();

                            if matches!(dl.state, DownloadState::Active | DownloadState::Paused | DownloadState::Unpacking) && dl.total_bytes > 0 {
                                let bar_y = y + PROGRESS_BAR_Y_OFFSET;
                                let bar_w = (WINDOW_WIDTH - LEFT_MARGIN as u32 - RIGHT_MARGIN as u32) as i32;
                                let pct = dl.downloaded_bytes as f64 / dl.total_bytes as f64;
                                let filled = (bar_w as f64 * pct) as i32;

                                canvas.set_draw_color(BAR_BG_COLOR);
                                canvas.fill_rect(Rect::new(LEFT_MARGIN, bar_y, bar_w as u32, PROGRESS_BAR_HEIGHT as u32)).unwrap();
                                if filled > 0 {
                                    let bar_color = match dl.state {
                                        DownloadState::Paused => PAUSED_COLOR,
                                        DownloadState::Unpacking => UNPACKING_COLOR,
                                        _ => BAR_FG_COLOR,
                                    };
                                    canvas.set_draw_color(bar_color);
                                    canvas.fill_rect(Rect::new(LEFT_MARGIN, bar_y, filled as u32, PROGRESS_BAR_HEIGHT as u32)).unwrap();
                                }
                            }
                        }
                    }
                    Row::Installed(_) => {
                        if let Some(Some(rendered)) = self.rendered.get(ri) {
                            let (tex, w, h) = if is_selected {
                                (&rendered.selected_texture, rendered.sel_w, rendered.sel_h)
                            } else {
                                (&rendered.left_texture, rendered.left_w, rendered.left_h)
                            };
                            canvas.copy(tex, None, Rect::new(LEFT_MARGIN, y, w, h)).unwrap();

                            let rx = WINDOW_WIDTH as i32 - RIGHT_MARGIN - rendered.right_w as i32;
                            canvas.copy(&rendered.right_texture, None, Rect::new(rx, y, rendered.right_w, rendered.right_h)).unwrap();
                        }
                    }
                }
            }
        }

        // Legend — contextual based on selected row
        let legend_str = match self.rows.get(self.selected) {
            Some(Row::Download(dl)) => match dl.state {
                DownloadState::Active | DownloadState::Queued => "B: Back    X: Pause    Y: Delete",
                DownloadState::Paused => "B: Back    X: Resume    Y: Delete",
                DownloadState::Failed => "B: Back    X: Retry    Y: Delete",
                DownloadState::Unpacking => "B: Back",
                _ => "B: Back    Y: Delete",
            },
            Some(Row::Installed(_)) => "B: Back    Y: Delete",
            _ => "B: Back",
        };
        let text_r = TextRenderer::new();
        let legend_texture = text_r.render_text(
            self.texture_creator, legend_str, LEGEND_FONT_SIZE,
            LEGEND_COLOR.r, LEGEND_COLOR.g, LEGEND_COLOR.b, LEGEND_COLOR.a,
        );
        let lq = legend_texture.query();
        let legend_y = WINDOW_HEIGHT as i32 - lq.height as i32 - 12;
        let legend_x = (WINDOW_WIDTH as i32 - lq.width as i32) / 2;
        canvas.copy(&legend_texture, None, Rect::new(legend_x, legend_y, lq.width, lq.height)).unwrap();

        // Delete confirmation overlay
        if self.confirm_delete {
            canvas.set_draw_color(Color::RGBA(0, 0, 0, 180));
            canvas.fill_rect(Rect::new(0, 0, WINDOW_WIDTH, WINDOW_HEIGHT)).unwrap();

            let text = TextRenderer::new();
            let title = text.render_text(self.texture_creator, "Delete?", 36.0, NORMAL_COLOR.r, NORMAL_COLOR.g, NORMAL_COLOR.b, NORMAL_COLOR.a);
            let tq = title.query();

            let no_color = if self.confirm_selected == 0 { SELECTED_COLOR } else { NORMAL_COLOR };
            let yes_color = if self.confirm_selected == 1 { SELECTED_COLOR } else { NORMAL_COLOR };

            let no_tex = text.render_text(self.texture_creator, "No", 36.0, no_color.r, no_color.g, no_color.b, no_color.a);
            let nq = no_tex.query();
            let yes_tex = text.render_text(self.texture_creator, "Yes", 36.0, yes_color.r, yes_color.g, yes_color.b, yes_color.a);
            let yq = yes_tex.query();

            let spacing = 50i32;
            let total_h = tq.height as i32 + spacing + nq.height as i32 + spacing + yq.height as i32;
            let start_y = (WINDOW_HEIGHT as i32 - total_h) / 2;

            let max_w = tq.width.max(nq.width).max(yq.width) as i32;
            let dialog_w = (max_w + 80) as u32;
            let dialog_h = (total_h + 40) as u32;
            let dialog_x = (WINDOW_WIDTH as i32 - dialog_w as i32) / 2;
            let dialog_y = start_y - 20;

            canvas.set_draw_color(Color::RGBA(0, 0, 0, 230));
            canvas.fill_rect(Rect::new(dialog_x, dialog_y, dialog_w, dialog_h)).unwrap();

            let tx = (WINDOW_WIDTH as i32 - tq.width as i32) / 2;
            canvas.copy(&title, None, Rect::new(tx, start_y, tq.width, tq.height)).unwrap();

            let ny = start_y + tq.height as i32 + spacing;
            let nx = (WINDOW_WIDTH as i32 - nq.width as i32) / 2;
            canvas.copy(&no_tex, None, Rect::new(nx, ny, nq.width, nq.height)).unwrap();

            let yy = ny + nq.height as i32 + spacing;
            let yx = (WINDOW_WIDTH as i32 - yq.width as i32) / 2;
            canvas.copy(&yes_tex, None, Rect::new(yx, yy, yq.width, yq.height)).unwrap();
        }
    }
}

fn format_bytes(bytes: u64) -> String {
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

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn platform_display(code: &str) -> String {
    match code {
        "PS" => "PlayStation".to_string(),
        "GBA" => "Game Boy Advance".to_string(),
        "GBC" => "Game Boy Color".to_string(),
        "GB" => "Game Boy".to_string(),
        "FC" => "Famicom/NES".to_string(),
        "SFC" => "Super Famicom/SNES".to_string(),
        "MD" => "Mega Drive/Genesis".to_string(),
        "N64" => "Nintendo 64".to_string(),
        "NDS" => "Nintendo DS".to_string(),
        "PSP" => "PlayStation Portable".to_string(),
        "DC" => "Dreamcast".to_string(),
        "SS" => "Sega Saturn".to_string(),
        "PCE" => "PC Engine".to_string(),
        "MAME" => "Arcade (MAME)".to_string(),
        other => other.to_string(),
    }
}
