use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use crate::utils::{find, local_plugin_dir};

use serde::{Deserialize, Serialize};

pub static BANG_FILENAME: &str = "db.json";
pub static PLUGIN_CONFIG_FILENAME: &str = "config.json";

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
/// It will only accept one database at a time.
/// If it has no database or forcing the download, it will use the database from the home directory
/// instead.
///
/// It will also take care of automatically downloading the default database in the local plugin
/// path if there's no database found in the plugin paths.
pub fn load(app_config: &AppConfig) -> Database {
    let mut db = Database::default();

    // Finding all `db.json` files, taking only the local (as much as possible) plugin path.
    let mut db_path: PathBuf = match find("bangs", BANG_FILENAME).take(1).next() {
        Some(p) => p,
        None => {
            let mut p = local_plugin_dir("bangs");
            p.push(BANG_FILENAME);

            p
        }
    };

    // Download Duckduckgo's bang database when there's no such database anywhere or if the app is
    // configured to force the download.
    // We'll download it in the home directory (since that is just the safest location for it).
    // Specifically at `LOCAL` variable given from the `pop_launcher` crate.
    // Being synchronous makes it a bit harder to handle this well.
    //
    // We also use `curl` from the command line instead of using an HTTP client because I just want
    // to save some bytes lel.
    if app_config.force_download || !db_path.is_file() {
        eprintln!("[bangs] forced download, downloading the configured database");
        match Command::new("curl")
            .arg("--silent")
            .arg(&app_config.db_url)
            .output()
        {
            Ok(process) => {
                // We'll force the download to the home directory since it is the safest
                // location.
                db_path = local_plugin_dir("bangs");
                db_path.push(BANG_FILENAME);

                if let Ok(mut file) = File::create(&db_path) {
                    match file.write(&process.stdout) {
                        Ok(_) => eprintln!("[bangs] default database file successfully downloaded"),
                        Err(e) => eprintln!("[bangs] not able to write in to file: {}", e),
                    }
                }
            }
            Err(err) => eprintln!("[bangs] default database download failed: {}", err),
        }
    }

    if let Ok(string) = std::fs::read_to_string(&db_path) {
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

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    /// The URL of the default database.
    /// It is to be used when there's no database file is found.
    #[serde(default = "AppConfig::default_db")]
    pub db_url: String,

    /// Indicates the maximum number of search results.
    #[serde(default = "AppConfig::max_limit")]
    pub max_limit: u64,

    /// Indicates whether to force downloading of the default database when no database file is
    /// found.
    #[serde(default = "AppConfig::force_download")]
    pub force_download: bool,

    /// A list of bangs to be used when there's no bang found from the search query.
    pub default_bangs: Vec<String>,
}

/// Plugin-specific configuration.
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            db_url: Self::default_db(),
            max_limit: Self::max_limit(),
            force_download: Self::force_download(),
            default_bangs: Vec::new(),
        }
    }
}

impl AppConfig {
    /// Loads the plugin configuration if it has one.
    ///
    /// Note this will not merge configuration.
    pub fn load() -> Self {
        let mut config = Self::default();

        // We'll also take only one.
        // Keep in mind the list of plugin paths from `pop_launcher` crate are sorted from local to
        // system-wide locations.
        if let Some(config_file) = find("bangs", PLUGIN_CONFIG_FILENAME).take(1).next() {
            if let Ok(content) = std::fs::read_to_string(config_file) {
                match serde_json::from_str::<Self>(&content) {
                    Ok(new_config) => config = new_config,
                    Err(why) => eprintln!("[bangs] failed to read config: {}", why),
                }
            }
        }

        config
    }

    // The following functions are just used for making the default values.
    fn default_db() -> String {
        "https://duckduckgo.com/bang.js".to_string()
    }

    fn max_limit() -> u64 {
        8
    }

    fn force_download() -> bool {
        false
    }
}
