use crate::entry::Entry;
use regex::Regex;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    #[error("can not get current directory: {0}")]
    GetCurrentDir(std::io::Error),
}

#[derive(Debug, Default)]
pub struct Filter {
    pub hostname: Option<String>,
    pub directory: Option<PathBuf>,
    pub command: Option<String>,
    pub no_subdirs: bool,
    pub command_text: Option<Regex>,
    pub command_text_excluded: Option<Regex>,
    pub count: usize,
    pub session: Option<Regex>,
    pub failed: bool,
    pub find_status: Option<u16>,
}

impl Filter {
    pub const fn get_hostname(&self) -> Option<&String> {
        self.hostname.as_ref()
    }

    pub fn hostname(self, hostname: Option<String>, all_hosts: bool) -> Result<Self, Error> {
        let current_hostname = hostname::get()
            .map_err(Error::GetHostname)?
            .to_string_lossy()
            .to_string();

        let hostname = if all_hosts {
            None
        } else {
            Some(hostname.unwrap_or(current_hostname))
        };

        Ok(Self { hostname, ..self })
    }

    pub fn directory(
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

    pub fn count(self, count: usize) -> Self {
        Self { count, ..self }
    }

    pub fn command(
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

    pub fn filter_entries(&self, entries: Vec<Entry>) -> Vec<Entry> {
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

    pub fn session(self, session: Option<Regex>) -> Self {
        Self { session, ..self }
    }

    pub fn filter_failed(self, filter_failed: bool) -> Self {
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

    pub fn find_status(self, find_status: Option<u16>) -> Self {
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
