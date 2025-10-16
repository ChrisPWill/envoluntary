mod cache;
mod util;
mod watches;

use std::env;
use std::process;

use crate::cache::{freshen_cache, preflight, print_cur_cache};
use crate::util::{get_args, hash_files, is_file};
use crate::watches::get_extant;

fn main() {
    let args: Vec<String> = env::args().collect();

    match get_args(&args) {
        Ok((layout_directory, flake_specifier, other_args)) => match preflight(&layout_directory) {
            Ok(()) => match get_extant() {
                Ok(watches) => {
                    let paths = watches
                        .iter()
                        .map(|watch| watch.path.clone())
                        .collect::<Vec<_>>();

                    let hash = match hash_files(paths) {
                        Ok(hsh) => hsh,
                        Err(msg) => {
                            eprintln!("{}", msg);
                            process::exit(1);
                        }
                    };

                    let profile = format!("{}/flake-profile-{}", layout_directory, hash);
                    let profile_rc = format!("{}.rc", profile);

                    match (is_file(&profile_rc), is_file(&profile)) {
                        (true, true) => {
                            let profile_rc_metadata = std::fs::metadata(&profile_rc)
                                .expect("Failed to get profile_rc metadata");
                            let profile_rc_mtime = profile_rc_metadata
                                .modified()
                                .expect("Failed to get modified time")
                                .duration_since(std::time::UNIX_EPOCH)
                                .expect("Time went backwards")
                                .as_secs();

                            let all_older = watches
                                .iter()
                                .all(|watch| watch.modtime <= profile_rc_mtime as i64);

                            if all_older {
                                print_cur_cache(&profile_rc);
                            } else {
                                freshen_cache(
                                    &layout_directory,
                                    &hash,
                                    &flake_specifier,
                                    &other_args,
                                );
                            }
                        }
                        _ => {
                            freshen_cache(&layout_directory, &hash, &flake_specifier, &other_args);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("{}", e);
                    process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("{}", e);
                process::exit(1);
            }
        },
        Err(()) => {
            eprintln!(
                "{}  <layout_directory> <flake specifier> <...args>",
                args[0]
            );
            process::exit(1);
        }
    }
}
