pub mod shells;
pub mod state;

use std::{
    collections::{BTreeMap, HashSet},
    env, fs, num,
    path::PathBuf,
    process::ExitStatus,
};

use bstr::{B, BString, ByteSlice};
use duct::cmd;
use indexmap::IndexSet;
use once_cell::sync::Lazy;
use shell_quote::Bash;

pub type EnvVars = BTreeMap<String, String>;
pub type EnvVarsState = BTreeMap<String, Option<String>>;

pub fn get_old_env_vars_to_be_updated(old_env_vars: EnvVars, new_env_vars: &EnvVars) -> EnvVars {
    old_env_vars
        .into_iter()
        .fold(BTreeMap::new(), |mut acc, (key, value)| {
            if new_env_vars.contains_key(&key) && new_env_vars.get(&key) != Some(&value) {
                acc.insert(key, value);
            }
            acc
        })
}

// TODO: Create new type patern for `EnvVars` and `EnvVarsState`?
// impl From<EnvVars> for EnvVarsState {
//   fn from(value: EnvVars) -> Self {
//     value
//       .into_iter()
//       .map(|(key, value)| (key, Some(value)))
//       .collect()
//   }
// }
pub fn env_vars_state_from_env_vars(env_vars: EnvVars) -> EnvVarsState {
    env_vars
        .into_iter()
        .map(|(key, value)| (key, Some(value)))
        .collect()
}

pub fn get_env_vars_reset(
    mut old_env_vars_that_were_updated: EnvVars,
    new_env_vars: HashSet<String>,
    env_state_var_key: String,
) -> EnvVarsState {
    let mut env_vars_state = new_env_vars
        .into_iter()
        .fold(EnvVarsState::new(), |mut acc, key| {
            let value = old_env_vars_that_were_updated.remove(&key);
            acc.insert(key, value);
            acc
        });
    env_vars_state.insert(env_state_var_key, None);
    env_vars_state
}

pub fn get_env_vars_from_current_process() -> EnvVars {
    env::vars().collect::<EnvVars>()
}

pub enum BashSource {
    File(PathBuf),
    Script(BString),
}

impl AsRef<BashSource> for BashSource {
    fn as_ref(&self) -> &BashSource {
        self
    }
}

impl BashSource {
    fn to_command_string(&self) -> BString {
        match &self {
            Self::File(path) => bstr::join(" ", [B("source"), &Bash::quote_vec(path)]).into(),
            Self::Script(script) => bstr::join(" ", [B("eval"), &Bash::quote_vec(script)]).into(),
        }
    }
}

pub(crate) trait SimplifiedExitOk {
    fn simplified_exit_ok(&self) -> anyhow::Result<()>;
}

impl SimplifiedExitOk for ExitStatus {
    /// Simplified implementation of <https://github.com/rust-lang/rust/issues/84908>
    // TODO: Remove this and use `exit_ok` when it's stabilized.
    fn simplified_exit_ok(&self) -> anyhow::Result<()> {
        match num::NonZero::try_from(self.code().unwrap_or(-1)) {
            Ok(_) => Err(anyhow::format_err!(
                "process exited unsuccessfully: {}",
                &self
            )),
            Err(_) => Ok(()),
        }
    }
}

pub fn get_env_vars_from_bash(
    source: impl AsRef<BashSource>,
    env_vars: Option<EnvVars>,
) -> anyhow::Result<EnvVars> {
    let bash_env_vars_file = tempfile::NamedTempFile::new()?;

    let command_string = bstr::join(
        " ",
        [
            &source.as_ref().to_command_string(),
            B("&& env -0 >"),
            &Bash::quote_vec(bash_env_vars_file.path()),
        ],
    );
    let handle = cmd!("bash", "-c", command_string.to_os_str()?)
        .full_env(env_vars.unwrap_or_default())
        .stdout_to_stderr()
        .start()?;
    let output = handle.wait()?;
    output
        .status
        .simplified_exit_ok()
        .map_err(|e| anyhow::format_err!("Bash command to retrieve env vars failed:\n{e}"))?;

    let bash_env_vars_string = fs::read_to_string(bash_env_vars_file.path())?;

    let bash_env_vars = bash_env_vars_string
        .split('\0')
        .filter_map(|env_var| env_var.split_once('='))
        .map(|(key, value)| (String::from(key), String::from(value)))
        .collect::<EnvVars>();

    Ok(bash_env_vars)
}

pub fn merge_delimited_env_var(
    env_var: &str,
    split_delimiter: char,
    join_delimiter: char,
    old_env_vars: &BTreeMap<String, String>,
    new_env_vars: &mut BTreeMap<String, String>,
) {
    if let (Some(old_value), Some(new_value)) =
        (old_env_vars.get(env_var), new_env_vars.get_mut(env_var))
    {
        *new_value = merge_delimited_values(split_delimiter, join_delimiter, old_value, new_value);
    }
}

pub fn merge_delimited_values(
    split_delimiter: char,
    join_delimiter: char,
    old_value: &str,
    new_value: &str,
) -> String {
    new_value
        .split(split_delimiter)
        .chain(old_value.split(split_delimiter))
        .collect::<IndexSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(&join_delimiter.to_string())
}

const IGNORED_ENV_VAR_PREFIXES: &[&str] = &["__fish", "BASH_FUNC_"];

static IGNORED_ENV_VAR_KEYS: Lazy<HashSet<&str>> = Lazy::new(|| {
    HashSet::from([
        // direnv env config
        "DIRENV_CONFIG",
        "DIRENV_BASH",
        // should only be available inside of the .envrc or .env
        "DIRENV_IN_ENVRC",
        "COMP_WORDBREAKS", // Avoids segfaults in bash
        "PS1",             // PS1 should not be exported, fixes problem in bash
        // variables that should change freely
        "OLDPWD",
        "PWD",
        "SHELL",
        "SHELLOPTS",
        "SHLVL",
        "_",
    ])
});

pub fn ignored_env_var_key(env_var_key: &str) -> bool {
    for ignored_env_var_prefix in IGNORED_ENV_VAR_PREFIXES {
        if env_var_key.starts_with(ignored_env_var_prefix) {
            return true;
        }
    }
    IGNORED_ENV_VAR_KEYS.contains(env_var_key)
}

pub fn remove_ignored_env_vars(env_vars: &mut EnvVars) {
    let env_var_keys = env_vars.keys().cloned().collect::<Vec<_>>();
    env_var_keys.into_iter().for_each(|env_var_key| {
        if ignored_env_var_key(&env_var_key) {
            env_vars.remove(&env_var_key);
        }
    });
}
