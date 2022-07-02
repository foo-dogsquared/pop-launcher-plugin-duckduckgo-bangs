mod config;
mod utils;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::config::{AppConfig, Bang, BytesCollector, Database};

use pop_launcher::{json_input_stream, IconSource, PluginResponse, PluginSearchResult, Request};
use rayon::prelude::*;
use tokio_stream::{self, StreamExt};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use urlencoding::encode;

// TODO:
// * Plugin-side relevance.
// ** This pretty much destroys the usability of the plugin since Duckduckgo's database has them
// inside of the database. We'll have to make some compromises by creating a built-in relevance
// database inside of the plugin and appending it to the user-side relevance database.

// In any case, we can also move the following static variables into a plugin-specific configuration file.
/// The prefix for activating the plugin.
static PLUGIN_PREFIX: &str = "!";

/// The placeholder string for the search query.
static BANGS_PLACEHOLDER: &str = "{query}";

/// The prefix for indicating an inline bang search.
static BANG_INDICATOR: &str = "!";

/// The plugin name.
static PLUGIN_NAME: &str = "bangs";

#[tokio::main]
async fn main() {
    let mut app = App::default();
    let mut requests = json_input_stream(tokio::io::stdin());
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::ERROR)
        .finish();

    while let Some(result) = requests.next().await {
        match result {
            Ok(request) => match request {
                Request::Activate(id) => app.activate(id).await,
                Request::Search(query) => app.search(query).await,
                Request::Complete(id) => app.complete(id).await,
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

    cache_dir: PathBuf,

    /// Metadata relating to the user input.
    search: Vec<String>,

    /// The search result where it holds the ID returned from Pop launcher.
    /// The string is assumed to be the trigger word of one of the bangs from the database.
    results: HashMap<u32, String>,

    /// The standard output stream.
    out: tokio::io::Stdout,
}

impl Default for App {
    fn default() -> Self {
        let config = AppConfig::load();

        let mut db = Database::load(&config.db_url).unwrap();
        let mut cache = Vec::new();

        // TODO: Handle this better.
        let mut cache_dir = dirs::cache_dir().expect("user doesn't have a cache directory");
        cache_dir.push("pop-launcher");
        cache_dir.push(PLUGIN_NAME);

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
            cache_dir,
            config,
            out: tokio::io::stdout(),
            search: Vec::new(),
            results: HashMap::new(),
        }
    }
}

impl App {
    /// Opens the selected bangs and its URL.
    /// Upon activation, it also closes the launcher.
    async fn activate(&mut self, _id: u32) {
        let query = self.get_search_query();
        let encoded_query = encode(&query);
        let mut bangs_from_query = self.get_bangs_from_search_query();

        if bangs_from_query.is_empty() {
            bangs_from_query.append(&mut self.config.default_bangs.clone());
        }

        for bang_trigger in bangs_from_query {
            if let Some(bang) = self.db.get(&bang_trigger) {
                let url = bang.url.clone().replace(BANGS_PLACEHOLDER, &encoded_query);
                let _ = open::that(url);
            }
        }

        crate::utils::send(&mut self.out, PluginResponse::Close).await;
    }

    /// Searches the bangs database with the given query.
    /// The search results are then sent out as a plugin response and stored as one of the queries.
    async fn search(&mut self, query: String) {
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

                // Getting a new hashmap for the results.
                // Until we can find an elegant way to just use the original map, this will do for now.
                let mut results = HashMap::new();
                let mut id = 0;

                let items = self
                    .cache
                    .iter()
                    .filter(|(_trigger, item)| item.contains(&query))
                    .filter_map(|(trigger, _item)| self.db.get(trigger))
                    .take(self.config.max_limit as usize);

                for bang in items {
                    // This also doubles as a counter.
                    id += 1;

                    crate::utils::send(
                        // This is a terrible solution, WTF.
                        &mut tokio::io::stdout(),
                        PluginResponse::Append(PluginSearchResult {
                            id: id as u32,
                            name: bang.name(),
                            description: bang.description(),
                            ..Default::default()
                        }),
                    )
                    .await;

                    results.insert(id, bang.trigger.clone());
                }

                // Send an extra launcher item if there's no search result.
                // We'll just reuse the `id` variable for this.
                // Take note we need to make a launcher item to make activation event possible.
                if id == 0 {
                    self.send_empty_launcher_item().await;
                }

                self.results = results;
            } else {
                self.send_empty_launcher_item().await;
            }
        }

        crate::utils::send(&mut self.out, PluginResponse::Finished).await;
    }

    /// Provide the completion with the selected item.
    /// In this case, it should respond with the trigger word of the entry.
    async fn complete(&mut self, id: u32) {
        // If the given ID is valid from the search results.
        if let Some(trigger) = self.results.get(&id) {
            // If the associated trigger is in the database.
            if let Some(bang) = self.db.get(trigger) {
                // For the best user experience, we just delete the last element first and update the query.
                self.search.pop();
                self.search
                    .push(BANG_INDICATOR.to_string() + &bang.trigger.clone());
                crate::utils::send(
                    &mut self.out,
                    PluginResponse::Fill(format!("{} {}", PLUGIN_PREFIX, &self.search.join(" "))),
                )
                .await;
            }
        }
    }

    fn get_bangs_from_search_query(&self) -> Vec<String> {
        self.search
            .par_iter()
            .filter_map(|q| q.strip_prefix(BANG_INDICATOR))
            .map(|b| b.to_string())
            .collect()
    }

    /// Get the search query excluding the bangs.
    fn get_search_query(&self) -> String {
        self.search
            .par_iter()
            .filter(|q| !q.starts_with(BANG_INDICATOR))
            .map(|q| q.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }

    async fn send_empty_launcher_item(&mut self) {
        crate::utils::send(
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
