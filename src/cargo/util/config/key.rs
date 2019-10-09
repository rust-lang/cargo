use std::fmt;

/// Key for a configuration variable.
///
/// This type represents a configuration variable that we're looking up in
/// Cargo's configuration. This structure simultaneously keeps track of a
/// corresponding environment variable name as well as a TOML config name. The
/// intention here is that this is built up and torn down over time efficiently,
/// avoiding clones and such as possible.
#[derive(Debug, Clone)]
pub struct ConfigKey {
    // The current environment variable this configuration key maps to. This is
    // updated with `push` methods and looks like `CARGO_FOO_BAR` for pushing
    // `foo` and then `bar`.
    env: String,
    // The current toml key this configuration key maps to. This is
    // updated with `push` methods and looks like `foo.bar` for pushing
    // `foo` and then `bar`.
    config: String,
    // This is used to keep track of how many sub-keys have been pushed on this
    // `ConfigKey`. Each element of this vector is a new sub-key pushed onto
    // this `ConfigKey`. Each element is a pair of `usize` where the first item
    // is an index into `env` and the second item is an index into `config`.
    // These indices are used on `pop` to truncate `env` and `config` to rewind
    // back to the previous `ConfigKey` state before a `push`.
    parts: Vec<(usize, usize)>,
}

impl ConfigKey {
    /// Creates a new blank configuration key which is ready to get built up by
    /// using `push` and `push_sensitive`.
    pub fn new() -> ConfigKey {
        ConfigKey {
            env: "CARGO".to_string(),
            config: String::new(),
            parts: Vec::new(),
        }
    }

    /// Creates a `ConfigKey` from the `key` specified.
    ///
    /// The `key` specified is expected to be a period-separated toml
    /// configuration key.
    pub fn from_str(key: &str) -> ConfigKey {
        let mut cfg = ConfigKey::new();
        for part in key.split('.') {
            cfg.push(part);
        }
        return cfg;
    }

    /// Pushes a new sub-key on this `ConfigKey`. This sub-key should be
    /// equivalent to accessing a sub-table in TOML.
    ///
    /// Note that this considers `name` to be case-insensitive, meaning that the
    /// corrseponding toml key is appended with this `name` as-is and the
    /// corresponding env key is appended with `name` after transforming it to
    /// uppercase characters.
    pub fn push(&mut self, name: &str) {
        let env = name.replace("-", "_").to_uppercase();
        self._push(&env, name);
    }

    /// Performs the same function as `push` except that the corresponding
    /// environment variable does not get the uppercase letters of `name` but
    /// instead `name` is pushed raw onto the corresponding environment
    /// variable.
    pub fn push_sensitive(&mut self, name: &str) {
        self._push(name, name);
    }

    fn _push(&mut self, env: &str, config: &str) {
        self.parts.push((self.env.len(), self.config.len()));

        self.env.push_str("_");
        self.env.push_str(env);

        if !self.config.is_empty() {
            self.config.push_str(".");
        }
        self.config.push_str(config);
    }

    /// Rewinds this `ConfigKey` back to the state it was at before the last
    /// `push` method being called.
    pub fn pop(&mut self) {
        let (env, config) = self.parts.pop().unwrap();
        self.env.truncate(env);
        self.config.truncate(config);
    }

    /// Returns the corresponding environment variable key for this
    /// configuration value.
    pub fn as_env_key(&self) -> &str {
        &self.env
    }

    /// Returns the corresponding TOML (period-separated) key for this
    /// configuration value.
    pub fn as_config_key(&self) -> &str {
        &self.config
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_config_key().fmt(f)
    }
}
