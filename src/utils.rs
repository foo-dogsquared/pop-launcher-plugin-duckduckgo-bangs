use std::iter::Iterator;
use std::path::PathBuf;

use pop_launcher::{plugin_paths, PluginResponse};
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub async fn send<W: AsyncWrite + Unpin>(tx: &mut W, response: PluginResponse) {
    if let Ok(mut bytes) = serde_json::to_string(&response) {
        bytes.push('\n');
        let _ = tx.write_all(bytes.as_bytes()).await;
    }
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
