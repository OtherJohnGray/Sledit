// file src/tui_app.rs

use crate::app::*;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout}, style::{Color, Style}, widgets::{Block, Borders, List, ListItem, ListState, Paragraph}, DefaultTerminal,
    prelude::Stylize,
};
use std::path::PathBuf;

pub struct TuiApp {
    terminal: DefaultTerminal,
    app: App,
    view_mode: ViewMode,
    list_state: ListState,
    scroll_state: u16,
    focused_pane: Pane,
}

pub enum Pane {
    List,
    Value
}

pub enum ViewMode {
    Trees,
    Keys,
}


impl TuiApp {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let terminal = ratatui::init();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        let mut app = App::new();
        app.db = Some(sled::open(db_path)?);
        app.refresh_trees()?;

        Ok(Self {
            terminal,
            app: app,
            view_mode: ViewMode::Trees,
            list_state,
            scroll_state: 0,
            focused_pane: Pane::List,            
        })
    }


    pub fn run(&mut self) -> Result<()> {
        loop {
            self.terminal.draw(|frame| {
                let vertical_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),  // Path display
                        Constraint::Min(0),     // Main content
                    ].as_ref())
                    .split(frame.area());

                // Render path at top
                let path_text = match self.view_mode {
                    ViewMode::Trees => "Select Tree".to_string(),
                    ViewMode::Keys => {
                        let tree_name = if let Some(tree) = &self.app.current_tree {
                            String::from_utf8_lossy(&tree.name()).to_string()
                        } else {
                            "default".to_string()
                        };
                        format!("Tree: {} | Path: /{}", tree_name, self.app.current_path.join("/"))
                    }
                };
                
                let path_widget = Paragraph::new(path_text)
                    .block(Block::default().borders(Borders::ALL));
                frame.render_widget(path_widget, vertical_chunks[0]);

                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                    .split(vertical_chunks[1]);

                
                let list_title = match self.view_mode {
                    ViewMode::Trees => format!(" {} Trees:", self.app.current_keys.len()),
                    ViewMode::Keys => format!(" {} Keys:", self.app.current_keys.len()),
                };

                if self.app.current_keys.len() > 0 {
                    let items: Vec<ListItem> = self.app
                    .current_keys
                    .iter()
                    .enumerate()  
                    .map(|(index, key)| { 
                        if self.app.has_subkeys(index) {
                            ListItem::new(format!("{} +", key))
                        } else {
                            ListItem::new(key.as_str())
                        }
                    })
                    .collect();

                    let keys_list = List::new(items)
                    .block(Block::default()
                        .title(list_title)
                        .borders(Borders::ALL))
                        .highlight_style(Style::default().reversed());
                        // .highlight_style(Style::default().bg(Color::Gray));
                
                    frame.render_stateful_widget(keys_list, chunks[0], &mut self.list_state);
                } else {
                    let tree_name = if let Some(tree) = &self.app.current_tree {
                        &String::from_utf8_lossy(&tree.name()).into_owned()
                    } else {
                        "Default"
                    };

                    frame.render_widget(
                        Paragraph::new(format!("No Keys found in tree {}", &tree_name)), // assume key mode if we got her, since there is always a Default item in trees list.
                        chunks[0]
                    );
                }

                if let Some(value) = &self.app.current_value {
                    let content = String::from_utf8_lossy(value).to_string();
                    let value_widget = Paragraph::new(content)
                        .block(Block::default()
                            .title("Value")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(
                                if matches!(self.focused_pane, Pane::Value) {
                                    Color::Blue
                                } else {
                                    Color::White
                                }
                            )))
                        .scroll((self.scroll_state, 0));
                    frame.render_widget(value_widget, chunks[1]);
                }

            })?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Tab => {
                            self.focused_pane = match self.focused_pane {
                                Pane::List => Pane::Value,
                                Pane::Value => Pane::List,
                            };
                            self.scroll_state = 0; // Reset scroll when switching panes
                        },                        
                        KeyCode::Up => {
                            match self.focused_pane {
                                Pane::List => {
                                    if let Some(selected) = self.list_state.selected() {
                                        if selected > 0 {
                                            self.app.get_value(selected - 1)?;
                                            self.list_state.select(Some(selected - 1));
                                        }
                                    }
                                }
                                Pane::Value => {
                                    self.scroll_state = self.scroll_state.saturating_sub(1);
                                }
                            }                            
                        }
                        KeyCode::Down => {
                            match self.focused_pane {
                                Pane::List => {
                                    if let Some(selected) = self.list_state.selected() {
                                        if selected < self.app.current_keys.len().saturating_sub(1) {
                                            self.app.get_value(selected + 1)?;
                                            self.list_state.select(Some(selected + 1));
                                        }
                                    }
                                }
                                Pane::Value => {
                                    // You might want to add a maximum scroll limit based on content height
                                    self.scroll_state = self.scroll_state.saturating_add(1);
                                }
                            }                            
                        }
                        KeyCode::Enter => {
                            match self.view_mode {
                                ViewMode::Trees => {
                                    self.view_mode = ViewMode::Keys;
                                    self.app.select_tree(self.list_state.selected().unwrap_or(0))?;
                                    if self.app.current_keys.len() > 0 {
                                        self.app.get_value(0)?;
                                        self.list_state.select(Some(0));
                                    } else {
                                        self.list_state.select(None);
                                    }
                                }
                                ViewMode::Keys => {
                                    let index = self.list_state.selected().unwrap_or(0);
                                    if self.app.has_subkeys(index) {
                                        self.app.select_key(index)?;
                                        if self.app.current_keys.len() > 0 {
                                            self.list_state.select(Some(0));
                                            self.app.get_value(0)?;
                                        } else {
                                            self.list_state.select(None);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            if self.app.current_path.len() > 1 {
                                self.app.go_back_in_path()?;
                                if self.app.current_keys.len() > 0 {
                                    self.app.get_value(0)?;
                                    self.list_state.select(Some(0));    
                                } else {
                                    self.list_state.select(None);
                                }
                            } else { // go back to tree mode, assume at least Default tree available
                                self.view_mode = ViewMode::Trees;
                                self.app.refresh_trees()?;
                                self.app.current_value = None;
                                self.list_state.select(Some(0));
                            }
                        },
                        KeyCode::PageUp => {
                            if matches!(self.focused_pane, Pane::Value) {
                                self.scroll_state = self.scroll_state.saturating_sub(10);
                            }
                        },
                        KeyCode::PageDown => {
                            if matches!(self.focused_pane, Pane::Value) {
                                self.scroll_state = self.scroll_state.saturating_add(10);
                            }
                        },
                        KeyCode::Home => {
                            if matches!(self.focused_pane, Pane::Value) {
                                self.scroll_state = 0;
                            }
                        },
                        KeyCode::End => {
                            if matches!(self.focused_pane, Pane::Value) {
                                // You might want to calculate this based on content height
                                self.scroll_state = u16::MAX;
                            }
                        },                        
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

}    


impl Drop for TuiApp {
    fn drop(&mut self) {
        let _ = ratatui::restore();
    }
}
