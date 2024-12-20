// file src/app.rs

use anyhow::{Error, Result};
use sled::Db;
use std::collections::BTreeMap;


pub struct App {
    pub db: Option<Db>,
    pub sled_trees: Vec<String>,
    pub current_tree: Option<sled::Tree>,
    pub current_path: Vec<String>, // current path within cached_key_tree
    pub delimiter: Option<String>,
    cached_key_tree: Option<KeyTree>,
    // current_key_range represents the keys to display in the left panel.
    // If no delimiter, offset and range are within set of all keys in the sled tree
    // if delimiter, offset and range are within the branch of cached_key_tree that is 
    // identified by current_path
    pub current_key_range: KeyRange, // (offset, visible_keys)  
    pub total_keys: usize, 
}

struct KeyTree {
    keys: BTreeMap<String, KeyNode>,
}

struct KeyNode {
    children: BTreeMap<String, KeyNode>,
}

#[derive(Clone)]
pub struct KeyEntry {
    pub key: String,
    pub has_children: bool,
}

pub struct  KeyRange {
    pub offset: usize,
    pub keys: Vec<KeyEntry>,
}

impl App {
    pub fn new() -> Self {
        Self {
            db: None,
            sled_trees: vec![],
            current_tree: None,
            current_path: vec![],
            delimiter: None,
            cached_key_tree: None,
            current_key_range: KeyRange{ offset: 0, keys: vec![] },
            total_keys: 0,
        }
    }

    fn build_key_tree(&mut self) -> Result<()> {
        if let Some(tree) = &self.current_tree {
            // If we have a delimiter, build the hierarchical tree
            if let Some(delimiter) = &self.delimiter {
                let mut key_tree = KeyTree {
                    keys: BTreeMap::new(),
                };

                for result in tree.iter() {
                    let (key, _) = result?;
                    let key_str = String::from_utf8_lossy(&key).to_string();
                    let parts: Vec<&str> = key_str.split(delimiter).collect();
                    
                    let mut current = &mut key_tree.keys;
                    for part in parts.iter() {
                        let entry = current.entry(part.to_string()).or_insert_with(|| KeyNode {
                            children: BTreeMap::new(),
                        });
                        current = &mut entry.children;
                    }
                }
                self.cached_key_tree = Some(key_tree);
            }
        }
        Ok(())
    }

    // Get a range of keys, either from the cached_key_tree (if delimiter) or the DB (if not),
    // and cache it in current_key_range so it can be used to render and to reference keys by index. 
    pub fn set_key_range(&mut self, offset: usize, count: usize) -> Result<()> {
        if self.delimiter.is_none() {
            // Use sled's range functionality for flat key list
            if let Some(tree) = &self.current_tree {
                let mut keys = Vec::with_capacity(count);
                for result in tree.iter().skip(offset).take(count) {
                    let (key, _) = result?;
                    keys.push(KeyEntry {
                        key: String::from_utf8_lossy(&key).to_string(),
                        has_children: false,
                    });
                }
                self.current_key_range = KeyRange{offset, keys};
            } else {
                self.current_key_range = KeyRange{offset: 0, keys: vec![]};
            }
        } else {
            // Use cached key tree for hierarchical keys
            // the key tree is cached when the sled tree is first selected
            if let Some(tree) = &self.cached_key_tree {
                let mut current = &tree.keys;
                for path_segment in &self.current_path {
                    if let Some(node) = current.get(path_segment) {
                        current = &node.children;
                    } else {
                        self.current_key_range = KeyRange{offset: 0, keys: vec![]};
                    }
                }

                let keys: Vec<KeyEntry> = current
                    .iter()
                    .skip(offset)
                    .take(count)
                    .map(|(k, v)| KeyEntry {
                        key: k.clone(),
                        has_children: !v.children.is_empty(),
                    })
                    .collect();
                    self.current_key_range = KeyRange{offset, keys};
                } else {
                self.current_key_range = KeyRange{offset: 0, keys: vec![]};
            }
        }
        Ok(())
    }


    // Total number of keys that can be scrolled in the left pane
    fn total_keys(&self) -> usize {
        if self.current_tree.is_none() { return 0 }
        if self.delimiter.is_none() { return (self.current_tree.as_ref().expect("This is a bug. There should be a guard clause immediately before this.")).len() }
        if self.cached_key_tree.is_none() { return 0 }
        let mut current = &self.cached_key_tree.as_ref().expect("This is a bug. There should be a guard clause immediately before this.").keys;
        for path_segment in &self.current_path {
            if let Some(node) = current.get(path_segment) {
                current = &node.children;
            } else {
                return 0;
            }
        }
        current.iter().count()
    }        


    // Refresh the list of sled trees that are available for selection in this DB
    pub fn refresh_trees(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            let mut trees: Vec<String> = db.tree_names()
                    .into_iter()
                    .map(|name| String::from_utf8_lossy(&name).to_string())
                    .collect();
                trees.sort();
            self.sled_trees = trees;
        }
        Ok(())
    }


    // Select a particular sled tree and cache a tree of it's hierarchical keys if a delimiter is set
    pub fn select_tree(&mut self, index: usize) -> Result<()> {
        if let Some(db) = &self.db {
            self.current_tree = Some(db.open_tree(&self.sled_trees[index])?);
            self.current_path.clear();
            if self.delimiter.is_some() {
                self.build_key_tree()?;
            }
            self.total_keys = self.total_keys();
        }
        Ok(())
    }


    // Navigate down the key hierachy - should only be used if a delimiter is set
    pub fn select_key(&mut self, index: usize) -> Result<()> {
        if self.current_tree.is_some() && self.delimiter.is_some() {
            self.current_path.push(self.current_key_range.keys[index].key.clone());
            self.total_keys = self.total_keys();
        }
        Ok(())
    }    


    // get the value associated with a particular current key
    pub fn get_value(&mut self, index: usize) -> Result<Option<Vec<u8>>, Error> {
        if let Some(tree) = &self.current_tree {
            if self.current_key_range.keys.len() > index {
                let key = self.current_key_range.keys[index].to_owned();
                let mut new_path = self.current_path.clone();
                new_path.push(key.key);
                let full_key = new_path.join("/");
                let value = tree.get(full_key.as_bytes())?;
                if let Some(value) = value {
                    return Ok(Some(value.to_vec()));
                }
            }
        }
        Ok(None)
    }


    // Remove elements from the current path to navigate back up the key hierachy
    pub fn go_back_in_path(&mut self) -> Result<()> {
        if !self.current_path.is_empty() && self.current_path.len() > 1 {
            self.current_path.pop();
            self.total_keys = self.total_keys();
        } 
        Ok(())
    }

}
