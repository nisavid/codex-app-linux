//! Shared test helpers.
//!
//! Several tests across modules mutate process-wide env vars
//! (`HOME`, `PATH`, `NVM_DIR`, `CODEX_CLI_PATH`, display sockets, ...) so
//! they can drive `command_path_env`, `npm_program`, and
//! `hydrate_session_bus_env` deterministically. Cargo runs unit tests in
//! parallel; without serialisation those mutations race across threads
//! — on a developer machine with `nvm` installed the tests would otherwise
//! pick up the real `~/.nvm/.../bin/npm` instead of the temp-dir fake. Each
//! test that touches env vars must hold this lock for its entire body.

use std::sync::MutexGuard;

pub(crate) fn env_lock() -> MutexGuard<'static, ()> {
    crate::TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

pub(crate) struct EnvVarGuard {
    key: &'static str,
    original: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set<K: Into<std::ffi::OsString>>(key: &'static str, value: K) -> Self {
        let original = std::env::var_os(key);
        std::env::set_var(key, value.into());
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(value) = &self.original {
            std::env::set_var(self.key, value);
        } else {
            std::env::remove_var(self.key);
        }
    }
}
