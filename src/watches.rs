use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::util;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Watch {
    pub exists: bool,
    pub modtime: i64,
    pub path: PathBuf,
}

pub type Watches = Vec<Watch>;

pub fn get() -> Result<Watches, String> {
    let direnv_watch_str = std::env::var("DIRENV_WATCHES")
        .map_err(|_| "DIRENV_WATCHES environment variable not set".to_string())?;

    let output = Command::new("direnv")
        .args(["show_dump", &direnv_watch_str])
        .stdout(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run direnv: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    serde_json::from_str::<Watches>(&stdout).map_err(|e| {
        format!(
            "Failed parsing watches; Has the direnv JSON shape changed? Error: {}",
            e
        )
    })
}

pub fn get_extant() -> Result<Vec<Watch>, String> {
    get().map(|watches| watches.into_iter().filter(|w| w.exists).collect())
}

fn get_path(doc: &Value) -> Option<String> {
    doc.get("path").and_then(|p| p.as_str()).map(|pth| {
        if pth.len() > 11 {
            pth[11..].to_string()
        } else {
            pth.to_string()
        }
    })
}

fn get_paths_from_doc(doc: &Value, _paths: &[String]) -> Vec<String> {
    let mut result = Vec::new();

    if let Some(p) = get_path(doc) {
        result.push(p);
    }

    if let Some(inputs) = doc.get("inputs").and_then(|i| i.as_object()) {
        for (_k, v) in inputs {
            let sub_paths = get_paths_from_doc(v, &[]);
            result.extend(sub_paths);
        }
    }

    result
}

pub fn get_input_paths() -> Vec<String> {
    match util::nix(&["flake", "archive", "--json", "--no-write-lock-file"]) {
        (util::ExitStatus::Success, output) => match serde_json::from_str::<Value>(&output) {
            Ok(json) => get_paths_from_doc(&json, &[]),
            Err(_) => {
                eprintln!(
                    "Failed to parse output of `nix flake archive --json`. Ignoring flake inputs."
                );
                Vec::new()
            }
        },
        _ => {
            eprintln!("Failed to run `nix flake archive --json`. Ignoring flake inputs.");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::watches::{get_path, get_paths_from_doc};

    #[test]
    fn test_get_path_removes_prefix() {
        let input = json!({
            "path": "aaaaaaaaaaabbbbb"
        });
        let result = get_path(&input);
        assert_eq!(result, Some("bbbbb".to_string()));
    }

    #[test]
    fn test_get_paths_from_doc() {
        let input = json!({
            "path": "aaaaaaaaaaabbbbb",
            "inputs": {
                "foo": {
                    "path": "aaaaaaaaaaaccccc",
                    "inputs": {
                        "bar": {
                            "path": "aaaaaaaaaaaddddd",
                            "inputs": {}
                        }
                    }
                }
            }
        });
        let result = get_paths_from_doc(&input, &[]);
        assert_eq!(
            result,
            vec![
                "bbbbb".to_string(),
                "ccccc".to_string(),
                "ddddd".to_string()
            ]
        );
    }
}
