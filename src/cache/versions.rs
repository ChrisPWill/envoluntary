use std::cmp::Ordering;

use regex::Regex;

use crate::util;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: i32,
    pub minor: i32,
    pub point: i32,
}

impl Version {
    /// Initialize a Version from major, minor, and point versions
    pub fn new(major: i32, minor: i32, point: i32) -> Self {
        Version {
            major,
            minor,
            point,
        }
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ {}.{}.{} }}", self.major, self.minor, self.point)
    }
}

const REQUIRED_DIRENV_VERSION: Version = Version {
    major: 2,
    minor: 21,
    point: 3,
};

const REQUIRED_NIX_VERSION: Version = Version {
    major: 2,
    minor: 10,
    point: 0,
};

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => match self.minor.cmp(&other.minor) {
                Ordering::Equal => self.point.cmp(&other.point),
                other => other,
            },
            other => other,
        }
    }
}

pub fn extract_version_number(cmd: impl AsRef<str>) -> Result<Version, String> {
    let cmd = cmd.as_ref();
    let (exit_status, stdout) = util::run_process(cmd, &["--version"]);

    match exit_status {
        util::ExitStatus::Success if !stdout.is_empty() => {
            let semver_re = Regex::new(r"([0-9]+)\.([0-9]+)\.([0-9]+)").expect("Invalid regex");

            match semver_re.captures(&stdout) {
                Some(caps) => {
                    let major = caps
                        .get(1)
                        .and_then(|m| m.as_str().parse().ok())
                        .ok_or_else(|| "Failed to parse major version".to_string())?;
                    let minor = caps
                        .get(2)
                        .and_then(|m| m.as_str().parse().ok())
                        .ok_or_else(|| "Failed to parse minor version".to_string())?;
                    let point = caps
                        .get(3)
                        .and_then(|m| m.as_str().parse().ok())
                        .ok_or_else(|| "Failed to parse point version".to_string())?;

                    Ok(Version::new(major, minor, point))
                }
                None => Err(format!(
                    "Stdout did not contain a version number for `{} --version`",
                    cmd
                )),
            }
        }
        _ => Err(format!("Failed executing '{}'", cmd)),
    }
}

pub fn is_new_enough(cur: Result<Version, String>, needed: &Version) -> Result<bool, String> {
    cur.map(|cur| cur >= *needed)
}

pub fn in_direnv() -> bool {
    // direnv sets `$direnv` to the executable's full path
    // If it is empty - we're running tests
    // If it isn't found, we're running outside direnv
    matches!(std::env::var("direnv"), Ok(val) if !val.is_empty())
}

