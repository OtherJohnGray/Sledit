// file src/app.rs

use anyhow::Result;
use sled::Db;


pub struct App {
    pub db: Option<Db>,
    pub current_tree: Option<sled::Tree>,
    pub current_path: Vec<String>,
    pub current_keys: Vec<String>,
    pub current_value: Option<Vec<u8>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            db: None,
            current_tree: None,
            current_path: vec![],
            current_keys: vec![],
            current_value: None,
        }
    }

    pub fn refresh_trees(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            self.current_keys = db.tree_names()
                    .into_iter()
                    .map(|name| String::from_utf8_lossy(&name).to_string())
                    .collect();
        }
        Ok(())
    }


    pub fn select_tree(&mut self, tree_name: &str) -> Result<()> {
        if let Some(db) = &self.db {
            self.current_tree = Some(db.open_tree(tree_name)?);
            self.current_path.clear();
            self.refresh_keys()?;
        }
        Ok(())
    }

    pub fn refresh_keys(&mut self) -> Result<()> {
        if let Some(tree) = &self.current_tree {
            let prefix = if self.current_path.len() > 0 {
                &format!("{}/", self.current_path.join("/"))
            } else {
                ""
            };
            let mut keys = Vec::new();
            let iter = tree.scan_prefix(prefix.as_bytes());
            for item in iter {
                let (key, _value) = item?;
                let key_str = String::from_utf8_lossy(&key).to_string();
                if let Some(next_segment) = key_str
                    .strip_prefix(&prefix)
                    .and_then(|s| s.split('/').next())
                {
                    if ! keys.contains(&next_segment.to_string()) {
                        keys.push(next_segment.to_string());
                    }
                }
            }
            self.current_keys = keys;
        }
        Ok(())
    }

    pub fn has_subkeys(&self, key: &str ) -> bool {
        if let Some(tree) = &self.current_tree {
            let path = if self.current_path.len() > 0 {
                    format!("{}/{}/", self.current_path.join("/"), key)
                } else {
                    format!("{}/", key)
                };
            return tree.scan_prefix(path.as_bytes()).count() > 0;
        }
        return false;
    }

    pub fn select_key(&mut self, key: &str) -> Result<()> {
        if self.has_subkeys(key) {
            self.current_path.push(key.to_string());
            self.refresh_keys()?;
        }
        Ok(())
    }

    pub fn get_value(&mut self, key: &str) -> Result<()> {
        if let Some(tree) = &self.current_tree {
            let mut new_path = self.current_path.clone();
            new_path.push(key.to_string());
            let full_key = new_path.join("/");
            let value = tree.get(full_key.as_bytes())?;
            if let Some(value) = value {
                self.current_value = Some(value.to_vec());
            }
        }
        Ok(())
    }


    pub fn go_back_in_path(&mut self) -> Result<()> {
        if !self.current_path.is_empty() && self.current_path.len() > 1 {
            self.current_path.pop();
            self.refresh_keys()?;
            self.current_value = None;
        } 
        Ok(())
    }

}
