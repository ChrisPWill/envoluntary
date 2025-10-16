use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use sha1::{Digest, Sha1};

#[derive(Debug, PartialEq)]
pub enum ExitStatus {
    Success,
    Failure(i32),
}

/// Run a process [name] with args [args], returning (exit_code, stdout text)
pub fn run_process(name: impl AsRef<str>, args: &[&str]) -> (ExitStatus, String) {
    let output = Command::new(name.as_ref())
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output();

    match output {
        Ok(output) => {
            let stdout_content = String::from_utf8_lossy(&output.stdout).to_string();
            let exit_status = if output.status.success() {
                ExitStatus::Success
            } else {
                ExitStatus::Failure(output.status.code().unwrap_or(-1))
            };
            (exit_status, stdout_content)
        }
        Err(_) => (ExitStatus::Failure(-1), String::new()),
    }
}

/// Run nix with some fixed arguments, appending a user provided set of arguments.
pub fn nix(args: &[&str]) -> (ExitStatus, String) {
    let mut nix_args = vec!["--extra-experimental-features", "nix-command flakes"];
    nix_args.extend_from_slice(args);
    run_process("nix", &nix_args)
}

pub fn is_file(pth: &str) -> bool {
    match fs::metadata(pth) {
        Ok(metadata) => metadata.is_file(),
        Err(_) => false,
    }
}

pub fn is_directory(pth: &str) -> bool {
    match fs::metadata(pth) {
        Ok(metadata) => metadata.is_dir(),
        Err(_) => false,
    }
}

pub fn hash_files(filenames: impl IntoIterator<Item = PathBuf>) -> Result<String, String> {
    let mut hasher = Sha1::new();
    let mut files_to_hash = Vec::new();

    for f in filenames {
        if f.exists() {
            files_to_hash.push(f);
        } else {
            eprintln!(
                "Cannot find file {} (cwd: {})",
                f.to_string_lossy(),
                std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "unknown".to_string())
            );
        }
    }

    if files_to_hash.is_empty() {
        return Err("No files found to hash".to_string());
    }

    for f in files_to_hash {
        match fs::read(&f) {
            Ok(content) => hasher.update(&content),
            Err(e) => {
                return Err(format!(
                    "Failed to read file {}: {}",
                    f.to_string_lossy(),
                    e
                ));
            }
        }
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

pub fn get_args(argv: &[String]) -> Result<(String, String, Vec<String>), ()> {
    let argc = argv.len();
    if argc >= 3 {
        let layout_directory = argv[1].clone();
        let flake_specifier = argv[2].clone();
        let other_args = argv[3..].to_vec();
        Ok((layout_directory, flake_specifier, other_args))
    } else {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write, path::PathBuf};

    use once_cell::sync::Lazy;
    use tempfile::NamedTempFile;

    use crate::util::{ExitStatus, get_args, hash_files, run_process};

    static SPIT_VERSION_SH: Lazy<NamedTempFile> = Lazy::new(|| {
        let mut spit_version_sh = tempfile::NamedTempFile::new().unwrap();
        writeln!(spit_version_sh.as_file_mut(), r#"echo "1.1.1";"#).unwrap();
        spit_version_sh
    });

    #[test]
    fn test_run_process_success() {
        let (exit_status, output) = run_process("true", &[]);
        assert_eq!(exit_status, ExitStatus::Success);
        assert_eq!(output, "");
    }

    #[test]
    fn test_run_process_failure() {
        let (exit_status, output) = run_process("false", &[]);
        assert!(matches!(exit_status, ExitStatus::Failure(_)));
        assert_eq!(output, "");
    }

    #[test]
    fn test_run_process_stdout() {
        let (exit_status, output) = run_process("echo", &["echoed"]);
        assert_eq!(exit_status, ExitStatus::Success);
        assert_eq!(output, "echoed\n");
    }

    #[test]
    fn test_hash_one() {
        // Note: We need to run this from the tests directory
        let result = hash_files([SPIT_VERSION_SH.path().to_path_buf()]);
        assert_eq!(
            result,
            Ok("6ead949bf4bcae230b9ed9cd11e578e34ce9f9ea".to_string())
        );
    }

    #[test]
    fn test_hash_multiple() {
        let result = hash_files([
            SPIT_VERSION_SH.path().to_path_buf(),
            SPIT_VERSION_SH.path().to_path_buf(),
        ]);
        assert_eq!(
            result,
            Ok("f109b7892a541ed1e3cf39314cd25d21042b984f".to_string())
        );
    }

    #[test]
    fn test_hash_filters_nonexistent() {
        let result = hash_files([
            SPIT_VERSION_SH.path().to_path_buf(),
            PathBuf::from("FOOBARBAZ"),
        ]);
        assert_eq!(
            result,
            Ok("6ead949bf4bcae230b9ed9cd11e578e34ce9f9ea".to_string())
        );
    }

    #[test]
    fn test_get_args_simple() {
        let args = vec![
            "000".to_string(),
            "foo".to_string(),
            "bar".to_string(),
            "oof".to_string(),
            "rab".to_string(),
            "zab".to_string(),
        ];
        let result = get_args(&args);
        assert_eq!(
            result,
            Ok((
                "foo".to_string(),
                "bar".to_string(),
                vec!["oof".to_string(), "rab".to_string(), "zab".to_string()]
            ))
        );
    }

    #[test]
    fn test_get_args_just_enough() {
        let args = vec!["000".to_string(), "foo".to_string(), "bar".to_string()];
        let result = get_args(&args);
        assert_eq!(result, Ok(("foo".to_string(), "bar".to_string(), vec![])));
    }

    #[test]
    fn test_get_args_error() {
        let args = vec!["000".to_string(), "111".to_string()];
        let result = get_args(&args);
        assert_eq!(result, Err(()));
    }
}
