// file src/tui_app.rs

use crate::app::*;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout}, prelude::Margin, style::{Color, Style}, widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph}, DefaultTerminal,
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
    pub fn new() -> Result<Self> {
        let terminal = ratatui::init();
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        Ok(Self {
            terminal,
            app: App::new(),
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
                    .map(|key| {
                        if self.app.has_subkeys(key) {
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
                        KeyCode::Char('o') => {
                            self.open_db()?;
                            self.view_mode = ViewMode::Trees;
                            self.app.refresh_trees()?;
                        },
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
                                            let key = &self.app.current_keys[selected - 1].to_owned();
                                            self.app.get_value(key)?;
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
                                            let key = &self.app.current_keys[selected + 1].to_owned();
                                            self.app.get_value(key)?;
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
                            let name = &self.app.current_keys[self.list_state.selected().unwrap_or(0)].to_owned();
                            match self.view_mode {
                                ViewMode::Trees => {
                                    self.view_mode = ViewMode::Keys;
                                    self.app.select_tree(&name)?;
                                    if self.app.current_keys.len() > 0 {
                                        self.app.get_value(&self.app.current_keys[0].to_owned())?;
                                        self.list_state.select(Some(0));
                                    } else {
                                        self.list_state.select(None);
                                    }
                                }
                                ViewMode::Keys => {
                                    if self.app.has_subkeys(name) {
                                        self.app.select_key(&name)?;
                                        if self.app.current_keys.len() > 0 {
                                            self.list_state.select(Some(0));
                                            self.app.get_value(&self.app.current_keys[0].to_owned())?;
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
                                    self.app.get_value(&self.app.current_keys[0].to_owned())?;
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


    fn file_select(&mut self) -> Result<Option<PathBuf>> {
        let mut current_dir = std::env::current_dir()?;
        let mut selected_index = 0;
        
        loop {
            let mut entries = vec![PathBuf::from("..")];  // Add parent directory option
            entries.extend(
                std::fs::read_dir(&current_dir)?
                    .filter_map(|entry| entry.ok())
                    .map(|entry| entry.path())
                    .filter(|path| path.is_dir())  // Only show directories since sled DBs are directories
                    .collect::<Vec<_>>()
            );

            let items: Vec<String> = entries.iter()
                .map(|path| {
                    if path == &PathBuf::from("..") {
                        String::from("..")
                    } else {
                        let name = path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("???");
                        format!("📁 {}", name)
                    }
                })
                .collect();

            self.terminal.draw(|frame| {
                let area = frame.area();
                let width = area.width.min(60);
                let height = area.height.min(20);
                let x = (area.width - width) / 2;
                let y = (area.height - height) / 2;
                
                let modal_area = ratatui::layout::Rect::new(x, y, width, height);
                
                let block = Block::default()
                    .borders(Borders::ALL)
                    .title("Select Database Directory (Enter to navigate, s to select)");
                
                let items: Vec<ListItem> = items.iter()
                    .map(|item| ListItem::new(item.as_str()))
                    .collect();
                
                let list = List::new(items)
                    .highlight_style(
                        Style::default()
                            .bg(Color::Blue)  // Blue background for selection
                            .fg(Color::White)  // White text for selection
                    );
                
                let mut list_state = ListState::default();
                list_state.select(Some(selected_index));
                
                frame.render_widget(Clear, modal_area);
                frame.render_widget(block.clone(), modal_area);
                frame.render_stateful_widget(
                    list,
                    modal_area.inner(Margin { vertical: 1, horizontal: 1 }),
                    &mut list_state
                );
            })?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Esc => return Ok(None),
                        KeyCode::Up => {
                            selected_index = selected_index.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            if selected_index < entries.len() - 1 {
                                selected_index += 1;
                            }
                        }
                        KeyCode::Char('s') => {
                            // Enter selects the current directory as the database
                            let selected_path = &entries[selected_index];
                            if selected_path != &PathBuf::from("..") {
                                return Ok(Some(selected_path.clone()));
                            }
                        }
                        KeyCode::Enter => {
                            // Space enters the directory for navigation
                            let selected_path = &entries[selected_index];
                            if selected_path == &PathBuf::from("..") {
                                if let Some(parent) = current_dir.parent() {
                                    current_dir = parent.to_path_buf();
                                    selected_index = 0;
                                }
                            } else {
                                current_dir = selected_path.clone();
                                selected_index = 0;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // fn handle_selection(&mut self) -> Result<()> {
    //     match self.view_mode {
    //         ViewMode::Trees => self.select_tree()?,
    //         ViewMode::Keys => self.app.select_key()?,
    //     }
    //     Ok(())
    // }

    // fn select_tree(&mut self) -> Result<()> {
    //     if let Some(selected_index) = self.list_state.selected() {
    //         if let Some(selected_tree) = self.app.current_keys.get(selected_index) {
    //             // Update model
    //             self.app.select_tree(&selected_tree.to_owned())?;
    //             // Update view state
    //             self.view_mode = ViewMode::Keys;
    //             self.list_state.select(Some(0));
    //         }
    //     }
    //     Ok(())
    // }

    fn open_db(&mut self) -> Result<()> {
        if let Some(path) = self.file_select()? {
            self.app.db = Some(sled::open(path)?);
            self.app.current_tree = None;
            self.app.current_path.clear();
            self.app.current_value = None;
            self.view_mode = ViewMode::Trees;  // Set view mode to Trees
            self.app.refresh_trees()?;  // Show available trees
        }
        Ok(())
    }

    // pub fn handle_go_back(&mut self) -> Result<()> {
    //     match self.view_mode {
    //         ViewMode::Trees => {
    //             self.app.clear_db();
    //         }
    //         ViewMode::Keys => {
    //             if self.app.current_path.is_empty() {
    //                 // If at root level of keys, go back to trees view
    //                 self.view_mode = ViewMode::Trees;
    //                 self.app.current_tree = None;
    //                 self.app.refresh_trees()?;
    //             } else {
    //                 // Otherwise go up one level in the key hierarchy
    //                 self.app.go_back_in_path()?;
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    // pub fn handle_key_selection(&mut self) -> Result<()> {
    //     if let Some(selected_index) = self.list_state.selected() {
    //         if let Some(selected_key) = self.app.current_keys.get(selected_index) {
    //             self.app.select_key(&selected_key.to_owned())?;
    //         }
    //     }
    //     Ok(())
    // }    

}    


impl Drop for TuiApp {
    fn drop(&mut self) {
        let _ = ratatui::restore();
    }
}
