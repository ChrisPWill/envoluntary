mod versions;

use std::fs;
use std::io;
use std::process;

use crate::{util, watches};

pub fn read_file(f_path: &str) -> io::Result<String> {
    fs::read_to_string(f_path)
}

pub fn write_file(f_path: &str, content: &str) -> io::Result<()> {
    fs::write(f_path, content)
}

pub fn print_cur_cache(profile_rc: &str) {
    match read_file(profile_rc) {
        Ok(content) => print!("{}", content),
        Err(e) => eprintln!("Failed to read profile: {}", e),
    }
}

pub fn clean_old_gcroots(layout_dir: &str) {
    let _ = fs::remove_dir_all(layout_dir);
    let flake_inputs_path = format!("{}/flake-inputs/", layout_dir);
    fs::create_dir_all(flake_inputs_path).expect("Failed to create flake-inputs directory");
}

pub fn add_gcroot(store_path: &str, symlink: &str) -> Result<(), String> {
    match util::nix(&["build", "--out-link", symlink, store_path]) {
        (util::ExitStatus::Success, _) => Ok(()),
        _ => Err("Failed to run `nix build`!".to_string()),
    }
}

pub fn freshen_cache(layout_dir: &str, hash: &str, flake_specifier: &str, other_args: &[String]) {
    clean_old_gcroots(layout_dir);
    let tmp_profile = format!("{}flake-tmp-profile.{}", layout_dir, process::id());

    let mut pde_args = vec![
        "print-dev-env".to_string(),
        "--profile".to_string(),
        tmp_profile.clone(),
        flake_specifier.to_string(),
    ];
    pde_args.extend_from_slice(other_args);

    let pde_args_refs: Vec<&str> = pde_args.iter().map(|s| s.as_str()).collect();
    let (exit_code, stdout_content) = util::nix(&pde_args_refs);

    let profile = format!("{}/flake-profile-{}", layout_dir, hash);
    let profile_rc = format!("{}.rc", profile);

    match exit_code {
        util::ExitStatus::Success => {
            if let Err(e) = write_file(&profile_rc, &stdout_content) {
                eprintln!("Failed to write profile: {}", e);
                process::exit(1);
            }

            match add_gcroot(&tmp_profile, &profile) {
                Ok(()) => {
                    let _ = fs::remove_file(&tmp_profile);
                    let flake_input_cache_path = format!("{}/flake-inputs/", layout_dir);
                    let flake_inputs = watches::get_input_paths();

                    for input in flake_inputs {
                        let store_path = format!("/nix/store/{}", input);
                        let symlink_path = format!("{}{}", flake_input_cache_path, input);
                        if let Err(err) = add_gcroot(&store_path, &symlink_path) {
                            eprintln!("Failed creating flake-input gcroot: {}", err);
                        }
                    }
                    print_cur_cache(&profile_rc);
                }
                Err(err) => {
                    eprintln!("Failed creating gcroot: {}", err);
                    process::exit(1);
                }
            }
        }
        _ => {
            eprintln!("Failed evaluating flake");
            process::exit(1);
        }
    }
}

pub fn preflight(layout_directory: &str) -> Result<(), String> {
    match (
        versions::preflight_versions(),
        util::is_directory(layout_directory),
    ) {
        (Ok(_), true) => Ok(()),
        (Ok(_), false) => {
            fs::create_dir_all(layout_directory)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
            Ok(())
        }
        (err, _) => err,
    }
}
