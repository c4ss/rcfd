//! The run service's budgets (ADR 0733 P3, F-837): `runner_config` reads `.rcfd/runner-config.json` with the defaults and
//! what the runner running now started with; `set_runner_config` changes keys of it through the runner's own
//! `rcfd_runner::runner::Config::set` (the runner's checks, `replace_if_hash` under the edit lock, one journal line with the
//! actor, the reason and each changed key's old and new value). A change applies at the next runner start.

use crate::journal::reason_arg;
use crate::{err, RpcError, Workspace, CONFLICT, INTERNAL, LOCK_BUSY, PARAMS, VALIDATION};
use rcfd_runner::config::{Base, ConfigError, SetOptions};
use rcfd_runner::runner::Config;
use serde_json::{json, Value};

/// The command name of the write, in its journal line.
pub const SET_RUNNER_CONFIG: &str = "set_runner_config";

/// A budgets error as a command error: a refusal 1003 located at its key, a conflict 1200 with `data.current_hash`, a
/// busy edit lock 1201, anything else -32000.
pub fn config_error(e: ConfigError) -> RpcError {
    let message = e.to_string();
    match e {
        ConfigError::Refused { key, .. } => RpcError {
            code: VALIDATION,
            message,
            data: Some(json!({"key": key})),
        },
        ConfigError::Conflict { current_hash } => RpcError {
            code: CONFLICT,
            message,
            data: Some(json!({"current_hash": current_hash})),
        },
        ConfigError::LockBusy(_) => err(LOCK_BUSY, message),
        ConfigError::Io(_) => err(INTERNAL, message),
    }
}

impl Workspace {
    pub(crate) fn runner_config(&self) -> Result<Value, RpcError> {
        Config::state(self.project.root()).map_err(config_error)
    }

    pub(crate) fn set_runner_config(&self, args: &Value) -> Result<Value, RpcError> {
        let reason = reason_arg(args)?;
        let Some(set) = args.get("set").and_then(Value::as_object) else {
            return Err(err(PARAMS, "set: give {key: number or null}"));
        };
        if set.is_empty() {
            return Err(err(PARAMS, "set: give at least one {key: number or null}"));
        }
        if let Some((k, _)) = set.iter().find(|(_, v)| !(v.is_number() || v.is_null())) {
            return Err(err(
                PARAMS,
                format!("set.{k}: a number, or null for the runner's default"),
            ));
        }
        let base = match args.get("base_hash") {
            Some(Value::Null) => Base::Absent,
            Some(Value::String(h)) => Base::Hash(h),
            Some(_) => {
                return Err(err(
                    PARAMS,
                    "base_hash: the hash from runner_config, or null when there is no file",
                ))
            }
            None => return Err(err(PARAMS, "missing required argument 'base_hash'")),
        };
        let replace = match args.get("replace") {
            None | Some(Value::Null) => false,
            Some(Value::Bool(b)) => *b,
            Some(_) => return Err(err(PARAMS, "replace: true or false")),
        };
        let opts = SetOptions {
            replace,
            who: SET_RUNNER_CONFIG.into(),
            extra: self.origin(SET_RUNNER_CONFIG, reason).fields(),
        };
        Config::set(self.project.root(), set, base, opts).map_err(config_error)
    }
}
