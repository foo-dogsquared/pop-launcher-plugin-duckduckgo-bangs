use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, Write};
use std::iter::Iterator;
use std::path::PathBuf;
use std::process::Command;

use dirs::home_dir;
use pop_launcher::{plugin_paths, PluginResponse, LOCAL_PLUGINS};
use serde::Deserialize;

/// Fetch the inputs from a buffer.
/// This is mostly used for reading the standard input stream.
pub fn json_input_stream<S>(input: impl BufRead) -> impl Iterator<Item = serde_json::Result<S>>
where
    S: for<'a> Deserialize<'a>,
{
    input
        .lines()
        .map(Result::unwrap)
        .map(|line| serde_json::from_str::<S>(&line))
}

/// Send `PluginResponse` to the specified stream.
pub fn send(tx: &mut impl Write, response: PluginResponse) {
    if let Ok(mut bytes) = serde_json::to_string(&response) {
        bytes.push('\n');
        let _ = tx.write_all(bytes.as_bytes());
    }
}

/// Open the given file through `xdg-open` command.
pub fn xdg_open<S: AsRef<OsStr>>(file: S) {
    let _ = Command::new("xdg-open").arg(file).spawn();
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

/// Returns the local path of the given plugin as `name`.
pub fn local_plugin_dir(name: &str) -> PathBuf {
    let mut path = fs::canonicalize(LOCAL_PLUGINS)
        .unwrap_or_else(|_| PathBuf::from("~/.local/share/pop-launcher/plugins"));

    if let Ok(p) = path.strip_prefix("~/") {
        // We'll panic here since it is expected to run it with a home directory.
        path = home_dir().expect("user does not have home dir").join(p)
    }

    path.push(name);

    path
}
