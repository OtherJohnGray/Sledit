// file src/example.rs
use anyhow::{Result, bail};
use serde::{Serialize, Deserialize};
use std::path::Path;
use indicatif::{ProgressBar, ProgressStyle, MultiProgress};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Serialize, Deserialize)]
struct ExampleData {
    key_id: String,
    timestamp: String,
    description: String,
    tags: Vec<String>,
    metrics: Metrics,
    logs: Vec<LogEntry>,
    status: Status,
}

#[derive(Serialize, Deserialize)]
struct Metrics {
    cpu_usage: f64,
    memory_mb: f64,
    disk_io_mbps: f64,
    network_mbps: f64,
    latency_ms: f64,
}

#[derive(Serialize, Deserialize)]
struct LogEntry {
    level: String,
    component: String,
    message: String,
    details: String,
}

#[derive(Serialize, Deserialize)]
struct Status {
    state: String,
    health: String,
    last_update: String,
    dependencies: Vec<String>,
    configuration: std::collections::HashMap<String, String>,
}

pub fn create_example_db(path: &Path, running: Arc<AtomicBool>) -> Result<()> {
        if path.exists() {
        bail!("Error: Path {} already exists", path.display());
    }

    let db = sled::open(path)?;
    let delimiters = ["/", "\\", ":", "::", ",", ".", "-", "_"];

    let multi_progress = MultiProgress::new();
    let total_entries = (50u64 * 50 * 50 * delimiters.len() as u64) as u64;
    
    let main_pb = multi_progress.add(ProgressBar::new(total_entries));
    main_pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({per_sec}, {eta})")
        .unwrap()
        .progress_chars("#>-"));
    
    main_pb.set_message("Creating database entries...");


    for (tree_idx, delimiter) in delimiters.iter().enumerate() {
        let tree_name = format!("tree{}", tree_idx + 1);
        let tree = db.open_tree(tree_name)?;

        let tree_pb = multi_progress.add(ProgressBar::new(50));
        tree_pb.set_style(ProgressStyle::default_bar()
            .template("{prefix:.bold.dim} {spinner:.green} [{wide_bar:.cyan/blue}] {pos}/{len}")
            .unwrap()
            .progress_chars("#>-"));
        tree_pb.set_prefix(format!("Tree {} ({})", tree_idx + 1, delimiter));

        for i in 1..=50 {
            if !running.load(Ordering::SeqCst) {
                println!("\nOperation cancelled. Database may be incomplete.");
                return Ok(());
            }            
            tree_pb.set_position(i);
            for j in 1..=50 {
                for k in 1..=50 {
                    let key = format!("key{1}{0}subkey{2}{0}subsubkey{3}", 
                        delimiter, i, j, k);
                    
                    let data = ExampleData {
                        key_id: format!("value{}{}{}{}", i, delimiter, j, k),
                        timestamp: "2024-04-09T12:34:56Z".to_string(),
                        description: "This is a long description that will require horizontal scrolling to view completely. It contains detailed information about the test data entry.".to_string(),
                        tags: vec!["test".to_string(), "example".to_string(), "generated".to_string()],
                        metrics: Metrics {
                            cpu_usage: 45.7,
                            memory_mb: 1234.5,
                            disk_io_mbps: 89.3,
                            network_mbps: 156.7,
                            latency_ms: 23.4,
                        },
                        logs: vec![
                            LogEntry {
                                level: "INFO".to_string(),
                                component: "TestGenerator".to_string(),
                                message: "Generated test entry".to_string(),
                                details: "Additional details about the test entry generation process that spans multiple lines\nto demonstrate vertical scrolling capabilities.".to_string(),
                            },
                            LogEntry {
                                level: "DEBUG".to_string(),
                                component: "DataValidator".to_string(),
                                message: "Validated entry structure".to_string(),
                                details: "Performed structural validation of the generated test data\nwith multiple validation rules applied.".to_string(),
                            },
                        ],
                        status: Status {
                            state: "ACTIVE".to_string(),
                            health: "HEALTHY".to_string(),
                            last_update: "2024-04-09T12:34:56Z".to_string(),
                            dependencies: vec!["system1".to_string(), "system2".to_string()],
                            configuration: [
                                ("param1".to_string(), "value1".to_string()),
                                ("param2".to_string(), "value2".to_string()),
                            ].into_iter().collect(),
                        },
                    };

                    // Serialize based on position in the tree
                    let serialized = match (i + j + k) % 4 {
                        0 => ron::ser::to_string_pretty(&data, ron::ser::PrettyConfig::new())?,
                        1 => serde_json::to_string_pretty(&data)?,
                        2 => serde_yaml::to_string(&data)?,
                        _ => toml::to_string_pretty(&data)?,
                    };

                    tree.insert(key.as_bytes(), serialized.as_bytes())?;
                    main_pb.inc(1);
                }
            }
        }
        tree_pb.finish_with_message("Done");
    }

    db.flush()?;
    println!("Created example database at {}", path.display());
    Ok(())
}