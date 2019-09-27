use std::fmt;

/// Key for a configuration variable.
#[derive(Debug, Clone)]
pub struct ConfigKey {
    env: String,
    config: String,
    parts: Vec<(usize, usize)>,
}

impl ConfigKey {
    pub fn new() -> ConfigKey {
        ConfigKey {
            env: "CARGO".to_string(),
            config: String::new(),
            parts: Vec::new(),
        }
    }

    pub fn from_str(key: &str) -> ConfigKey {
        let mut cfg = ConfigKey::new();
        for part in key.split('.') {
            cfg.push(part);
        }
        return cfg;
    }

    pub fn push(&mut self, name: &str) {
        let env = name.replace("-", "_").to_uppercase();
        self._push(&env, name);
    }

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

    pub fn pop(&mut self) {
        let (env, config) = self.parts.pop().unwrap();
        self.env.truncate(env);
        self.config.truncate(config);
    }

    pub fn as_env_key(&self) -> &str {
        &self.env
    }

    pub fn as_config_key(&self) -> &str {
        &self.config
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_config_key().fmt(f)
    }
}
