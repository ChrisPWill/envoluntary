use std::cell::{Cell, RefCell};
use std::env;
use std::path::PathBuf;

use assert_matches::assert_matches;
use env_hooks::state::{self, GetEnvStateVar, MatchRcs};

#[test]
fn workflow_no_rcs_found_resets_existing_state() {
    const ENV_STATE_VAR_KEY: &str = "TEST_ENV_STATE_VAR_KEY";
    const ENV_STATE_VAR_VALUE: &str = "old_environment_state";

    unsafe {
        env::set_var(ENV_STATE_VAR_KEY, ENV_STATE_VAR_VALUE);
    }

    let current_dir_state = state::ShellPromptState::get_current_dir(None).unwrap();
    let match_rcs = current_dir_state
        .match_rcs(|_| Ok::<Vec<String>, anyhow::Error>(vec![]))
        .unwrap();
    let no_rcs_state = assert_matches!(match_rcs, MatchRcs::NoRcs(no_rcs_state) => no_rcs_state);
    let ready_for_reset = no_rcs_state.get_env_state_var(ENV_STATE_VAR_KEY).unwrap();

    let reset_executed = Cell::new(false);
    let reset_value = RefCell::new(String::new());
    ready_for_reset
        .reset_env_vars(|value| {
            reset_executed.set(true);
            *reset_value.borrow_mut() = value.to_string_lossy().to_string();
            Ok(())
        })
        .unwrap();
    assert!(reset_executed.get());
    assert_eq!(reset_value.borrow().as_str(), ENV_STATE_VAR_VALUE);

    unsafe {
        std::env::remove_var(ENV_STATE_VAR_KEY);
    }
}

#[test]
fn workflow_rcs_found_new_environment_sets_initial_state() {
    let matched_rcs = vec![
        PathBuf::from("/home/user/project/.envrc"),
        PathBuf::from("/home/user/.envrc"),
        PathBuf::from("/home/.envrc"),
    ];

    let current_dir_state = state::ShellPromptState::get_current_dir(None).unwrap();
    let match_rcs = current_dir_state
        .match_rcs(|_| Ok(matched_rcs.clone()))
        .unwrap();
    let rcs_state = assert_matches!(match_rcs, MatchRcs::Rcs(rcs_state) => rcs_state);
    let get_env_state_var = rcs_state.get_env_state_var("WORKFLOW_STATE_VAR");
    let no_env_state_var_state = assert_matches!(get_env_state_var, GetEnvStateVar::NoEnvStateVar(no_env_state_var_state) => no_env_state_var_state);

    let new_state_set = Cell::new(false);
    let rcs_paths = RefCell::new(vec![]);
    no_env_state_var_state
        .set_new_env_state_var(|rcs| {
            new_state_set.set(true);
            *rcs_paths.borrow_mut() = rcs;
            Ok(())
        })
        .unwrap();

    assert!(new_state_set.get());
    assert_eq!(rcs_paths.borrow().clone(), matched_rcs);
}

#[test]
fn workflow_rcs_found_existing_environment_resets_and_updates_state() {
    const ENV_STATE_VAR_KEY: &str = "TEST_ENV_STATE_VAR_KEY";
    const ENV_STATE_VAR_VALUE: &str = "previous_environment_state";
    let matched_rcs = vec![
        PathBuf::from("/home/user/project/.envrc"),
        PathBuf::from("/home/user/.envrc"),
    ];

    unsafe {
        env::set_var(ENV_STATE_VAR_KEY, ENV_STATE_VAR_VALUE);
    }

    let current_dir_state = state::ShellPromptState::get_current_dir(None).unwrap();
    let match_rcs = current_dir_state
        .match_rcs(|_| Ok(matched_rcs.clone()))
        .unwrap();
    let rcs_state = assert_matches!(match_rcs, MatchRcs::Rcs(rcs_state) => rcs_state);
    let get_env_state_var = rcs_state.get_env_state_var(ENV_STATE_VAR_KEY);
    let env_state_var_state = assert_matches!(get_env_state_var, GetEnvStateVar::EnvStateVar(env_state_var_state) => env_state_var_state);

    let reset_phase_ran = Cell::new(false);
    let setup_phase_ran = Cell::new(false);
    let old_state = RefCell::new(String::new());
    let new_rcs_paths = RefCell::new(vec![]);
    env_state_var_state
        .reset_and_set_new_env_state_var(
            |rcs, env_state_var_value| {
                reset_phase_ran.set(true);
                Ok((rcs, env_state_var_value))
            },
            |(rcs, env_state_var_value)| {
                setup_phase_ran.set(true);
                *new_rcs_paths.borrow_mut() = rcs;
                *old_state.borrow_mut() = env_state_var_value.to_string_lossy().to_string();
                Ok(())
            },
        )
        .unwrap();

    assert!(reset_phase_ran.get());
    assert!(setup_phase_ran.get());
    assert_eq!(old_state.borrow().as_str(), ENV_STATE_VAR_VALUE);
    assert_eq!(new_rcs_paths.borrow().clone(), matched_rcs);

    unsafe {
        std::env::remove_var(ENV_STATE_VAR_KEY);
    }
}
