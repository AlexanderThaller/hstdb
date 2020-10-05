pub mod filter;

use crate::entry::Entry;
pub use filter::Filter;
use std::{
    fs,
    path::{
        Path,
        PathBuf,
    },
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

    #[error("glob is not valid: {0}")]
    InvalidGlob(glob::PatternError),

    #[error("problem while iterating glob: {0}")]
    GlobIteration(glob::GlobError),

    #[error("can not read log file {0:?}: {1}")]
    ReadLogFile(PathBuf, csv::Error),

    #[error("{0}")]
    Filter(#[from] filter::Error),
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

        Ok(())
    }

    pub fn get_entries(&self, filter: &Filter) -> Result<Vec<Entry>, Error> {
        let mut entries: Vec<_> = if let Some(hostname) = filter.get_hostname() {
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

        let entries = filter.filter_entries(entries)?;

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
