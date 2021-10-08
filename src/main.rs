mod config;
mod utils;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use crate::config::{AppConfig, Database, BANG_FILENAME};
use crate::utils::{find, local_plugin_dir};

use pop_launcher::{PluginResponse, PluginSearchResult, Request};
use urlencoding::encode;

// In any case, we can also move the following static variables into a plugin-specific configuration file.
/// The prefix for activating the plugin.
static PLUGIN_PREFIX: &str = "!";

/// The placeholder string for the search query.
static BANGS_PLACEHOLDER: &str = "{{{s}}}";

/// The prefix for indicating an inline bang search.
static BANG_INDICATOR: &str = "!";

fn main() {
    let mut app = App::default();
    let stdin = io::stdin();
    let requests = utils::json_input_stream(stdin.lock());

    for result in requests {
        match result {
            Ok(request) => match request {
                Request::Activate(id) => app.activate(id),
                Request::Search(query) => app.search(query),
                Request::Complete(id) => app.complete(id),
                Request::Exit => break,
                _ => (),
            },
            Err(why) => eprintln!("malformed JSON input: {}", why),
        }
    }
}

struct App {
    /// Plugin-specific configuration.
    config: AppConfig,

    /// Contains the bangs database.
    db: Database,

    /// The cache for the items generated from the database.
    /// This is where search operations should go.
    /// Ideally, this should be updated along with the database that will then generate the cache.
    cache: Vec<(String, String)>,

    /// Metadata relating to the user input.
    search: Vec<String>,

    /// The search result where it holds the ID returned from Pop launcher.
    /// The string is assumed to be the trigger word of one of the bangs from the database.
    results: HashMap<u32, String>,

    /// The standard output stream.
    out: io::Stdout,
}

impl Default for App {
    fn default() -> Self {
        let config = AppConfig::load();

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
        if config.force_download || !db_path.is_file() {
            eprintln!("[bangs] forced download, downloading the configured database");
            match Command::new("curl")
                .arg("--silent")
                .arg(&config.db_url)
                .output()
            {
                Ok(process) => {
                    // We'll force the download to the home directory since it is the safest
                    // location.
                    db_path = local_plugin_dir("bangs");
                    db_path.push(BANG_FILENAME);

                    if let Ok(mut file) = File::create(&db_path) {
                        match file.write(&process.stdout) {
                            Ok(_) => {
                                eprintln!("[bangs] default database file successfully downloaded")
                            }
                            Err(e) => eprintln!("[bangs] not able to write in to file: {}", e),
                        }
                    }
                }
                Err(err) => eprintln!("[bangs] default database download failed: {}", err),
            }
        }

        let mut db = Database::load(&db_path);
        let mut cache = Vec::new();

        // Generating the cache from the database.
        db.iter().for_each(|bang| {
            cache.push((bang.trigger.clone(), bang.format().to_lowercase()));
        });

        if config.unique_bangs {
            let mut urls: HashSet<String> = HashSet::new();
            db.retain(move |b| urls.insert(b.url.clone()));
        }

        Self {
            db,
            cache,
            config,
            out: io::stdout(),
            search: Vec::new(),
            results: HashMap::new(),
        }
    }
}

impl App {
    /// Opens the selected bangs and its URL.
    /// Upon activation, it also closes the launcher.
    fn activate(&mut self, _id: u32) {
        let query = self.get_search_query();
        let encoded_query = encode(&query);
        let mut bangs_from_query = self.get_bangs_from_search_query();

        if bangs_from_query.is_empty() {
            bangs_from_query.append(&mut self.config.default_bangs.clone());
        }

        for bang_trigger in bangs_from_query {
            if let Some(bang) = self.db.get(&bang_trigger) {
                let url = bang.url.clone().replace(BANGS_PLACEHOLDER, &encoded_query);
                utils::xdg_open(url);
            }
        }
        utils::send(&mut self.out, PluginResponse::Close);
    }

    /// Searches the bangs database with the given query.
    /// The search results are then sent out as a plugin response and stored as one of the queries.
    fn search(&mut self, query: String) {
        // Only proceed if the search query is prefixed with a certain character.
        if let Some(search) = query.strip_prefix(PLUGIN_PREFIX) {
            // Set the search metadata right after the plugin is enabled.
            // This is just to make input processing easier.
            self.search = search.split_whitespace().map(|q| q.to_string()).collect();

            // We're just going to base our search from the last bang since it is the most
            // practical way to do so.
            // It will also do its work when the last part of the query is a bang to not let the
            // plugin take more than what it needs.
            // We'll figure how to make it smarter by giving responses to recent edits later.
            if let Some(query) = self
                .search
                .last()
                .unwrap_or(&String::new())
                .strip_prefix(BANG_INDICATOR)
            {
                let query = query.to_lowercase();

                // Making the standard output accessible in the closure of the following block.
                let mut out = &self.out;

                // Getting a new hashmap for the results.
                // Until we can find an elegant way to just use the original map, this will do for now.
                let mut results = HashMap::new();
                let mut id = 0;

                self.cache
                    .iter()
                    .filter(|(_trigger, item)| item.contains(&query))
                    .filter_map(|(trigger, _item)| self.db.get(trigger))
                    .take(self.config.max_limit as usize)
                    .for_each(|bang| {
                        // This also doubles as a counter.
                        id += 1;

                        utils::send(
                            &mut out,
                            PluginResponse::Append(PluginSearchResult {
                                id: id as u32,
                                name: bang.name(),
                                description: bang.description(),
                                ..Default::default()
                            }),
                        );

                        results.insert(id, bang.trigger.clone());
                    });

                // Send an extra launcher item if there's no search result.
                // We'll just reuse the `id` variable for this.
                // Take note we need to make a launcher item to make activation event possible.
                if id == 0 {
                    self.send_empty_launcher_item();
                }

                self.results = results;
            } else {
                self.send_empty_launcher_item();
            }
        }

        utils::send(&mut self.out, PluginResponse::Finished);
    }

    /// Provide the completion with the selected item.
    /// In this case, it should respond with the trigger word of the entry.
    fn complete(&mut self, id: u32) {
        // If the given ID is valid from the search results.
        if let Some(trigger) = self.results.get(&id) {
            // If the associated trigger is in the database.
            if let Some(bang) = self.db.get(trigger) {
                // For the best user experience, we just delete the last element first and update the query.
                self.search.pop();
                self.search
                    .push(BANG_INDICATOR.to_string() + &bang.trigger.clone());
                utils::send(
                    &mut self.out,
                    PluginResponse::Fill(format!("{} {}", PLUGIN_PREFIX, &self.search.join(" "))),
                );
            }
        }
    }

    fn get_bangs_from_search_query(&self) -> Vec<String> {
        self.search
            .iter()
            .filter_map(|q| q.strip_prefix(BANG_INDICATOR))
            .map(|b| b.to_string())
            .collect()
    }

    /// Get the search query excluding the bangs.
    fn get_search_query(&self) -> String {
        self.search
            .iter()
            .filter(|q| !q.starts_with(BANG_INDICATOR))
            .map(|q| q.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }

    fn send_empty_launcher_item(&mut self) {
        utils::send(
            &mut self.out,
            PluginResponse::Append(PluginSearchResult {
                id: 1,
                name: "Finish".to_string(),
                description: "Launch this item to open all your URLs".to_string(),
                ..Default::default()
            }),
        );
    }
}
