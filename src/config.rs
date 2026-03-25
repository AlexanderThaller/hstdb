use log::{
    LevelFilter,
    debug,
};
use regex::Regex;
use serde::Deserialize;
use std::path::Path;
use thiserror::Error;

/// Errors returned while loading a configuration file.
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// Reading the configuration file from disk failed.
    #[error("can not read config file: {0}")]
    ReadFile(std::io::Error),

    /// Parsing the TOML configuration file failed.
    #[error("can not parse config file: {0}")]
    ParseConfig(toml::de::Error),

    /// Compiling a configured blacklist regex failed.
    #[error("invalid blacklist regex {pattern:?}: {source}")]
    InvalidBlacklistRegex {
        pattern: String,
        #[source]
        source: regex::Error,
    },
}

/// User-configurable runtime settings for `hstdb`.
#[derive(Debug)]
pub(crate) struct Config {
    /// When true disables recording commands that start with a space.
    pub(crate) ignore_space: bool,

    /// The log level to run under.
    pub(crate) log_level: LevelFilter,

    /// The hostname that should be used when writing an entry. If
    /// unset will dynamically get the hostname from the system.
    pub(crate) hostname: Option<String>,

    /// Regexes for commands that should not be recorded.
    pub(crate) blacklist_regex: Vec<Regex>,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
struct RawConfig {
    ignore_space: bool,
    log_level: LevelFilter,
    hostname: Option<String>,
    blacklist_regex: Vec<String>,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            ignore_space: true,
            log_level: LevelFilter::Warn,
            hostname: None,
            blacklist_regex: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ignore_space: true,
            log_level: LevelFilter::Warn,
            hostname: None,
            blacklist_regex: Vec::new(),
        }
    }
}

impl Config {
    /// Loads a config from `path`, returning defaults when the file does not
    /// exist.
    pub(crate) fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        if !path.as_ref().is_file() {
            debug!("no config file found using default");
            return Ok(Self::default());
        }

        let config_data = std::fs::read_to_string(path).map_err(Error::ReadFile)?;
        let config: RawConfig = toml::de::from_str(&config_data).map_err(Error::ParseConfig)?;

        Self::try_from(config)
    }

    /// Returns whether `command` matches any configured blacklist regex.
    #[must_use]
    pub(crate) fn is_blacklisted(&self, command: &str) -> bool {
        self.blacklist_regex
            .iter()
            .any(|pattern| pattern.is_match(command))
    }
}

impl TryFrom<RawConfig> for Config {
    type Error = Error;

    fn try_from(value: RawConfig) -> Result<Self, Self::Error> {
        let blacklist_regex = value
            .blacklist_regex
            .into_iter()
            .map(|pattern| {
                Regex::new(&pattern)
                    .map_err(|source| Error::InvalidBlacklistRegex { pattern, source })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            ignore_space: value.ignore_space,
            log_level: value.log_level,
            hostname: value.hostname,
            blacklist_regex,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{
        Config,
        Error,
    };

    #[test]
    fn open_compiles_blacklist_regex_patterns() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let config_path = temp_dir.path().join("config.toml");
        fs::write(
            &config_path,
            r#"
ignore_space = true
blacklist_regex = ["^ls$", "^cd$"]
"#,
        )
        .expect("config file should be written");

        let config = Config::open(&config_path).expect("config should load");

        assert!(config.is_blacklisted("ls"));
        assert!(config.is_blacklisted("cd"));
        assert!(!config.is_blacklisted("pwd"));
    }

    #[test]
    fn open_rejects_invalid_blacklist_regex_patterns() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let config_path = temp_dir.path().join("config.toml");
        fs::write(&config_path, r#"blacklist_regex = ["("]"#)
            .expect("config file should be written");

        let err = Config::open(&config_path).expect_err("config should fail to load");

        assert!(matches!(err, Error::InvalidBlacklistRegex { .. }));
    }
}
