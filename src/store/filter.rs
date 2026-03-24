use crate::{
    config::Config,
    entry::Entry,
};
use regex::Regex;
use std::path::PathBuf;
use thiserror::Error;

/// Errors returned while building filters from local runtime state.
#[derive(Error, Debug)]
pub(crate) enum Error {
    /// Resolving the current hostname failed.
    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    /// Resolving the current working directory failed.
    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),
}

/// Filter configuration applied when querying stored history entries.
#[derive(Debug, Default)]
pub(crate) struct Filter<'a> {
    /// Optional hostname to restrict the query to.
    pub(crate) hostname: Option<String>,
    /// Optional working directory prefix to restrict the query to.
    pub(crate) directory: Option<PathBuf>,
    /// Optional command name matched against pipeline segments.
    pub(crate) command: Option<String>,
    /// Whether directory filtering should exclude subdirectories.
    pub(crate) no_subdirs: bool,
    /// Optional regex that must match the full command text.
    pub(crate) command_text: Option<Regex>,
    /// Optional regex that must not match the full command text.
    pub(crate) command_text_excluded: Option<Regex>,
    /// Maximum number of entries to return, counting from the end.
    pub(crate) count: usize,
    /// Optional regex matched against the session identifier.
    pub(crate) session: Option<Regex>,
    /// Whether failed commands should be filtered out.
    pub(crate) failed: bool,
    /// Optional exit status that entries must match.
    pub(crate) find_status: Option<u16>,

    config_hostname: Option<&'a str>,
}

impl<'a> Filter<'a> {
    #[must_use]
    /// Returns the effective hostname restriction, if any.
    pub(crate) const fn get_hostname(&self) -> Option<&String> {
        self.hostname.as_ref()
    }

    #[must_use]
    /// Creates a new filter using defaults derived from `config`.
    pub(crate) fn new(config: &'a Config) -> Self {
        Self {
            config_hostname: config.hostname.as_deref(),
            ..Default::default()
        }
    }

    /// Sets the hostname filter, optionally resolving the current hostname when
    /// `all_hosts` is false and no explicit hostname was provided.
    pub(crate) fn hostname(self, hostname: Option<String>, all_hosts: bool) -> Result<Self, Error> {
        let current_hostname = if let Some(config_hostname) = self.config_hostname {
            config_hostname.to_string()
        } else {
            hostname::get()
                .map_err(Error::GetHostname)?
                .to_string_lossy()
                .to_string()
        };

        let hostname = if all_hosts {
            None
        } else {
            Some(hostname.unwrap_or(current_hostname))
        };

        Ok(Self { hostname, ..self })
    }

    /// Sets the directory filter, optionally resolving the current directory
    /// when `in_current` is true.
    pub(crate) fn directory(
        self,
        folder: Option<PathBuf>,
        in_current: bool,
        no_subdirs: bool,
    ) -> Result<Self, Error> {
        let directory = if in_current {
            Some(std::env::current_dir().map_err(Error::GetCurrentDir)?)
        } else {
            folder
        };

        Ok(Self {
            directory,
            no_subdirs,
            ..self
        })
    }

    #[must_use]
    /// Limits the number of entries returned by the filter.
    pub(crate) fn count(self, count: usize) -> Self {
        Self { count, ..self }
    }

    #[must_use]
    /// Configures command-name and command-text filters.
    pub(crate) fn command(
        self,
        command: Option<String>,
        command_text: Option<Regex>,
        command_text_excluded: Option<Regex>,
    ) -> Self {
        Self {
            command,
            command_text,
            command_text_excluded,
            ..self
        }
    }

    #[must_use]
    /// Applies the filter to a set of entries and returns the matching subset.
    pub(crate) fn filter_entries(&self, entries: Vec<Entry>) -> Vec<Entry> {
        let filtered: Vec<Entry> = entries
            .into_iter()
            .filter(|entry| {
                self.command
                    .as_ref()
                    .is_none_or(|command| Self::filter_command(&entry.command, command))
            })
            .filter(|entry| {
                self.directory.as_ref().is_none_or(|dir| {
                    if self.no_subdirs {
                        entry.pwd == *dir
                    } else {
                        entry.pwd.as_path().starts_with(dir)
                    }
                })
            })
            .filter(|entry| {
                self.command_text
                    .as_ref()
                    .is_none_or(|regex| regex.is_match(&entry.command))
            })
            .filter(|entry| {
                self.command_text_excluded
                    .as_ref()
                    .is_none_or(|regex| !regex.is_match(&entry.command))
            })
            .filter(|entry| {
                self.session
                    .as_ref()
                    .is_none_or(|regex| regex.is_match(&entry.session_id.to_string()))
            })
            .filter(|entry| !self.failed || entry.result == 0)
            .filter(|entry| {
                self.find_status
                    .and_then(|find_status| {
                        if find_status == entry.result {
                            None
                        } else {
                            Some(())
                        }
                    })
                    .is_none()
            })
            .collect();

        if self.count > 0 {
            filtered.into_iter().rev().take(self.count).rev().collect()
        } else {
            filtered
        }
    }

    #[must_use]
    /// Restricts matches to session ids matching `session`.
    pub(crate) fn session(self, session: Option<Regex>) -> Self {
        Self { session, ..self }
    }

    #[must_use]
    /// Enables filtering out non-zero exit statuses when `filter_failed` is
    /// true.
    pub(crate) fn filter_failed(self, filter_failed: bool) -> Self {
        Self {
            failed: filter_failed,
            ..self
        }
    }

    fn filter_command(entry_command: &str, command: &str) -> bool {
        entry_command
            .split('|')
            .any(|pipe_command| pipe_command.split_whitespace().next() == Some(command))
    }

    #[must_use]
    /// Restricts matches to entries with the given exit status.
    pub(crate) fn find_status(self, find_status: Option<u16>) -> Self {
        Self {
            find_status,
            ..self
        }
    }
}

#[cfg(test)]
mod test {
    use super::Filter;

    #[test]
    fn filter_command() {
        let cases = vec![
            ("tr -d ' '", true),
            ("echo 'tr'", false),
            ("echo 'test test' | tr -d ' '", true),
            ("echo 'test test' | echo tr -d ' '", false),
            ("echo 'test test' | tr -d ' ' | tr -d 't'", true),
            ("", false),
            ("tr", true),
        ];
        let check_command = "tr";

        for (entry_command, result) in cases {
            assert_eq!(Filter::filter_command(entry_command, check_command), result);
        }
    }
}
