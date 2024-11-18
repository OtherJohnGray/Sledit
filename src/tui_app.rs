// file src/tui_app.rs

use crate::app::*;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect}, prelude::Stylize, style::{Color, Style}, widgets::{Block, Borders, List, ListItem, ListState, Paragraph}, 
    DefaultTerminal, Frame
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct TuiApp {
    terminal: DefaultTerminal,
    app: App,
    view_mode: ViewMode,
    list_state: ListState,
    focused_pane: Pane,
    scroll_state: u16,
    max_scroll: u16,
    page_height: u16,
    wrap_text: bool,
    horizontal_scroll: u16,
    max_horizontal_scroll: u16,
    status_message: Option<String>,
    list_offset: usize,     // Starting index of the current window
    list_height: u16,
}

#[derive(PartialEq)]
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
        let mut terminal = ratatui::init();
        terminal.clear()?;
        println!("Opening database....");
        let mut app = App::new();
        terminal.clear()?;
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        
        app.db = Some(sled::open(db_path)?);
        app.refresh_trees()?;

        Ok(Self {
            terminal,
            app: app,
            view_mode: ViewMode::Trees,
            list_state,
            focused_pane: Pane::List,
            scroll_state: 0,
            max_scroll: 0,
            page_height: 0, 
            wrap_text: true,
            horizontal_scroll: 0,
            max_horizontal_scroll: 0,
            status_message: None,
            list_offset: 0,
            list_height: 0,     
        })
    }


    pub fn run(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        loop {
            self.draw()?;
            self.handle_input(running.clone())?;
            if !running.load(Ordering::SeqCst) {
                break;
            }
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        self.terminal.draw(|frame| {
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Path display
                    Constraint::Min(0),     // Main content
                    Constraint::Length(1),  // info bar
                ].as_ref())
                .split(frame.area());

            self.list_height = vertical_chunks[1].height.saturating_sub(2);
            self.page_height = vertical_chunks[1].height.saturating_sub(2); // calculate this again, don't just copy list_height as may not be same in future


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


            // render info bar
            if let Some(message) = &self.status_message {
                frame.render_widget(Paragraph::new(message.to_owned()), vertical_chunks[2]);
            } else {
                let key_help = match self.focused_pane {
                    // Pane::List =>   "q)uit - [enter] show subkeys - [backspace] show parent key - ↓↑ select key - [tab] select value pane - ←→ resize panes",
                    Pane::List =>   &format!("list_height {} - list_offset {} - total_keys {} - num trees {}", self.list_height, self.list_offset, self.app.total_keys, self.app.sled_trees.len()),
                    Pane::Value =>  "↓↑←→ scroll - [shift] x10 - [tab] select key pane - e)dit"
                };
                frame.render_widget(Paragraph::new(key_help), vertical_chunks[2]);

            }

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                .split(vertical_chunks[1]);


            // render tree or key list
            match self.view_mode {
                ViewMode::Trees => {
                    draw_tree_list(
                        frame,
                        chunks[0],
                        &self.app.sled_trees,
                        &mut self.list_state,
                        self.app.total_keys
                    );
                }
                ViewMode::Keys => {
                    draw_key_list(
                        frame,
                        chunks[0],
                        &self.app.current_key_range.keys,
                        &mut self.list_state,
                        self.app.total_keys,
                        self.app.current_tree.as_ref()
                    );
                }
            }


            
            if let Ok(Some(value)) = &self.app.get_value(self.list_state.selected().unwrap_or(0)) {
                let content = String::from_utf8_lossy(value).to_string();
                let lines: Vec<&str> = content.split('\n').collect();
                let visible_width = chunks[1].width.saturating_sub(2);

                let total_lines = if self.wrap_text {
                    calculate_wrapped_lines(&content, visible_width)
                } else {
                    content.split('\n').count()
                };

                // Calculate max scroll based on total wrapped lines
                self.max_scroll = total_lines.saturating_sub(self.page_height as usize) as u16;
                self.scroll_state = self.scroll_state.min(self.max_scroll);

                self.max_horizontal_scroll = if !self.wrap_text {
                    lines.iter()
                        .map(|line| line.len())
                        .max()
                        .unwrap_or(0)
                        .saturating_sub(visible_width as usize) as u16
                } else {
                    0
                };
                self.horizontal_scroll = self.horizontal_scroll.min(self.max_horizontal_scroll);

                let wrap_indicator = if self.wrap_text { "W" } else { "NW" };
                let scroll_indicator = if self.max_scroll > 0 {
                    format!(" [{}/{}]", self.scroll_state + 1, self.max_scroll + 1)
                } else {
                    String::new()
                };
                let h_scroll_indicator = if !self.wrap_text && self.max_horizontal_scroll > 0 {
                    format!(" <{}>", self.horizontal_scroll)
                } else {
                    String::new()
                };                    

                let value_widget = Paragraph::new(content)
                .block(Block::default()
                    .title(format!("Value [{}]{}{}", 
                        wrap_indicator, 
                        scroll_indicator,
                        h_scroll_indicator
                    ))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(
                        if matches!(self.focused_pane, Pane::Value) {
                            Color::Blue
                        } else {
                            Color::White
                        }
                    )));
            
                let value_widget = if self.wrap_text {
                    value_widget.wrap(ratatui::widgets::Wrap { trim: false })
                } else {
                    value_widget
                };
                
                let value_widget = value_widget.scroll((self.scroll_state as u16, self.horizontal_scroll));
            

                frame.render_widget(value_widget, chunks[1]);
            }

        })?;
        Ok(())
    }





    fn handle_input(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::FocusGained => {},
                Event::FocusLost => {},
                Event::Mouse(_) => {},
                Event::Resize(_,_) => {},                    
                Event::Paste(_) => {},
                Event::Key(key) => {
                    self.status_message = None;
                    match key.code {
                        KeyCode::Char('q') => {
                            running.store(false, Ordering::SeqCst);
                        },
                        KeyCode::Tab => {
                            self.focused_pane = match self.focused_pane {
                                Pane::List => Pane::Value,
                                Pane::Value => Pane::List,
                            };
                            self.scroll_state = 0; // Reset scroll when switching panes
                        },
                        KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right | KeyCode::PageUp | KeyCode::PageDown | KeyCode::Home | KeyCode::End => {
                            if matches!(self.focused_pane, Pane::Value) {
                                let shift_pressed = key.modifiers.contains(event::KeyModifiers::SHIFT);
                                let movement = if shift_pressed { 10 } else { 1 };
                                
                                match key.code {
                                    KeyCode::Up => {
                                        self.scroll_state = self.scroll_state.saturating_sub(movement);
                                    }
                                    KeyCode::Down => {
                                        self.scroll_state = (self.scroll_state + movement).min(self.max_scroll);
                                    }
                                    KeyCode::Left if !self.wrap_text => {
                                        self.horizontal_scroll = self.horizontal_scroll.saturating_sub(movement);
                                    }
                                    KeyCode::Right if !self.wrap_text => {
                                        self.horizontal_scroll = (self.horizontal_scroll + movement)
                                            .min(self.max_horizontal_scroll);
                                    }
                                    KeyCode::PageUp => {
                                        if matches!(self.focused_pane, Pane::Value) {
                                            self.scroll_state = self.scroll_state.saturating_sub(self.page_height.saturating_sub(1));
                                        }
                                    },
                                    KeyCode::PageDown => {
                                        if matches!(self.focused_pane, Pane::Value) {
                                            self.scroll_state = (self.scroll_state + self.page_height.saturating_sub(1)).min(self.max_scroll);
                                        }
                                    },
                                    KeyCode::Home => {
                                        if matches!(self.focused_pane, Pane::Value) {
                                            self.scroll_state = 0;
                                            self.horizontal_scroll = 0;
                                        }
                                    },
                                    KeyCode::End => {
                                        if matches!(self.focused_pane, Pane::Value) {
                                            self.scroll_state = self.max_scroll;
                                        }
                                    },     
        
                                    _ => {}
                                }
                            } else {
                                self.handle_list_navigation(key.code)?;
                            }
                        }
                        KeyCode::Enter => {
                            if matches!(self.focused_pane, Pane::List) {
                                let index = self.list_state.selected().unwrap_or(0);
                                match self.view_mode {
                                    ViewMode::Trees => {
                                        self.view_mode = ViewMode::Keys;
                                        self.app.select_tree(index)?;
                                        self.app.set_key_range(0, self.list_height as usize)?;
                                    }
                                    ViewMode::Keys => {
                                        if self.app.delimiter.is_some() {
                                            if self.app.current_key_range.keys[index].has_children {
                                                self.app.select_key(index)?;

                                            }
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Backspace => {
                            self.focused_pane = Pane::List;
                            if self.app.current_path.len() > 1 {
                                self.app.go_back_in_path()?;
                            } else { // go back to tree mode, assume at least Default tree available
                                self.view_mode = ViewMode::Trees;
                                self.list_offset = 0;
                                self.app.total_keys = 0;
                                self.app.current_tree = None;
                            }
                            self.list_state.select(Some(0));
                        },
                        KeyCode::Char('w') => {
                            if matches!(self.focused_pane, Pane::Value) {
                                self.wrap_text = !self.wrap_text;
                                self.horizontal_scroll = 0;
                            }
                        },

                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }


    fn handle_list_navigation(&mut self, key: KeyCode) -> Result<()> {
        let element_count = match self.view_mode {
            ViewMode::Trees => self.app.sled_trees.len(),
            ViewMode::Keys => self.app.total_keys,
        };

        if matches!(self.view_mode, ViewMode::Keys) && element_count == 0 {
            return Ok(());
        }

        let relative_selection = self.list_state.selected().unwrap_or(0);  // Relative to visible items
        let absolute_selection = self.list_offset + relative_selection;  // Actual position in full dataset

        match key {
            KeyCode::Up => {
                if absolute_selection > 0 {
                    if relative_selection > 0 {
                        // Just move the selection up
                        self.list_state.select(Some(relative_selection - 1));
                    } else {
                        // At top of window, need to shift window up
                        self.list_offset = self.list_offset.saturating_sub(1);
                        if matches!(self.view_mode, ViewMode::Keys) {
                            self.update_list()?;
                        }
                    }
                }
            },
            KeyCode::Down => {
                if absolute_selection + 1 < element_count {
                    if relative_selection + 1 < self.list_height as usize {
                        // Just move the selection down
                        self.list_state.select(Some(relative_selection + 1));
                    } else {
                        // At bottom of window, need to shift window down
                        self.list_offset += 1;
                        if matches!(self.view_mode, ViewMode::Keys) {
                            self.update_list()?;
                        }
                    }
                }
            },
            KeyCode::PageUp => {
                if self.list_offset > 0 {
                    // Move window up by visible_height or to start
                    self.list_offset = self.list_offset.saturating_sub(self.list_height as usize);
                    if matches!(self.view_mode, ViewMode::Keys) {
                        self.update_list()?;
                    }
            // Keep selection at top of new window
                    self.list_state.select(Some(0));
                } else if relative_selection > 0 {
                    // Already at top of data, just move selection to top of window
                    self.list_state.select(Some(0));
                }
            },
            KeyCode::PageDown => {
                let max_offset = element_count.saturating_sub(self.list_height as usize);
                if self.list_offset < max_offset {
                    // Move window down by visible_height or to end
                    self.list_offset = (self.list_offset + self.list_height as usize).min(max_offset);
                    if matches!(self.view_mode, ViewMode::Keys) {
                        self.update_list()?;
                    }
            // Keep selection at bottom of new window
                    self.list_state.select(Some(self.list_height as usize - 1));
                } else if relative_selection < self.list_height as usize - 1 {
                    // Already at bottom of data, just move selection to bottom of window
                    self.list_state.select(Some(self.list_height as usize - 1));
                }
            },
            _ => {}
        }
        // panic!("list height is {}", self.list_height);
        Ok(())
    }


    fn update_list(&mut self) -> Result<()> {
        // Get just enough items to fill the visible area
        self.app.set_key_range(self.list_offset, self.list_height as usize)?;
        Ok(())
    }


}    


impl Drop for TuiApp {
    fn drop(&mut self) {
        let _ = ratatui::restore();
    }
}


fn draw_tree_list(
    frame: &mut Frame,
    area: Rect,
    trees: &Vec<String>,
    list_state: &mut ListState,
    total_keys: usize,
) {
    if !trees.is_empty() {
        let items: Vec<ListItem> = trees
            .iter()
            .map(|entry| {
                ListItem::new(entry.clone())
            })
            .collect();

        let trees_list = List::new(items)
            .block(Block::default()
                .title(format!(" {} Keys ", total_keys))
                .borders(Borders::ALL))
            .highlight_style(Style::default().reversed());
        
        frame.render_stateful_widget(trees_list, area, list_state);
    } else {
        frame.render_widget(
            Paragraph::new("No SledDB trees found!"),
            area
        );
    }
}


fn draw_key_list(
    frame: &mut Frame,
    area: Rect,
    keys: &Vec<KeyEntry>,
    list_state: &mut ListState,
    total_keys: usize,
    current_tree: Option<&sled::Tree>,
) {
    if !keys.is_empty() {
        let items: Vec<ListItem> = keys
            .iter()
            .map(|entry| {
                if entry.has_children {
                    ListItem::new(format!("{} +", entry.key))
                } else {
                    ListItem::new(entry.key.clone())
                }
            })
            .collect();

        let keys_list = List::new(items)
            .block(Block::default()
                .title(format!(" {} Keys ", total_keys))
                .borders(Borders::ALL))
            .highlight_style(Style::default().reversed());
        
        frame.render_stateful_widget(keys_list, area, list_state);
    } else {
        let tree_name = if let Some(tree) = current_tree {
            String::from_utf8_lossy(&tree.name()).into_owned()
        } else {
            "Default".to_owned()
        };

        frame.render_widget(
            Paragraph::new(format!("No Keys found in tree {}", tree_name)),
            area
        );
    }
}



fn calculate_wrapped_lines(text: &str, width: u16) -> usize {
    let width = width as usize;
    let mut total_lines = 0;

    for line in text.split('\n') {
        if line.is_empty() {
            total_lines += 1;
            continue;
        }

        let mut remaining = line;
        while !remaining.is_empty() {
            total_lines += 1;
            
            // Find the last space within the width limit
            let mut split_at = width;
            if remaining.len() > width {
                // Look for a space to break at
                if let Some(last_space) = remaining[..width].rfind(' ') {
                    split_at = last_space + 1;
                }
            } else {
                break;
            }

            remaining = &remaining[split_at.min(remaining.len())..];
            
            // Handle the case where a very long word is wrapped
            if remaining.len() > width && split_at == width {
                // No space found, force wrap at width
                remaining = &remaining[width..];
            }
        }
    }

    total_lines
}


