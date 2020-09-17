use crate::entry::Entry;
use std::{
    fs,
    path::PathBuf,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not get hostname: {0}")]
    GetHostname(std::io::Error),

    #[error("can not create log folder: {0}")]
    CreateLogFolder(PathBuf, std::io::Error),

    #[error("can not open log file: {0}")]
    OpenLogFile(PathBuf, std::io::Error),

    #[error("can not serialize entry: {0}")]
    SerializeEntry(csv::Error),
}

pub struct Store {}

pub fn new() -> Store {
    Store {}
}

impl Store {
    pub fn add(&self, entry: Entry) -> Result<(), Error> {
        let xdg_dirs = xdg::BaseDirectories::with_prefix("histdb-rs").unwrap();
        let datadir_path = xdg_dirs.get_data_home();

        let hostname = hostname::get().map_err(Error::GetHostname)?;

        let uuid = entry.session_id.to_string();
        let mut uuid = uuid.chars();

        let mut uuid_first_part = String::new();
        uuid_first_part.push(uuid.next().unwrap());
        uuid_first_part.push(uuid.next().unwrap());

        let mut uuid_second_part = String::new();
        uuid_second_part.push(uuid.next().unwrap());
        uuid_second_part.push(uuid.next().unwrap());

        let folder_path = datadir_path
            .as_path()
            .join(hostname)
            .join(&uuid_first_part)
            .join(&uuid_second_part);

        let file_path = folder_path
            .join(entry.session_id.to_string())
            .with_extension("csv");

        dbg!(&folder_path);
        dbg!(&file_path);

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
}
