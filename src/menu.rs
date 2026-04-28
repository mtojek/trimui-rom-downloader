use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

use std::time::Duration;

use crate::cache::CatalogCache;
use crate::config::Config;
use crate::input::InputAction;
use crate::scene::{Scene, SceneResult};
use crate::widget::{Menu, MenuAction, MenuItem};

#[derive(Debug, Clone, PartialEq)]
enum MenuTarget {
    BrowseSources,
    MyGames,
    Source(usize),
    Catalog(usize, usize),
}

impl Copy for MenuTarget {}

#[derive(Debug, Clone, PartialEq)]
enum State {
    Main,
    BrowseSources,
    SourceCatalogs(usize),
}

pub enum MenuOutcome {
    None,
    OpenGameBrowser { source_idx: usize, catalog_idx: usize },
    OpenMyGames,
    RefreshAll,
}

pub struct MenuScene<'a> {
    state: State,
    menu: Menu<'a, MenuTarget>,
    config: Config,
    texture_creator: &'a TextureCreator<WindowContext>,
}

impl<'a> MenuScene<'a> {
    pub fn new(texture_creator: &'a TextureCreator<WindowContext>, config: Config) -> Self {
        let state = State::Main;
        let items = Self::main_items();
        let menu = Menu::new(texture_creator, &items, "Menu: Exit       A: Confirm");
        MenuScene { state, menu, config, texture_creator }
    }

    pub fn go_to_browse_sources(&mut self) {
        self.transition(State::BrowseSources);
    }

    pub fn new_at_source(texture_creator: &'a TextureCreator<WindowContext>, config: Config, source_idx: usize) -> Self {
        let mut scene = Self::new(texture_creator, config);
        if scene.config.sources[source_idx].catalogs.len() == 1 {
            scene.transition(State::BrowseSources);
        } else {
            scene.transition(State::SourceCatalogs(source_idx));
        }
        scene
    }

    fn main_items() -> Vec<MenuItem<MenuTarget>> {
        vec![
            MenuItem { label: "Browse Sources".to_string(), target: Some(MenuTarget::BrowseSources) },
            MenuItem { label: "My Games".to_string(), target: Some(MenuTarget::MyGames) },
        ]
    }

    fn source_items(&self) -> Vec<MenuItem<MenuTarget>> {
        let cache = CatalogCache::new();
        self.config.sources.iter().enumerate().map(|(i, source)| {
            let age = source.catalogs.iter()
                .filter_map(|c| cache.age(&source.name, c))
                .min();
            let age_str = match age {
                Some(d) => format_age(d),
                None => "never".to_string(),
            };
            MenuItem {
                label: format!("{} ({})", source.name, age_str),
                target: Some(MenuTarget::Source(i)),
            }
        }).collect()
    }

    fn catalog_items(&self, source_idx: usize) -> Vec<MenuItem<MenuTarget>> {
        self.config.sources[source_idx].catalogs.iter().enumerate().map(|(i, catalog)| {
            MenuItem {
                label: catalog.path.clone(),
                target: Some(MenuTarget::Catalog(source_idx, i)),
            }
        }).collect()
    }

    fn transition(&mut self, new_state: State) {
        let (items, legend) = match &new_state {
            State::Main => (Self::main_items(), "Menu: Exit       A: Confirm"),
            State::BrowseSources => (self.source_items(), "Menu: Exit    B: Back    A: Confirm    Y: Refresh"),
            State::SourceCatalogs(_) => (self.catalog_items(match &new_state {
                State::SourceCatalogs(i) => *i,
                _ => unreachable!(),
            }), "Menu: Exit       B: Back       A: Confirm"),
        };
        self.state = new_state;
        self.menu = Menu::new(self.texture_creator, &items, legend);
    }

    fn parent(&self) -> Option<State> {
        match &self.state {
            State::BrowseSources => Some(State::Main),
            State::SourceCatalogs(_) => Some(State::BrowseSources),
            _ => None,
        }
    }

    pub fn handle_input(&mut self, action: InputAction) -> MenuOutcome {
        if action == InputAction::Refresh && self.state == State::BrowseSources {
            return MenuOutcome::RefreshAll;
        }
        match self.menu.handle_input(action) {
            MenuAction::Selected(target) => match target {
                MenuTarget::BrowseSources => {
                    self.transition(State::BrowseSources);
                    MenuOutcome::None
                }
                MenuTarget::MyGames => {
                    MenuOutcome::OpenMyGames
                }
                MenuTarget::Source(idx) => {
                    let catalogs = &self.config.sources[idx].catalogs;
                    if catalogs.len() == 1 {
                        MenuOutcome::OpenGameBrowser { source_idx: idx, catalog_idx: 0 }
                    } else {
                        self.transition(State::SourceCatalogs(idx));
                        MenuOutcome::None
                    }
                }
                MenuTarget::Catalog(source_idx, catalog_idx) => {
                    MenuOutcome::OpenGameBrowser { source_idx, catalog_idx }
                }
            },
            MenuAction::Back => {
                if let Some(parent) = self.parent() {
                    self.transition(parent);
                }
                MenuOutcome::None
            }
            MenuAction::None => MenuOutcome::None,
        }
    }
}

fn format_age(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

impl<'a> Scene for MenuScene<'a> {
    fn update(&mut self, _elapsed: u128) -> SceneResult {
        SceneResult::Continue
    }

    fn render(&mut self, canvas: &mut Canvas<Window>, _elapsed: u128) {
        self.menu.render(canvas);
    }
}
