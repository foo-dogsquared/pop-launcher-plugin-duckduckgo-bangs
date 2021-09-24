use std::collections::HashMap;
use std::iter::Iterator;
use std::path::PathBuf;

use pop_launcher::plugin_paths;
use serde::Deserialize;

/// The bangs database.
/// It's based from how [Duckduckgo's own database](https://duckduckgo.com/bang.js) is structured.
pub type Database = HashMap<String, Bang>;

#[derive(Debug, Deserialize)]
pub struct Bang {
    #[serde(alias = "c", default)]
    pub category: String,

    #[serde(alias = "sc", default)]
    pub subcategory: String,

    #[serde(alias = "d")]
    pub domain: String,

    #[serde(alias = "r", default)]
    pub relevance: u64,

    #[serde(alias = "t")]
    pub trigger: String,

    #[serde(alias = "s")]
    pub name: String,

    #[serde(alias = "u")]
    pub url: String,
}

impl Bang {
    /// The full format of the bang.
    /// Useful for searching if the query is found on the bang data.
    pub fn format(&self) -> String {
        format!(
            "{} | {} > {} | {}",
            self.trigger, self.category, self.subcategory, self.domain
        )
    }

    /// The launcher item name for the bang.
    pub fn name(&self) -> String {
        format!("{} | {} ({})", self.trigger, self.name, self.domain)
    }

    /// The launcher item description for the bang.
    pub fn description(&self) -> String {
        format!("{} > {}", self.category, self.subcategory)
    }
}

/// Loads the database (`db.json`) from the plugin paths.
/// Please take note it merges the database from each plugin path.
pub fn load() -> Database {
    let mut db = Database::default();

    // Finding all `db.json` files and merging the databases together.
    // I'm not sure if this is ideal so expect this will change in the future.
    // It'll most likely change to override the top-level plugin path (e.g., the user plugins
    // directory over the distribution plugin path).
    for path in find("bangs", "db.json") {
        let string = match std::fs::read_to_string(&path) {
            Ok(string) => string,
            Err(why) => {
                eprintln!("failed to read config: {}", why);
                continue;
            }
        };

        match serde_json::from_str::<Vec<Bang>>(&string) {
            Ok(config) => {
                for bang in config {
                    db.insert(bang.trigger.clone(), bang);
                }
            }
            Err(why) => eprintln!("failed to deserialize config: {}", why),
        }
    }

    db
}

/// Find `file` inside of the plugin `name` in each of the plugin paths.
/// Useful for finding configuration files.
pub fn find<'a>(name: &'a str, file: &'a str) -> impl Iterator<Item = PathBuf> + 'a {
    plugin_paths()
        .filter_map(|path| path.read_dir().ok())
        .flat_map(move |dir| {
            dir.filter_map(Result::ok).filter_map(move |entry| {
                if entry.file_name() == name {
                    let path = entry.path();
                    let file_path = path.join(file);
                    if file_path.exists() {
                        return Some(file_path);
                    }
                }

                None
            })
        })
}
