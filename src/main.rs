mod config;
mod utils;

use std::collections::HashMap;
use std::convert::From;
use std::fmt::{self, Display};
use std::io;

use pop_launcher::{PluginResponse, PluginSearchResult, Request};
use urlencoding::encode;

static PLUGIN_PREFIX: &str = "!";
static BANGS_PLACEHOLDER: &str = "{{{s}}}";
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
    /// Contains the bangs database.
    db: config::Database,

    /// The cache for the items to be searched.
    cache: Vec<(String, String)>,

    /// Metadata relating to the user input.
    search: BangsQuery,

    /// The search result.
    /// The string is assumed to be the trigger word of one of the bangs from the database.
    results: HashMap<u32, String>,

    /// The standard output stream.
    out: io::Stdout,
}

impl Default for App {
    fn default() -> Self {
        let db = config::load();
        let mut cache = Vec::new();

        db.iter()
            .for_each(|(_k, bang)| cache.push((bang.trigger.clone(), bang.format())));

        Self {
            db,
            cache,
            out: io::stdout(),
            search: BangsQuery::default(),
            results: HashMap::new(),
        }
    }
}

impl App {
    /// Opens the selected bangs and its URL.
    /// Upon activation, it also closes the launcher.
    fn activate(&mut self, _id: u32) {
        let query = self.search.query.join(" ");
        let encoded_query = encode(&query);
        for bang_trigger in &self.search.bangs {
            if let Some(bang) = self.db.get(bang_trigger) {
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
            // Refresh the list.
            self.results.clear();

            // Set the search metadata right after the plugin is enabled.
            // This is just to make input processing easier.
            self.search = BangsQuery::from(search.to_string());

            // We're just going to base our search from the last bang since it is the most
            // practical way to do so.
            // We'll figure how to make it smarter by giving responses to recent edits later.
            let query = self.get_query();

            // TODO: Improve the performance of this function, pls.
            //       This is just too slow.
            // The main event of this function.
            let mut id = 0;
            for (trigger, item) in &self.cache {
                if !item.contains(&query) {
                    continue;
                }

                if let Some(bang) = self.db.get(trigger) {
                    utils::send(
                        &mut self.out,
                        PluginResponse::Append(PluginSearchResult {
                            id: id as u32,
                            name: bang.name(),
                            description: bang.description(),
                            ..Default::default()
                        }),
                    );
                    self.results.insert(id, bang.trigger.clone());
                    id += 1;
                }
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
                match self.has_query_bang_search() {
                    true => self.search.query.pop(),
                    _ => None,
                };

                self.search.bangs.pop();
                self.search.bangs.push(bang.trigger.clone());
                utils::send(&mut self.out, PluginResponse::Fill(self.search.to_string()));
            }
        }
    }

    /// Get the search query to be based.
    fn get_query(&self) -> String {
        let ss = String::new();
        let s = self.search.query.last().unwrap_or(&ss);

        match s.strip_prefix(BANG_INDICATOR) {
            Some(q) => q.to_string().clone(),
            None => self.search.bangs.last().unwrap_or(&ss).clone(),
        }
    }

    fn has_query_bang_search(&self) -> bool {
        self.search
            .query
            .last()
            .unwrap_or(&String::new())
            .starts_with(BANG_INDICATOR)
    }
}

/// Contains the data for the user input.
/// Since the plugin requires the search query in a certain format, this makes it easier to handle.
#[derive(Debug, Default)]
struct BangsQuery {
    /// An array of triggers to be opened.
    /// This came from user input as a comma-separated list of triggers (e.g., `g,ddg,yt`).
    bangs: Vec<String>,

    /// The search query.
    /// This is assumed it came from the user input.
    query: Vec<String>,
}

impl Display for BangsQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            PLUGIN_PREFIX,
            self.bangs.join(","),
            self.query.join(" "),
        )
    }
}

impl From<String> for BangsQuery {
    fn from(s: String) -> Self {
        let mut s = s.split_whitespace();
        let bangs = s
            .next()
            .unwrap_or("")
            .split(',')
            .map(|b| b.to_string())
            .collect();
        let query = s.map(|q| q.to_string()).collect();

        Self { bangs, query }
    }
}
