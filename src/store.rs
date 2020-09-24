use crate::entry::Entry;
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

    #[error("glob is not valid: {0}")]
    InvalidGlob(glob::PatternError),

    #[error("problem while iterating glob: {0}")]
    GlobIteration(glob::GlobError),

    #[error("can not read log file {0:?}: {1}")]
    ReadLogFile(PathBuf, csv::Error),
}

pub struct Store {}

pub fn new() -> Store {
    Store {}
}

impl Store {
    pub fn add(&self, entry: Entry) -> Result<(), Error> {
        if entry.command.is_empty() {
            return Ok(());
        }

        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let datadir_path = xdg_dirs.get_data_home();

        let hostname = &entry.hostname;

        let folder_path = datadir_path.as_path();
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

        self.commit(hostname)?;

        Ok(())
    }

    fn commit(&self, hostname: &str) -> Result<(), Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let datadir_path = xdg_dirs.get_data_home();

        Command::new("git")
            .arg("add")
            .arg(":/")
            .current_dir(&datadir_path)
            .output()
            .map_err(Error::GitAdd)?;

        Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg(format!("added changes from {:?}", hostname))
            .current_dir(&datadir_path)
            .output()
            .map_err(Error::GitCommit)?;

        Ok(())
    }

    pub fn get_nth_entries(&self, hostname: &str, count: usize) -> Result<Vec<Entry>, Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let datadir_path = xdg_dirs.get_data_home();

        let glob_string = datadir_path.join("*.csv");

        let glob = glob::glob(&glob_string.to_string_lossy()).map_err(Error::InvalidGlob)?;

        let index_paths = glob
            .collect::<Result<Vec<PathBuf>, glob::GlobError>>()
            .map_err(Error::GlobIteration)?;

        let mut entries: Vec<_> = index_paths
            .into_iter()
            .map(Self::read_log_file)
            .collect::<Result<Vec<Vec<_>>, Error>>()?
            .into_iter()
            .flatten()
            .filter(|entry| entry.hostname == hostname)
            .collect();

        entries.sort();

        let entries = entries.into_iter().rev().take(count).rev().collect();

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
