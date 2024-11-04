use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    execute,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::Margin,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Terminal,
};
use std::path::PathBuf;
use sled::Db;
use std::io;


struct TuiApp {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    app: App,
}


impl TuiApp {
    fn new() -> Result<Self> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            app: App::new(),
        })
    }

    fn select_tree(&mut self) -> Result<()> {
        if let Some(db) = &self.app.db {
            let tree_names: Vec<String> = db
                .tree_names()
                .into_iter()
                .map(|name| String::from_utf8_lossy(&name).to_string())
                .collect();

            let mut selected_index = 0;
            
            loop {
                let items: Vec<String> = std::iter::once("default".to_string())
                    .chain(tree_names.iter().cloned())
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
                        .title("Select Tree (Enter to select)");
                    
                    let list_items: Vec<ListItem> = items.iter()
                        .map(|item| ListItem::new(item.as_str()))
                        .collect();
                    
                    let list = List::new(list_items)
                        .highlight_style(
                            Style::default()
                                .bg(Color::Blue)
                                .fg(Color::White)
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
                            KeyCode::Esc => break,
                            KeyCode::Up => {
                                selected_index = selected_index.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                if selected_index < items.len() - 1 {
                                    selected_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                let selected_name = &items[selected_index];
                                if selected_name == "default" {
                                    self.app.current_tree = None;
                                } else {
                                    self.app.current_tree = Some(db.open_tree(selected_name)?);
                                }
                                self.app.current_path.clear();
                                self.app.current_value = None;
                                self.app.refresh_keys()?;
                                break;
                            }
                            _ => {}
                        }
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

    fn open_db(&mut self) -> Result<()> {
        if let Some(path) = self.file_select()? {
            self.app.db = Some(sled::open(path)?);
            self.app.current_path.clear();
            self.app.current_value = None;
            self.app.refresh_keys()?;
        }
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        loop {
            self.terminal.draw(|frame| {
                let chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                    .split(frame.area());

                let items: Vec<ListItem> = self.app
                    .current_keys
                    .iter()
                    .map(|key| ListItem::new(key.as_str()))
                    .collect();
                
                    let keys_list = List::new(items)
                    .block(Block::default()
                        .title(format!("Keys ({})", 
                            if let Some(tree) = &self.app.current_tree {
                                "Tree: ".to_string() + &String::from_utf8_lossy(&tree.name())
                            } else {
                                "Default Tree".to_string()
                            }
                        ))
                        .borders(Borders::ALL))
                    .highlight_style(Style::default().bg(Color::Gray));
                
                frame.render_stateful_widget(keys_list, chunks[0], &mut self.app.list_state);

                let value_display = if let Some(value) = &self.app.current_value {
                    match rmp_serde::from_slice::<serde_json::Value>(value) {
                        Ok(v) => format!("{}", serde_json::to_string_pretty(&v).unwrap()),
                        Err(_) => "Invalid MessagePack data".to_string(),
                    }
                } else {
                    "No value selected".to_string()
                };

                let value_widget = Paragraph::new(value_display)
                    .block(Block::default().title("Value").borders(Borders::ALL));
                
                frame.render_widget(value_widget, chunks[1]);
            })?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('o') => self.open_db()?,
                        KeyCode::Char('t') => self.select_tree()?,
                        KeyCode::Up => {
                            if let Some(selected) = self.app.list_state.selected() {
                                if selected > 0 {
                                    self.app.list_state.select(Some(selected - 1));
                                }
                            }
                        }
                        KeyCode::Down => {
                            if let Some(selected) = self.app.list_state.selected() {
                                if selected < self.app.current_keys.len().saturating_sub(1) {
                                    self.app.list_state.select(Some(selected + 1));
                                }
                            }
                        }
                        KeyCode::Enter => self.app.select_key()?,
                        KeyCode::Backspace => self.app.go_back()?,
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
        // Restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
        );
    }
}

struct App {
    db: Option<Db>,
    current_tree: Option<sled::Tree>,
    current_path: Vec<String>,
    list_state: ListState,
    current_keys: Vec<String>,
    current_value: Option<Vec<u8>>,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            db: None,
            current_tree: None,
            current_path: vec![],
            list_state,
            current_keys: vec![],
            current_value: None,
        }
    }


    fn refresh_keys(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            let prefix = self.current_path.join("/");
            let mut keys = Vec::new();
            
            let iter = if let Some(tree) = &self.current_tree {
                tree.scan_prefix(prefix.as_bytes())
            } else {
                db.scan_prefix(prefix.as_bytes())
            };
            
            for item in iter {
                let (key, _value) = item?;
                let key_str = String::from_utf8_lossy(&key).to_string();
                if let Some(next_segment) = key_str
                    .strip_prefix(&prefix)
                    .and_then(|s| s.split('/').next())
                {
                    if !keys.contains(&next_segment.to_string()) {
                        keys.push(next_segment.to_string());
                    }
                }
            }
            
            self.current_keys = keys;
            self.list_state.select(Some(0));
        }
        Ok(())
    }

    fn select_key(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            if let Some(selected_index) = self.list_state.selected() {
                if let Some(selected_key) = self.current_keys.get(selected_index) {
                    let mut new_path = self.current_path.clone();
                    new_path.push(selected_key.clone());
                    let full_key = new_path.join("/");
                    
                    let value = if let Some(tree) = &self.current_tree {
                        tree.get(full_key.as_bytes())?
                    } else {
                        db.get(full_key.as_bytes())?
                    };
                    
                    if let Some(value) = value {
                        self.current_value = Some(value.to_vec());
                    } else {
                        self.current_path.push(selected_key.clone());
                        self.refresh_keys()?;
                    }
                }
            }
        }
        Ok(())
    }

    fn go_back(&mut self) -> Result<()> {
        if !self.current_path.is_empty() {
            self.current_path.pop();
            self.refresh_keys()?;
            self.current_value = None;
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    let mut tui = TuiApp::new()?;
    tui.run()?;
    Ok(())
}