use std::error::Error;
use std::iter::{Extend, Iterator};
use std::ops::Index;

use crate::utils::find;

use curl::easy::{Easy2, Handler, WriteError};
use serde::{Deserialize, Serialize};

pub static PLUGIN_CONFIG_FILENAME: &str = "config.json";

pub struct BytesCollector(pub Vec<u8>);

impl Handler for BytesCollector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
}

/// The bangs database.
pub struct Database {
    data: Vec<Bang>,
}

impl Database {
    /// Creates an empty database.
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Loads the database from a given URL.
    pub fn load(url: &str) -> Result<Self, Box<dyn Error>> {
        let mut db = Self::new();

        let mut handle = Easy2::new(BytesCollector(Vec::new()));
        handle.get(true)?;
        handle.accept_encoding("gzip")?;
        handle.url(url)?;
        handle.perform()?;

        // TODO: Improve error handling for this part, please.
        // Don't make it panic,
        assert_eq!(handle.response_code().unwrap(), 200);
        let contents = handle.get_ref();

        db.data = serde_json::from_slice::<Vec<Bang>>(&contents.0)?;
        db.data.sort_by(|a, b| b.partial_cmp(a).unwrap());
        Ok(db)
    }

    /// Returns an iterator visiting all values inside of the database.
    pub fn iter(&self) -> impl Iterator<Item = &Bang> + '_ {
        self.data.iter()
    }

    /// Retains all elements from a given predicate.
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Bang) -> bool,
    {
        self.data.retain(f)
    }

    /// Get the bang corresponding to the trigger.
    pub fn get(&self, trigger: impl ToString) -> Option<&Bang> {
        self.data
            .iter()
            .filter(|bang| bang.trigger == trigger.to_string())
            .take(1)
            .next()
    }
}

impl<'a> Index<&'a str> for Database {
    type Output = Bang;

    fn index(&self, trigger: &'a str) -> &Self::Output {
        self.get(trigger).unwrap()
    }
}

impl Extend<Bang> for Database {
    fn extend<T: IntoIterator<Item = Bang>>(&mut self, iter: T) {
        self.data.extend(iter);
    }
}

/// A bang object directly based from the Brave's bang database schema.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
pub struct Bang {
    /// The bang used to trigger the search.
    #[serde(alias = "bang")]
    pub trigger: String,

    /// The URL directly pointing to the query alongside a hint where the search query could be
    /// inserted.
    pub url: String,

    #[serde(alias = "meta")]
    pub metadata: BangMetadata,

    /// The title of the website.
    pub title: String,

    pub category: String,

    pub sub_category: String,
}

impl Bang {
    /// The full format of the bang.
    /// Useful for searching if the query is found on the bang data.
    pub fn format(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.trigger, self.title, self.category, self.sub_category, self.metadata.netloc
        )
    }

    /// The launcher item name for the bang.
    pub fn name(&self) -> String {
        format!(
            "{} | {} ({})",
            self.trigger, self.title, self.metadata.netloc
        )
    }

    /// The launcher item description for the bang.
    pub fn description(&self) -> String {
        format!("{} > {}", self.category, self.sub_category)
    }
}

/// Contains various metadata to the URL of the referred bang. This is particularly useful to point
/// to general locations of the bang. However, since it doesn't contain hints where the search
/// query could be inserted, it can't replicate the URL specifically used for bangs functionality.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BangMetadata {
    /// The network location of the URL.
    pub netloc: String,

    /// The full hostname of the URL.
    pub hostname: Option<String>,

    /// Indicates the location of the favicon for the bang location. The value could be empty which
    /// would be handled by the plugin with various fallbacks.
    pub favicon: Option<String>,

    /// The scheme part of the URL.
    pub scheme: String,

    /// The path of the search page delimited with `>`.
    ///
    /// A bang metadata with a path of `> search > page` will refer to the
    /// `<scheme>://<netloc>/search/page` with `>` substituted with the path delimiter and trimmed
    /// whitespaces.
    pub path: String,
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

    /// A list of bangs to be used when there's no bang found from the search query.
    pub default_bangs: Vec<String>,

    /// Specify whether the database should remove the duplicate bangs with the same URL.
    #[serde(default = "AppConfig::unique_bangs")]
    pub unique_bangs: bool,
}

/// Plugin-specific configuration.
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            db_url: Self::default_db(),
            max_limit: Self::max_limit(),
            default_bangs: Vec::new(),
            unique_bangs: Self::unique_bangs(),
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
        "https://search.brave.com/bang/data".to_string()
    }

    fn max_limit() -> u64 {
        12
    }

    fn unique_bangs() -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_bang() -> Bang {
        Bang {
            url: "https://duckduckgo.com/?q={{{s}}}".to_string(),
            title: "Duckduckgo".to_string(),
            trigger: "ddg".to_string(),
            category: "Web".to_string(),
            sub_category: "Search".to_string(),
            metadata: BangMetadata {
                favicon: Some("https://imgs.search.brave.com/keBhPmRqAbkFJbssC8z36MLAvxORzMIgUfRTzbAJhis/rs:fit:32:32:1/g:ce/aHR0cDovL2Zhdmlj/b25zLnNlYXJjaC5i/cmF2ZS5jb20vaWNv/bnMvZTUxYTE2NmI0/MTNjOGYzMjMwMjk3/MGNkNTA5MjhkODYx/MGVkZTJhMzFkYTQ3/MGVlODY2M2I2OGU1/ODZkNGQyMS9kdWNr/ZHVja2dvLmNvbS8".to_string()),
                hostname: "duckduckgo.com".to_string(),
                netloc: "duckduckgo.com".to_string(),
                scheme: "https".to_string(),
                path: "s > {{query}}".to_string(),
            },
        }
    }

    #[test]
    fn alphabetically_first_url_of_bang_should_be_greater() {
        let greater_bang = mock_bang();
        let lesser_bang = Bang {
            url: "https://buckduckgo.com/?q={{{s}}}".to_string(),
            ..greater_bang.clone()
        };

        assert!(greater_bang > lesser_bang);
    }

    #[test]
    fn two_identical_bangs_should_be_equal() {
        // We're not cloning here to show that two different instances with identical data is
        // really equal.
        let first_bang = mock_bang();
        let second_bang = mock_bang();

        assert!(first_bang == second_bang);
    }

    #[test]
    fn alphabetically_first_trigger_of_bang_should_be_greater() {
        let greater_bang = mock_bang();
        let lesser_bang = Bang {
            trigger: "bdg".to_string(),
            ..greater_bang.clone()
        };

        assert!(greater_bang > lesser_bang);
    }
}
