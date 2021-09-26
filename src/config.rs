use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use crate::utils::{find, local_plugin_dir};

use serde::Deserialize;

static DEFAULT_BANGS_DATABASE: &str = "https://duckduckgo.com/bang.js";

/// The bangs database.
/// It's based from how [Duckduckgo's own database](https://duckduckgo.com/bang.js) is structured.
pub type Database = HashMap<String, Bang>;

/// A bang object directly based from the Duckduckgo's database.
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
///
/// It will also take care of automatically downloading the default database in the local plugin
/// path if there's no database found in the plugin paths.
pub fn load() -> Database {
    let mut db = Database::default();

    // Finding all `db.json` files and merging the databases together.
    // I'm not sure if this is ideal so expect this will change in the future.
    // It'll most likely change to override the top-level plugin path (e.g., the user plugins
    // directory over the distribution plugin path).
    let mut paths: Vec<PathBuf> = find("bangs", "db.json").collect();

    // Download Duckduckgo's bang database when there's no such database anywhere.
    // We'll download it in the home directory (since that is just the safest location for it).
    // Specifically at `LOCAL` variable given from the `pop_launcher` crate.
    // Being synchronous makes it a bit harder to handle this well.
    //
    // We also use `curl` from the command line instead of using an HTTP client because I just want
    // to save some bytes lel.
    if paths.is_empty() {
        eprintln!("[bangs] no found database files, downloading the default database");
        match Command::new("curl")
            .arg("--silent")
            .arg(DEFAULT_BANGS_DATABASE)
            .output()
        {
            Ok(process) => {
                let mut db_path = local_plugin_dir("bangs");
                db_path.push("db.json");

                if let Ok(mut file) = File::create(&db_path) {
                    match file.write(&process.stdout) {
                        Ok(_) => eprintln!("[bangs] default database file successfully downloaded"),
                        Err(e) => eprintln!("[bangs] not able to write in to file: {}", e),
                    }
                }

                paths.push(db_path);
            }
            Err(err) => eprintln!("[bangs] default database download failed: {}", err),
        }
    }

    // Merge all databases.
    for path in find("bangs", "db.json") {
        let string = match std::fs::read_to_string(&path) {
            Ok(string) => string,
            Err(why) => {
                eprintln!("[bangs] failed to read config: {}", why);
                continue;
            }
        };

        match serde_json::from_str::<Vec<Bang>>(&string) {
            Ok(config) => {
                for bang in config {
                    db.insert(bang.trigger.clone(), bang);
                }
            }
            Err(why) => eprintln!("[bangs] failed to deserialize config: {}", why),
        }
    }

    db
}
