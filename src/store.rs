use crate::entry::Entry;
use regex::Regex;
use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
    process::Command,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not create log folder: {0}")]
    CreateLogFolder(PathBuf, std::io::Error),

    #[error("can not open log file: {0}")]
    OpenLogFile(PathBuf, std::io::Error),

    #[error("can not serialize entry: {0}")]
    SerializeEntry(csv::Error),

    #[error("can not git add changes: {0}")]
    GitAdd(std::io::Error),

    #[error("can not git commit changes: {0}")]
    GitCommit(std::io::Error),

    #[error("can not git init repository: {0}")]
    GitInit(std::io::Error),

    #[error("glob is not valid: {0}")]
    InvalidGlob(glob::PatternError),

    #[error("problem while iterating glob: {0}")]
    GlobIteration(glob::GlobError),

    #[error("can not read log file {0:?}: {1}")]
    ReadLogFile(PathBuf, csv::Error),
}

#[derive(Debug)]
pub struct Store {
    data_dir: PathBuf,
}

pub const fn new(data_dir: PathBuf) -> Store {
    Store { data_dir }
}

impl Store {
    pub fn add_entry(&self, entry: &Entry) -> Result<(), Error> {
        let hostname = &entry.hostname;

        let folder_path = self.data_dir.as_path();
        let file_path = folder_path.join(hostname).with_extension("csv");

        fs::create_dir_all(&folder_path)
            .map_err(|err| Error::CreateLogFolder(folder_path.to_path_buf(), err))?;

        let mut builder = csv::WriterBuilder::new();

        // We only want to write the header if the file does not exist yet so we can
        // just append new entries to the existing file without having multiple
        // headers.
        builder.has_headers(!file_path.exists());

        let index_file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&file_path)
            .map_err(|err| Error::OpenLogFile(file_path.to_path_buf(), err))?;

        let mut writer = builder.from_writer(index_file);

        writer.serialize(&entry).map_err(Error::SerializeEntry)?;

        Ok(())
    }

    pub fn add(&self, entry: &Entry) -> Result<(), Error> {
        if entry.command.is_empty() {
            return Ok(());
        }

        self.add_entry(entry)?;
        self.commit(format!("add entry from {:?}", entry.hostname))?;

        Ok(())
    }

    pub fn commit(&self, message: impl AsRef<str>) -> Result<(), Error> {
        if !&self.data_dir.join(".git").exists() {
            Command::new("git")
                .arg("init")
                .current_dir(&self.data_dir)
                .output()
                .map_err(Error::GitInit)?;
        }

        Command::new("git")
            .arg("add")
            .arg(":/")
            .current_dir(&self.data_dir)
            .output()
            .map_err(Error::GitAdd)?;

        Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg(message.as_ref())
            .current_dir(&self.data_dir)
            .output()
            .map_err(Error::GitCommit)?;

        Ok(())
    }

    pub fn get_entries(
        &self,
        hostname: Option<String>,
        count: usize,
        command_filter: &Option<String>,
        dir_filter: &Option<PathBuf>,
        no_subdirs: bool,
        command_text: &Option<Regex>,
    ) -> Result<Vec<Entry>, Error> {
        let mut entries: Vec<_> = if let Some(hostname) = hostname {
            let index_path = self.data_dir.join(format!("{}.csv", hostname));

            Self::read_log_file(index_path)?
        } else {
            let glob_string = self.data_dir.join("*.csv");

            let glob = glob::glob(&glob_string.to_string_lossy()).map_err(Error::InvalidGlob)?;

            let index_paths = glob
                .collect::<Result<Vec<PathBuf>, glob::GlobError>>()
                .map_err(Error::GlobIteration)?;

            index_paths
                .into_iter()
                .map(Self::read_log_file)
                .collect::<Result<Vec<Vec<_>>, Error>>()?
                .into_iter()
                .flatten()
                .collect()
        };

        entries.sort();

        let entries = entries
            .into_iter()
            .filter(|entry| {
                command_filter
                    .as_ref()
                    .map_or(true, |command| entry.command.starts_with(command))
            })
            .filter(|entry| {
                dir_filter.as_ref().map_or(true, |dir| {
                    if no_subdirs {
                        entry.pwd == *dir
                    } else {
                        entry.pwd.as_path().starts_with(dir)
                    }
                })
            })
            .filter(|entry| {
                command_text
                    .as_ref()
                    .map_or(true, |regex| regex.is_match(&entry.command))
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .take(count)
            .rev()
            .collect();

        Ok(entries)
    }

    fn read_log_file<P: AsRef<Path>>(file_path: P) -> Result<Vec<Entry>, Error> {
        let file = std::fs::File::open(&file_path)
            .map_err(|err| Error::OpenLogFile(file_path.as_ref().to_path_buf(), err))?;

        let reader = std::io::BufReader::new(file);

        Self::read_metadata(reader)
            .map_err(|err| Error::ReadLogFile(file_path.as_ref().to_path_buf(), err))
    }

    fn read_metadata<R: std::io::Read>(reader: R) -> Result<Vec<Entry>, csv::Error> {
        let mut csv_reader = csv::ReaderBuilder::new().from_reader(reader);

        csv_reader
            .deserialize()
            .collect::<Result<Vec<Entry>, csv::Error>>()
    }
}
