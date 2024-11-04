use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use rfd::FileDialog;
use sled::Db;
use std::io;


struct App {
    db: Option<Db>,
    current_path: Vec<String>,
    list_state: ListState,  // Replace selected_index with ListState
    current_keys: Vec<String>,
    current_value: Option<Vec<u8>>,
}

impl App {
    fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));  // Initialize with first item selected
        Self {
            db: None,
            current_path: vec![],
            list_state,  // Use list_state instead of selected_index
            current_keys: vec![],
            current_value: None,
        }
    }

    fn open_db(&mut self) -> Result<()> {
        if let Some(path) = FileDialog::new().pick_file() {
            self.db = Some(sled::open(path)?);
            self.refresh_keys()?;
        }
        Ok(())
    }

    fn refresh_keys(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            let prefix = self.current_path.join("/");
            let mut keys = Vec::new();
            
            for item in db.scan_prefix(prefix.as_bytes()) {
                let (key, value) = item?;
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
            self.selected_index = 0;
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
                    
                    if let Some(value) = db.get(full_key.as_bytes())? {
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
    enable_raw_mode()?;
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = App::new();

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
                .split(frame.area());

            // Keys list
            let items: Vec<ListItem> = app
                .current_keys
                .iter()
                .map(|key| ListItem::new(key.as_str()))
                .collect();
            
            let keys_list = List::new(items)
                .block(Block::default().title("Keys").borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::Gray));
            
            frame.render_stateful_widget(keys_list, chunks[0], &mut app.list_state);

            // Value display
            let value_display = if let Some(value) = &app.current_value {
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

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Char('o') => app.open_db()?,
                KeyCode::Up => {
                    if let Some(selected) = app.list_state.selected() {
                        if selected > 0 {
                            app.list_state.select(Some(selected - 1));
                        }
                    }
                }
                KeyCode::Down => {
                    if let Some(selected) = app.list_state.selected() {
                        if selected < app.current_keys.len().saturating_sub(1) {
                            app.list_state.select(Some(selected + 1));
                        }
                    }
                }
                KeyCode::Enter => app.select_key()?,
                KeyCode::Backspace => app.go_back()?,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    terminal.clear()?;
    Ok(())
}