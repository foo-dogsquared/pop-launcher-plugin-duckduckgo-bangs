use std::ffi::OsStr;
use std::io::{BufRead, Write};
use std::process::Command;

use pop_launcher::PluginResponse;
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
