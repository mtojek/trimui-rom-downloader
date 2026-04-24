use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::input::InputAction;
use crate::text::TextRenderer;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

const ITEM_FONT_SIZE: f32 = 44.0;
const LEGEND_FONT_SIZE: f32 = 36.0;
const ITEM_COLOR: Color = Color::RGBA(200, 200, 200, 255);
const SELECTED_COLOR: Color = Color::RGBA(255, 220, 80, 255);
const LEGEND_COLOR: Color = Color::RGBA(0, 0, 0, 220);
const ITEM_SPACING: i32 = 70;
const LEGEND_BOTTOM_MARGIN: i32 = 12;
const BOX_PADDING_X: i32 = 20;
const BOX_PADDING_Y: i32 = 12;
const BOX_COLOR: Color = Color::RGBA(0, 0, 0, 200);

pub struct MenuItem<S> {
    pub label: String,
    pub target: Option<S>,
}

struct RenderedItem<'a> {
    normal: Texture<'a>,
    selected: Texture<'a>,
    width: u32,
    height: u32,
}

pub struct Menu<'a, S> {
    items: Vec<RenderedItem<'a>>,
    targets: Vec<Option<S>>,
    legend_texture: Texture<'a>,
    legend_width: u32,
    legend_height: u32,
    selected: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAction<S> {
    None,
    Selected(S),
    Back,
}

impl<'a, S: Copy> Menu<'a, S> {
    pub fn new(
        texture_creator: &'a TextureCreator<WindowContext>,
        items: &[MenuItem<S>],
        legend: &str,
    ) -> Self {
        let text_renderer = TextRenderer::new();

        let rendered_items: Vec<RenderedItem<'a>> = items
            .iter()
            .map(|item| {
                let normal = text_renderer.render_text(
                    texture_creator,
                    &item.label,
                    ITEM_FONT_SIZE,
                    ITEM_COLOR.r,
                    ITEM_COLOR.g,
                    ITEM_COLOR.b,
                    ITEM_COLOR.a,
                );
                let selected = text_renderer.render_text(
                    texture_creator,
                    &item.label,
                    ITEM_FONT_SIZE,
                    SELECTED_COLOR.r,
                    SELECTED_COLOR.g,
                    SELECTED_COLOR.b,
                    SELECTED_COLOR.a,
                );
                let query = normal.query();
                RenderedItem {
                    normal,
                    selected,
                    width: query.width,
                    height: query.height,
                }
            })
            .collect();

        let legend_texture = text_renderer.render_text(
            texture_creator,
            legend,
            LEGEND_FONT_SIZE,
            LEGEND_COLOR.r,
            LEGEND_COLOR.g,
            LEGEND_COLOR.b,
            LEGEND_COLOR.a,
        );
        let legend_query = legend_texture.query();

        let targets: Vec<Option<S>> = items.iter().map(|item| item.target).collect();

        Menu {
            items: rendered_items,
            targets,
            legend_texture,
            legend_width: legend_query.width,
            legend_height: legend_query.height,
            selected: 0,
        }
    }

    pub fn handle_input(&mut self, action: InputAction) -> MenuAction<S> {
        match action {
            InputAction::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                MenuAction::None
            }
            InputAction::Down => {
                if self.selected < self.items.len() - 1 {
                    self.selected += 1;
                }
                MenuAction::None
            }
            InputAction::Confirm => {
                if let Some(target) = self.targets[self.selected] {
                    MenuAction::Selected(target)
                } else {
                    MenuAction::None
                }
            }
            InputAction::Back => MenuAction::Back,
            _ => MenuAction::None,
        }
    }

    pub fn render(&self, canvas: &mut Canvas<Window>) {
        let total_menu_height =
            self.items.len() as i32 * ITEM_SPACING - (ITEM_SPACING - self.items[0].height as i32);
        let menu_top_y = (WINDOW_HEIGHT as i32 - total_menu_height) / 2;

        // Background box
        let max_width = self.items.iter().map(|i| i.width).max().unwrap_or(0) as i32;
        let box_x = (WINDOW_WIDTH as i32 - max_width) / 2 - BOX_PADDING_X;
        let box_y = menu_top_y - BOX_PADDING_Y;
        let box_w = (max_width + 2 * BOX_PADDING_X) as u32;
        let box_h = (total_menu_height + 2 * BOX_PADDING_Y) as u32;
        canvas.set_draw_color(BOX_COLOR);
        canvas.fill_rect(Rect::new(box_x, box_y, box_w, box_h)).unwrap();

        for (i, item) in self.items.iter().enumerate() {
            let y = menu_top_y + (i as i32 * ITEM_SPACING);
            let x = (WINDOW_WIDTH as i32 - item.width as i32) / 2;
            let texture = if i == self.selected {
                &item.selected
            } else {
                &item.normal
            };
            canvas
                .copy(texture, None, Rect::new(x, y, item.width, item.height))
                .unwrap();
        }

        // Legend bar at bottom center
        let legend_x = (WINDOW_WIDTH as i32 - self.legend_width as i32) / 2;
        let legend_y = WINDOW_HEIGHT as i32 - self.legend_height as i32 - LEGEND_BOTTOM_MARGIN;
        canvas
            .copy(
                &self.legend_texture,
                None,
                Rect::new(legend_x, legend_y, self.legend_width, self.legend_height),
            )
            .unwrap();
    }
}
