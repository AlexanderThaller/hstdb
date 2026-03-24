//! CSV-backed persistent history storage for `hstdb`.

/// Query filtering primitives used when reading history entries.
pub(crate) mod filter;

use crate::entry::Entry;
pub(crate) use filter::Filter;
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs,
    io::BufReader,
    path::{
        Path,
        PathBuf,
    },
};
use thiserror::Error;

/// Errors returned while writing or reading persistent history files.
#[derive(Error, Debug)]
pub(crate) enum Error {
    /// Creating the directory for host history files failed.
    #[error("can not create log folder: {0}: {1}")]
    CreateLogFolder(PathBuf, #[source] std::io::Error),

    /// Opening a history file for reading or appending failed.
    #[error("can not open log file: {0}: {1}")]
    OpenLogFile(PathBuf, #[source] std::io::Error),

    /// Serializing an entry as CSV failed.
    #[error("can not serialize entry: {0}")]
    SerializeEntry(csv::Error),

    /// Building the glob used to read host files failed.
    #[error("glob is not valid: {0}")]
    InvalidGlob(glob::PatternError),

    /// Iterating over the matched history files failed.
    #[error("problem while iterating glob: {0}")]
    GlobIteration(glob::GlobError),

    /// Reading or deserializing a history file failed.
    #[error("can not read log file {0:?}: {1}")]
    ReadLogFile(PathBuf, csv::Error),

    /// Applying a filter that depends on runtime state failed.
    #[error("{0}")]
    Filter(#[from] filter::Error),
}

/// CSV-backed history store organized as one file per host.
#[derive(Debug)]
pub(crate) struct Store {
    data_dir: PathBuf,
}

#[must_use]
/// Creates a store rooted at `data_dir`.
pub(crate) const fn new(data_dir: PathBuf) -> Store {
    Store { data_dir }
}

impl Store {
    /// Appends an entry to the host-specific CSV file.
    pub(crate) fn add_entry(&self, entry: &Entry) -> Result<(), Error> {
        let hostname = &entry.hostname;

        let folder_path = self.data_dir.as_path();
        // Can't use .with_extension here as it will not work properly with hostnames
        // that contain dots. See test::dot_filename_with_extension for an
        // example.
        let file_path = folder_path.join(format!("{hostname}.csv"));

        fs::create_dir_all(folder_path)
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
            .map_err(|err| Error::OpenLogFile(file_path.clone(), err))?;

        let mut writer = builder.from_writer(index_file);

        writer.serialize(entry).map_err(Error::SerializeEntry)?;

        Ok(())
    }

    /// Appends an entry when it contains a non-empty command.
    pub(crate) fn add(&self, entry: &Entry) -> Result<(), Error> {
        if entry.command.is_empty() {
            return Ok(());
        }

        self.add_entry(entry)?;

        Ok(())
    }

    /// Reads, sorts, and filters entries from the persistent store.
    pub fn get_entries(&self, filter: &Filter<'_>) -> Result<Vec<Entry>, Error> {
        let mut collector = EntryCollector::new(filter.count_limit());

        if let Some(hostname) = filter.get_hostname() {
            let index_path = self.data_dir.join(format!("{hostname}.csv"));

            Self::read_log_file(index_path, filter, &mut collector)?;
        } else {
            let glob_string = self.data_dir.join("*.csv");

            let glob = glob::glob(&glob_string.to_string_lossy()).map_err(Error::InvalidGlob)?;

            let index_paths = glob
                .collect::<Result<Vec<PathBuf>, glob::GlobError>>()
                .map_err(Error::GlobIteration)?;

            for index_path in index_paths {
                Self::read_log_file(index_path, filter, &mut collector)?;
            }
        }

        Ok(collector.finish())
    }

    fn read_log_file<P: AsRef<Path>>(
        file_path: P,
        filter: &Filter<'_>,
        collector: &mut EntryCollector,
    ) -> Result<(), Error> {
        let file = std::fs::File::open(&file_path)
            .map_err(|err| Error::OpenLogFile(file_path.as_ref().to_path_buf(), err))?;

        let reader = BufReader::with_capacity(256 * 1024, file);

        Self::read_metadata(reader, filter, collector)
            .map_err(|err| Error::ReadLogFile(file_path.as_ref().to_path_buf(), err))
    }

    fn read_metadata<R: std::io::Read>(
        reader: R,
        filter: &Filter<'_>,
        collector: &mut EntryCollector,
    ) -> Result<(), csv::Error> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .buffer_capacity(256 * 1024)
            .from_reader(reader);

        for entry in csv_reader.deserialize() {
            let entry = entry?;

            if filter.matches_entry(&entry) {
                collector.push(entry);
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
enum EntryCollector {
    All(Vec<Entry>),
    Top {
        count: usize,
        entries: BinaryHeap<Reverse<Entry>>,
    },
}

impl EntryCollector {
    fn new(count: usize) -> Self {
        if count == 0 {
            Self::All(Vec::new())
        } else {
            Self::Top {
                count,
                entries: BinaryHeap::with_capacity(count),
            }
        }
    }

    fn push(&mut self, entry: Entry) {
        match self {
            Self::All(entries) => entries.push(entry),
            Self::Top { count, entries } => {
                if entries.len() < *count {
                    entries.push(Reverse(entry));
                    return;
                }

                if entries.peek().is_some_and(|smallest| entry > smallest.0) {
                    entries.pop();
                    entries.push(Reverse(entry));
                }
            }
        }
    }

    fn finish(self) -> Vec<Entry> {
        let mut entries = match self {
            Self::All(entries) => entries,
            Self::Top { entries, .. } => entries.into_iter().map(|entry| entry.0).collect(),
        };

        entries.sort();
        entries
    }
}

#[cfg(test)]
mod test {
    use super::EntryCollector;
    use crate::{
        config::Config,
        entry::Entry,
        store::Filter,
    };
    use chrono::{
        TimeZone,
        Utc,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn entry(second: i64, command: &str) -> Entry {
        Entry {
            time_finished: Utc.timestamp_opt(second, 0).unwrap(),
            time_start: Utc.timestamp_opt(second - 1, 0).unwrap(),
            hostname: "host".to_string(),
            command: command.to_string(),
            pwd: PathBuf::from("/tmp"),
            result: 0,
            session_id: Uuid::nil(),
            user: "user".to_string(),
        }
    }

    #[test]
    fn dot_filename_with_extension() {
        let folder_path = std::path::PathBuf::from("/tmp");
        let hostname = "test.test.test";
        let expected = std::path::PathBuf::from(format!("/tmp/{hostname}.csv"));

        let bad = folder_path.join(hostname).with_extension("csv");
        let good = folder_path.join(format!("{hostname}.csv"));

        assert_ne!(bad, expected);
        assert_eq!(good, expected);
    }

    #[test]
    fn entry_collector_keeps_highest_entries() {
        let mut collector = EntryCollector::new(2);

        collector.push(entry(3, "third"));
        collector.push(entry(1, "first"));
        collector.push(entry(4, "fourth"));
        collector.push(entry(2, "second"));

        let entries = collector.finish();

        assert_eq!(entries, vec![entry(3, "third"), entry(4, "fourth")]);
    }

    #[test]
    fn get_entries_applies_filter_before_count_limit() {
        let data_dir = tempfile::tempdir().unwrap();
        let store = crate::store::new(data_dir.path().to_path_buf());

        store.add_entry(&entry(1, "keep-one")).unwrap();
        store.add_entry(&entry(2, "drop-two")).unwrap();
        store.add_entry(&entry(3, "keep-three")).unwrap();
        store.add_entry(&entry(4, "keep-four")).unwrap();

        let config = Config::default();
        let filter = Filter::new(&config)
            .command(None, Some(regex::Regex::new("^keep").unwrap()), None)
            .count(2);

        let entries = store.get_entries(&filter).unwrap();

        assert_eq!(entries, vec![entry(3, "keep-three"), entry(4, "keep-four")]);
    }
}
