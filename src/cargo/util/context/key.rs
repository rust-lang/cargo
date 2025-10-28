use std::borrow::Cow;
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
    // This is used to keep track of how many sub-keys have been pushed on
    // this `ConfigKey`. Each element of this vector is a new sub-key pushed
    // onto this `ConfigKey`. Each element is a pair where the first item is
    // the key part as a string, and the second item is an index into `env`.
    // The `env` index is used on `pop` to truncate `env` to rewind back to
    // the previous `ConfigKey` state before a `push`.
    parts: Vec<(String, usize)>,
}

impl ConfigKey {
    /// Creates a new blank configuration key which is ready to get built up by
    /// using `push` and `push_sensitive`.
    pub fn new() -> ConfigKey {
        ConfigKey {
            env: "CARGO".to_string(),
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
        cfg
    }

    /// Pushes a new sub-key on this `ConfigKey`. This sub-key should be
    /// equivalent to accessing a sub-table in TOML.
    ///
    /// Note that this considers `name` to be case-insensitive, meaning that the
    /// corresponding toml key is appended with this `name` as-is and the
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
        self.parts.push((config.to_string(), self.env.len()));
        self.env.push('_');
        self.env.push_str(env);
    }

    /// Rewinds this `ConfigKey` back to the state it was at before the last
    /// `push` method being called.
    pub fn pop(&mut self) {
        let (_part, env) = self.parts.pop().unwrap();
        self.env.truncate(env);
    }

    /// Returns the corresponding environment variable key for this
    /// configuration value.
    pub fn as_env_key(&self) -> &str {
        &self.env
    }

    /// Returns an iterator of the key parts as strings.
    pub(crate) fn parts(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().map(|p| p.0.as_ref())
    }

    /// Returns whether or not this is a key for the root table.
    pub fn is_root(&self) -> bool {
        self.parts.is_empty()
    }

    /// Returns whether or not the given key string matches this key.
    /// Use * to match any key part.
    pub fn matches(&self, pattern: &str) -> bool {
        let mut parts = self.parts();
        pattern
            .split('.')
            .all(|pat| parts.next() == Some(pat) || pat == "*")
            && parts.next().is_none()
    }
}

impl fmt::Display for ConfigKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<_> = self.parts().map(|part| escape_key_part(part)).collect();
        parts.join(".").fmt(f)
    }
}

#[derive(Debug, Clone)]
pub enum KeyOrIdx {
    Key(String),
    Idx(usize),
}

/// Tracks the key path to an item in an array for detailed errro context.
#[derive(Debug, Clone)]
pub struct ArrayItemKeyPath {
    base: ConfigKey,
    /// This is stored in reverse, and is pushed only when erroring out.
    /// The first element is the innermost key.
    path: Vec<KeyOrIdx>,
}

impl ArrayItemKeyPath {
    pub fn new(base: ConfigKey) -> Self {
        Self {
            base,
            path: Vec::new(),
        }
    }

    pub fn push_key(&mut self, k: String) {
        self.path.push(KeyOrIdx::Key(k))
    }

    pub fn push_index(&mut self, i: usize) {
        self.path.push(KeyOrIdx::Idx(i))
    }
}

impl fmt::Display for ArrayItemKeyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.base)?;
        for k in self.path.iter().rev() {
            match k {
                KeyOrIdx::Key(s) => {
                    f.write_str(".")?;
                    f.write_str(&escape_key_part(&s))?;
                }
                KeyOrIdx::Idx(i) => {
                    write!(f, "[{i}]")?;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn escape_key_part<'a>(part: &'a str) -> Cow<'a, str> {
    let ok = part.chars().all(|c| {
        matches!(c,
        'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_')
    });
    if ok {
        Cow::Borrowed(part)
    } else {
        // This is a bit messy, but toml doesn't expose a function to do this.
        Cow::Owned(toml::Value::from(part).to_string())
    }
}

#[cfg(test)]
mod tests {
    use snapbox::assert_data_eq;
    use snapbox::str;

    use super::*;

    #[test]
    fn array_item_key_path_display() {
        let base = ConfigKey::from_str("array");
        let mut key_path = ArrayItemKeyPath::new(base);
        // These are pushed in reverse order.
        key_path.push_index(1);
        key_path.push_key("rustflags".into());
        key_path.push_key("thumbv8m.base-none-eabi".into());
        key_path.push_index(2);
        key_path.push_index(3);

        assert_data_eq!(
            key_path.to_string(),
            str![[r#"array[3][2]."thumbv8m.base-none-eabi".rustflags[1]"#]]
        );
    }
}