pub fn preflight_versions() -> Result<(), String> {
    let is_nix_new_enough = is_new_enough(extract_version_number("nix"), &REQUIRED_NIX_VERSION);
    let is_direnv_new_enough =
        is_new_enough(extract_version_number("direnv"), &REQUIRED_DIRENV_VERSION);

    match (in_direnv(), is_direnv_new_enough, is_nix_new_enough) {
        (false, _, _) => Err("Not in direnv!".to_string()),
        (_, Ok(false), _) => Err("Direnv version is not new enough".to_string()),
        (_, _, Ok(false)) => Err("Nix version is not new enough".to_string()),
        (_, Err(e), _) => Err(e),
        (_, _, Err(e)) => Err(e),
        (true, Ok(true), Ok(true)) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Write, os::unix::fs::PermissionsExt};

    use super::{Version, extract_version_number, in_direnv, is_new_enough};

    #[test]
    fn test_compare_equal() {
        let a = Version::new(1, 0, 0);
        assert_eq!(a.cmp(&a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_first_major_greater() {
        let a = Version::new(2, 0, 0);
        let b = Version::new(1, 0, 0);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_first_minor_greater() {
        let a = Version::new(1, 1, 0);
        let b = Version::new(1, 0, 0);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_first_point_greater() {
        let a = Version::new(1, 0, 1);
        let b = Version::new(1, 0, 0);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_second_major_greater() {
        let a = Version::new(1, 0, 0);
        let b = Version::new(2, 0, 0);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_second_minor_greater() {
        let a = Version::new(1, 0, 0);
        let b = Version::new(1, 1, 0);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_second_point_greater() {
        let a = Version::new(1, 0, 0);
        let b = Version::new(1, 0, 1);
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_ine_cur_newer() {
        let a = Ok(Version::new(2, 0, 0));
        let b = Version::new(1, 0, 0);
        let ine = is_new_enough(a, &b);
        assert!(ine.unwrap());
    }

    #[test]
    fn test_ine_cur_older() {
        let a = Ok(Version::new(1, 0, 0));
        let b = Version::new(2, 0, 0);
        let ine = is_new_enough(a, &b);
        assert!(!ine.unwrap());
    }

    #[test]
    fn test_ine_cur_equal() {
        let a = Version::new(1, 0, 0);
        let ine = is_new_enough(Ok(a.clone()), &a);
        assert!(ine.unwrap());
    }

    #[test]
    fn test_ine_error() {
        let a: Result<Version, String> = Err("foobarbaz".to_string());
        let ine = is_new_enough(a, &Version::new(1, 0, 0));
        assert_eq!(ine, Err("foobarbaz".to_string()));
    }

    macro_rules! test_direnv_env_var {
        ($direnv_env_var_value:expr; fn $test_name:ident() $body:block) => {
            #[test]
            fn $test_name() {
                // Eagerly convert everything to function pointers so that all
                // tests use the same instantiation of `fork`.
                fn body_fn() $body
                let body: fn () = body_fn;

                rusty_fork::fork(
                    rusty_fork::rusty_fork_test_name!($test_name),
                    rusty_fork::rusty_fork_id!(),
                    |child| {
                        child.env("direnv", $direnv_env_var_value);
                    },
                    |child, _| {
                        rusty_fork::fork_test::supervise_child(child, 0)
                    },
                    body
                ).expect("forking test failed");
            }
        };
    }

    test_direnv_env_var!(
        "direnv";
        fn test_in_direnv_true() {
            assert!(in_direnv());
        }
    );

    test_direnv_env_var!(
        "";
        fn test_in_direnv_false() {
            assert!(!in_direnv());
        }
    );

    #[test]
    fn test_extract_version_number_success() {
        let mut spit_version_sh = tempfile::NamedTempFile::new().unwrap();
        let spit_version_sh_file = spit_version_sh.as_file_mut();
        let mut perms = spit_version_sh_file.metadata().unwrap().permissions();
        perms.set_mode(0o777);
        spit_version_sh_file.set_permissions(perms).unwrap();
        writeln!(spit_version_sh_file, r#"echo "1.1.1";"#).unwrap();
        let result = extract_version_number(spit_version_sh.path().to_string_lossy());
        assert_eq!(result, Ok(Version::new(1, 1, 1)));
    }

    #[test]
    fn test_extract_version_number_no_version() {
        let mut spit_gibberish_sh = tempfile::NamedTempFile::new().unwrap();
        let spit_gibberish_sh_file = spit_gibberish_sh.as_file_mut();
        let mut perms = spit_gibberish_sh_file.metadata().unwrap().permissions();
        perms.set_mode(0o777);
        spit_gibberish_sh_file.set_permissions(perms).unwrap();
        writeln!(
            spit_gibberish_sh.as_file_mut(),
            r#"echo "sdlfkjdsfweiojlsjslfj.dofiwoksdj/sfowiefjw0";"#
        )
        .unwrap();
        let spit_gibberish_sh_path = spit_gibberish_sh.path().to_string_lossy();
        let result = extract_version_number(&spit_gibberish_sh_path);
        assert_eq!(
            result,
            Err(format!(
                "Stdout did not contain a version number for `{} --version`",
                spit_gibberish_sh_path
            ))
        );
    }

    #[test]
    fn test_extract_version_number_nonexistent() {
        let result = extract_version_number("nonexistent.sh");
        assert_eq!(result, Err("Failed executing 'nonexistent.sh'".to_string()));
    }
}
