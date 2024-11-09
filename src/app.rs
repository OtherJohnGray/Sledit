// file src/app.rs

use anyhow::Result;
use sled::Db;
use std::collections::BTreeMap;


pub struct App {
    pub db: Option<Db>,
    pub sled_trees: Vec<String>,
    pub current_tree: Option<sled::Tree>,
    pub current_path: Vec<String>,
    pub current_value: Option<Vec<u8>>,
    pub delimiter: Option<String>,
    cached_key_tree: Option<KeyTree>,
    current_key_range: (usize, Vec<KeyEntry>), // (offset, visible_keys)    
}

struct KeyTree {
    keys: BTreeMap<String, KeyNode>,
    total_keys: usize,
}

struct KeyNode {
    children: BTreeMap<String, KeyNode>,
    has_value: bool,
}

#[derive(Clone)]
pub struct KeyEntry {
    pub key: String,
    pub has_children: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            db: None,
            sled_trees: vec![],
            current_tree: None,
            current_path: vec![],
            current_value: None,
            delimiter: None,
            cached_key_tree: None,
            current_key_range: (0, Vec::new()),
        }
    }

    fn build_key_tree(&mut self) -> Result<()> {
        if let Some(tree) = &self.current_tree {
            // If we have a delimiter, build the hierarchical tree
            if let Some(delimiter) = &self.delimiter {
                let mut key_tree = KeyTree {
                    keys: BTreeMap::new(),
                    total_keys: 0,
                };

                for result in tree.iter() {
                    let (key, _) = result?;
                    let key_str = String::from_utf8_lossy(&key).to_string();
                    let parts: Vec<&str> = key_str.split(delimiter).collect();
                    
                    let mut current = &mut key_tree.keys;
                    for (i, part) in parts.iter().enumerate() {
                        let is_last = i == parts.len() - 1;
                        let entry = current.entry(part.to_string()).or_insert_with(|| KeyNode {
                            children: BTreeMap::new(),
                            has_value: false,
                        });
                        if is_last {
                            entry.has_value = true;
                        }
                        current = &mut entry.children;
                    }
                }
                key_tree.total_keys = self.count_visible_keys(&key_tree)?;
                self.cached_key_tree = Some(key_tree);
            }
        }
        Ok(())
    }

    fn count_visible_keys(&self, tree: &KeyTree) -> Result<usize> {
        if self.delimiter.is_none() {
            if let Some(tree) = &self.current_tree {
                Ok(tree.len() as usize)
            } else {
                Ok(0)
            }
        } else {
            let mut current = &tree.keys;
            for path_segment in &self.current_path {
                if let Some(node) = current.get(path_segment) {
                    current = &node.children;
                } else {
                    return Ok(0);
                }
            }
            Ok(current.len())
        }
    }

    pub fn get_keys_range(&mut self, offset: usize, count: usize) -> Result<Vec<KeyEntry>> {
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
                self.current_key_range = (offset, keys.clone());
                Ok(keys)
            } else {
                Ok(Vec::new())
            }
        } else {
            // Use cached tree for hierarchical keys
            if let Some(tree) = &self.cached_key_tree {
                let mut current = &tree.keys;
                for path_segment in &self.current_path {
                    if let Some(node) = current.get(path_segment) {
                        current = &node.children;
                    } else {
                        return Ok(Vec::new());
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

                self.current_key_range = (offset, keys.clone());
                Ok(keys)
            } else {
                Ok(Vec::new())
            }
        }
    }

    pub fn total_keys(&self) -> usize {
        if let Some(tree) = &self.cached_key_tree {
            let mut current = &tree.keys;
            for path_segment in &self.current_path {
                if let Some(node) = current.get(path_segment) {
                    current = &node.children;
                } else {
                    return 0;
                }
            }
            current.iter().count()
        } else {
            0
        }
    }        

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


    pub fn select_tree(&mut self, index: usize) -> Result<()> {
        if let Some(db) = &self.db {
            self.current_tree = Some(db.open_tree(&self.sled_trees[index])?);
            self.current_path.clear();
            // self.refresh_keys()?;
        }
        Ok(())
    }

    // pub fn refresh_keys(&mut self) -> Result<()> {
    //     if let Some(tree) = &self.current_tree {
    //         let prefix = if self.current_path.len() > 0 {
    //             &format!("{}/", self.current_path.join("/"))
    //         } else {
    //             ""
    //         };
    //         let mut keys = Vec::new();
    //         let iter = tree.scan_prefix(prefix.as_bytes());
    //         for item in iter {
    //             let (key, _value) = item?;
    //             let key_str = String::from_utf8_lossy(&key).to_string();
    //             if let Some(next_segment) = key_str
    //                 .strip_prefix(&prefix)
    //                 .and_then(|s| s.split('/').next())
    //             {
    //                 if ! keys.contains(&next_segment.to_string()) {
    //                     keys.push(next_segment.to_string());
    //                 }
    //             }
    //         }
    //         self.current_keys = keys;
    //     }
    //     Ok(())
    // }

    // pub fn has_subkeys(&self, index: usize ) -> bool {
    //     if let Some(tree) = &self.current_tree {
    //         let key = &self.current_keys[index];
    //         let path = if self.current_path.len() > 0 {
    //                 format!("{}/{}/", self.current_path.join("/"), key)
    //             } else {
    //                 format!("{}/", key)
    //             };
    //         return tree.scan_prefix(path.as_bytes()).count() > 0;
    //     }
    //     return false;
    // }

    // pub fn select_key(&mut self, index: usize) -> Result<()> {
    //     if self.has_subkeys(index) {
    //         self.current_path.push(self.current_keys[index].to_owned());
    //         self.refresh_keys()?;
    //     }
    //     Ok(())
    // }

    pub fn get_value(&mut self, index: usize) -> Result<()> {
        if let Some(tree) = &self.current_tree {
            let key = self.current_key_range.1[index].to_owned();
            let mut new_path = self.current_path.clone();
            new_path.push(key.key);
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
            self.current_value = None;
        } 
        Ok(())
    }

}
