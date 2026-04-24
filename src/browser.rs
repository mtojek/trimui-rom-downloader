use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::backend::RemoteGame;
use crate::input::InputAction;
use crate::scene::{Scene, SceneResult};
use crate::text::TextRenderer;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

const LETTERS: &[char] = &[
    '#', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const ROW1_LEN: usize = 14; // # A B C D E F G H I J K L M

const MAX_VISIBLE: usize = 10;
const LETTER_BAR_FONT_SIZE: f32 = 28.0;
const GAME_FONT_SIZE: f32 = 30.0;
const LEGEND_FONT_SIZE: f32 = 36.0;
const LETTER_BAR_Y: i32 = 15;
const LETTER_ROW_SPACING: i32 = 32;
const GAME_LIST_TOP: i32 = 90;
const GAME_SPACING: i32 = 50;
const GAME_LEFT_MARGIN: i32 = 40;
const SIZE_RIGHT_MARGIN: i32 = 40;
const LEGEND_BOTTOM_MARGIN: i32 = 12;

const NORMAL_COLOR: Color = Color::RGBA(200, 200, 200, 255);
const SELECTED_COLOR: Color = Color::RGBA(255, 220, 80, 255);
const DIM_COLOR: Color = Color::RGBA(100, 100, 100, 255);
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

pub struct GameBrowser<'a> {
    games: Vec<RemoteGame>,
    #[allow(dead_code)]
    platform: String,
    letter_idx: usize,
    selected: usize,
    scroll_offset: usize,
    rendered_games: Vec<RenderedGame<'a>>,
    rendered_selected: Vec<Texture<'a>>,
    letter_textures_normal: Vec<Texture<'a>>,
    letter_textures_active: Vec<Texture<'a>>,
    letter_textures_dim: Vec<Texture<'a>>,
    letter_widths: Vec<u32>,
    letter_heights: Vec<u32>,
    letter_has_games: Vec<bool>,
    legend_texture: Texture<'a>,
    legend_w: u32,
    legend_h: u32,
    texture_creator: &'a TextureCreator<WindowContext>,
}

impl<'a> GameBrowser<'a> {
    pub fn new(
        texture_creator: &'a TextureCreator<WindowContext>,
        all_games: Vec<RemoteGame>,
        platform: String,
    ) -> Self {
        let text = TextRenderer::new();

        // Pre-render letter bar
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

        let legend = text.render_text(texture_creator,
            "Menu: Exit       L1/R1: Letter       B: Back",
            LEGEND_FONT_SIZE,
            LEGEND_COLOR.r, LEGEND_COLOR.g, LEGEND_COLOR.b, LEGEND_COLOR.a);
        let lq = legend.query();

        let mut browser = GameBrowser {
            games: all_games,
            platform,
            letter_idx: 0,
            selected: 0,
            scroll_offset: 0,
            rendered_games: Vec::new(),
            rendered_selected: Vec::new(),
            letter_textures_normal,
            letter_textures_active,
            letter_textures_dim,
            letter_widths,
            letter_heights,
            letter_has_games,
            legend_texture: legend,
            legend_w: lq.width,
            legend_h: lq.height,
            texture_creator,
        };
        browser.rebuild_game_list();
        browser
    }

    fn current_letter(&self) -> char {
        LETTERS[self.letter_idx]
    }

    fn rebuild_game_list(&mut self) {
        let text = TextRenderer::new();
        let letter = self.current_letter();
        let filtered: Vec<(String, u64)> = self.games.iter().filter(|g| {
            let name = g.key.rsplit('/').next().unwrap_or(&g.key);
            if let Some(first) = name.chars().next() {
                if letter == '#' { !first.is_ascii_alphabetic() } else { first.to_ascii_uppercase() == letter }
            } else { false }
        }).map(|g| {
            let name = g.key.rsplit('/').next().unwrap_or(&g.key).to_string();
            (name, g.file_size)
        }).collect();

        self.rendered_games.clear();
        self.rendered_selected.clear();

        for (name, file_size) in &filtered {
            let size_str = format_size(*file_size);

            let name_tex = text.render_text(self.texture_creator, &name, GAME_FONT_SIZE,
                NORMAL_COLOR.r, NORMAL_COLOR.g, NORMAL_COLOR.b, NORMAL_COLOR.a);
            let name_sel = text.render_text(self.texture_creator, &name, GAME_FONT_SIZE,
                SELECTED_COLOR.r, SELECTED_COLOR.g, SELECTED_COLOR.b, SELECTED_COLOR.a);
            let size_tex = text.render_text(self.texture_creator, &size_str, GAME_FONT_SIZE,
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

        self.selected = 0;
        self.scroll_offset = 0;
    }

    pub fn handle_input(&mut self, action: InputAction) -> BrowserOutcome {
        match action {
            InputAction::Left => {
                if self.letter_idx > 0 {
                    self.letter_idx -= 1;
                    self.rebuild_game_list();
                }
                BrowserOutcome::None
            }
            InputAction::Right => {
                if self.letter_idx < LETTERS.len() - 1 {
                    self.letter_idx += 1;
                    self.rebuild_game_list();
                }
                BrowserOutcome::None
            }
            InputAction::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                    if self.selected < self.scroll_offset {
                        self.scroll_offset = self.selected;
                    }
                }
                BrowserOutcome::None
            }
            InputAction::Down => {
                if self.selected + 1 < self.rendered_games.len() {
                    self.selected += 1;
                    if self.selected >= self.scroll_offset + MAX_VISIBLE {
                        self.scroll_offset = self.selected - MAX_VISIBLE + 1;
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
        // Letter bar — row 1
        let row1_width: u32 = self.letter_widths[..ROW1_LEN].iter().sum();
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
        let row2_width: u32 = self.letter_widths[ROW1_LEN..].iter().sum();
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

        // Game list
        let end = (self.scroll_offset + MAX_VISIBLE).min(self.rendered_games.len());
        for (vi, gi) in (self.scroll_offset..end).enumerate() {
            let y = GAME_LIST_TOP + (vi as i32 * GAME_SPACING);
            let game = &self.rendered_games[gi];

            let name_tex = if gi == self.selected {
                &self.rendered_selected[gi]
            } else {
                &game.name_texture
            };

            canvas.copy(name_tex, None, Rect::new(GAME_LEFT_MARGIN, y, game.name_w, game.name_h)).unwrap();

            let size_x = WINDOW_WIDTH as i32 - game.size_w as i32 - SIZE_RIGHT_MARGIN;
            canvas.copy(&game.size_texture, None, Rect::new(size_x, y, game.size_w, game.size_h)).unwrap();
        }

        // Legend
        let legend_x = (WINDOW_WIDTH as i32 - self.legend_w as i32) / 2;
        let legend_y = WINDOW_HEIGHT as i32 - self.legend_h as i32 - LEGEND_BOTTOM_MARGIN;
        canvas.copy(&self.legend_texture, None, Rect::new(legend_x, legend_y, self.legend_w, self.legend_h)).unwrap();
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
