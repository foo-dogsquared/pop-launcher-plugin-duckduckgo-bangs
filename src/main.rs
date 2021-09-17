mod config;
mod utils;

use std::io;
use std::collections::HashMap;

use pop_launcher::{Request, PluginResponse, PluginSearchResult};
use urlencoding::encode;

static PLUGIN_PREFIX: &str = "!";
static BANGS_PLACEHOLDER: &str = "{{{s}}}";

fn main() {
    let mut app = App::default();
    let stdin = io::stdin();
    let requests = utils::json_input_stream(stdin.lock());

    for result in requests {
        match result {
            Ok(request) => match request {
                Request::Activate(id) => app.activate(id),
                Request::Search(query) => app.search(query),
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

    /// Contains formatted queries from the search results.
    queries: HashMap<u32, String>,

    /// The standard output stream.
    out: io::Stdout,
}

impl Default for App {
    fn default() -> Self {
        Self {
            db: config::load(),
            queries: HashMap::new(),
            out: io::stdout(),
        }
    }
}

impl App {
    /// Opens the selected item.
    /// Upon activation, it also closes the launcher.
    fn activate(&mut self, id: u32) {
        if let Some(query) = self.queries.get(&id) {
            eprintln!("got query: {}", query);
            utils::xdg_open(query);
        }

        utils::send(&mut self.out, PluginResponse::Close);
    }

    /// Searches the bangs database with the given query.
    /// The search results are then sent out as a plugin response and stored as one of the queries.
    fn search(&mut self, query: String) {
        // Since it is assumed to be given a new search query each time, we want to refresh the
        // results.
        self.queries.clear();

        // Only proceed if the search query is prefixed with a certain character.
        if let Some(search) = query.strip_prefix(PLUGIN_PREFIX) {
            // Append the bangs item to the launcher if the user prefixed the query with a certain
            // character.
            let mut search = search.split_whitespace();

            let bang_query = search.next().unwrap_or("");
            let search_query = search.collect::<Vec<&str>>().join(" ");
            let encoded_query = encode(&search_query);
            eprintln!("{}", bang_query);

            // The main event of this function.
            for (id, bang) in self.db.iter().enumerate() {
                if bang.format().contains(&bang_query) {
                    utils::send(
                        &mut self.out,
                        PluginResponse::Append(PluginSearchResult {
                            id: id as u32,
                            name: bang.name(),
                            description: bang.description(),
                            ..Default::default()
                        })
                    );

                    self.queries.insert(id as u32, bang.url.clone().replace(BANGS_PLACEHOLDER, &encoded_query));
                }
            }
        }

        utils::send(&mut self.out, PluginResponse::Finished);
    }
}
