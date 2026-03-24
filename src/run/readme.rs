use std::{
    fs,
    path::PathBuf,
};

use clap::CommandFactory;
use thiserror::Error;

use crate::opt::{
    self,
    Opt,
};

const README_USAGE_START: &str = "<!-- BEGIN GENERATED SECTION: usage-help -->";
const README_USAGE_END: &str = "<!-- END GENERATED SECTION: usage-help -->";
const README_IMPORT_HISTDB_START: &str = "<!-- BEGIN GENERATED SECTION: import-histdb-help -->";
const README_IMPORT_HISTDB_END: &str = "<!-- END GENERATED SECTION: import-histdb-help -->";
const README_IMPORT_HISTFILE_START: &str = "<!-- BEGIN GENERATED SECTION: import-histfile-help -->";
const README_IMPORT_HISTFILE_END: &str = "<!-- END GENERATED SECTION: import-histfile-help -->";
const README_COMPLETION_START: &str = "<!-- BEGIN GENERATED SECTION: completion-help -->";
const README_COMPLETION_END: &str = "<!-- END GENERATED SECTION: completion-help -->";

#[derive(Debug, Clone, Copy)]
struct HelpSection {
    command_path: &'static [&'static str],
    start_marker: &'static str,
    end_marker: &'static str,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not read README file {path}: {source}")]
    ReadReadme {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("can not write README file {path}: {source}")]
    WriteReadme {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("missing README marker `{marker}`")]
    MissingMarker { marker: &'static str },

    #[error("missing clap subcommand `{name}` while rendering README help")]
    MissingSubCommand { name: &'static str },
}

pub fn generate(readme_path: PathBuf) -> Result<(), Error> {
    let readme = fs::read_to_string(&readme_path).map_err(|source| Error::ReadReadme {
        path: readme_path.clone(),
        source,
    })?;
    let rendered = replace_generated_sections(&readme)?;

    fs::write(&readme_path, rendered).map_err(|source| Error::WriteReadme {
        path: readme_path,
        source,
    })?;

    Ok(())
}

fn replace_generated_sections(readme: &str) -> Result<String, Error> {
    let mut rendered = readme.to_owned();

    for section in help_sections() {
        rendered = replace_section(
            &rendered,
            section.start_marker,
            section.end_marker,
            &render_help(section.command_path)?,
        )?;
    }

    Ok(rendered)
}

fn help_sections() -> Vec<HelpSection> {
    let mut sections = vec![HelpSection {
        command_path: &[],
        start_marker: README_USAGE_START,
        end_marker: README_USAGE_END,
    }];

    #[cfg(feature = "histdb-import")]
    sections.push(HelpSection {
        command_path: &["import", "histdb"],
        start_marker: README_IMPORT_HISTDB_START,
        end_marker: README_IMPORT_HISTDB_END,
    });

    sections.push(HelpSection {
        command_path: &["import", "histfile"],
        start_marker: README_IMPORT_HISTFILE_START,
        end_marker: README_IMPORT_HISTFILE_END,
    });
    sections.push(HelpSection {
        command_path: &["completion"],
        start_marker: README_COMPLETION_START,
        end_marker: README_COMPLETION_END,
    });

    sections
}

fn replace_section(
    readme: &str,
    start_marker: &'static str,
    end_marker: &'static str,
    help: &str,
) -> Result<String, Error> {
    let start = readme.find(start_marker).ok_or(Error::MissingMarker {
        marker: start_marker,
    })?;
    let after_start = start + start_marker.len();

    let end = readme[after_start..]
        .find(end_marker)
        .map(|index| after_start + index)
        .ok_or(Error::MissingMarker { marker: end_marker })?;

    let replacement = format!(
        "{start_marker}\n```text\n{help}\n```\n{end_marker}",
        help = help.trim_end(),
    );

    Ok(format!(
        "{}{}{}",
        &readme[..start],
        replacement,
        &readme[end + end_marker.len()..]
    ))
}

fn render_help(command_path: &'static [&'static str]) -> Result<String, Error> {
    let mut command = Opt::command();
    let mut command = find_subcommand(&mut command, command_path)?.clone();
    command = command.bin_name(help_command_path(command_path));

    let mut help = Vec::new();
    command
        .write_long_help(&mut help)
        .expect("writing help output to a Vec should never fail");

    Ok(normalize_help_for_docs(
        String::from_utf8(help).expect("clap help output should be valid UTF-8"),
    ))
}

fn help_command_path(command_path: &[&str]) -> String {
    if command_path.is_empty() {
        "hstdb".to_string()
    } else {
        format!("hstdb {}", command_path.join(" "))
    }
}

fn find_subcommand<'a>(
    command: &'a mut clap::Command,
    command_path: &'static [&'static str],
) -> Result<&'a mut clap::Command, Error> {
    match command_path.split_first() {
        Some((name, tail)) => {
            let subcommand = command
                .find_subcommand_mut(name)
                .ok_or(Error::MissingSubCommand { name })?;

            find_subcommand(subcommand, tail)
        }
        None => Ok(command),
    }
}

fn normalize_help_for_docs(help: String) -> String {
    let mut normalized = help;

    for (actual, placeholder) in opt::readme_help_path_replacements() {
        normalized = normalized.replace(&actual, placeholder);
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        README_USAGE_END,
        README_USAGE_START,
        help_command_path,
        normalize_help_for_docs,
        render_help,
        replace_section,
    };

    #[test]
    fn render_help_uses_docs_friendly_placeholders() {
        let help = render_help(&[]).expect("top-level help should render");

        assert!(help.contains("$XDG_DATA_HOME/hstdb"));
        assert!(help.contains("$XDG_CONFIG_HOME/hstdb/config.toml"));
    }

    #[test]
    fn replace_section_only_updates_marked_content() {
        let original = format!("before\n{README_USAGE_START}\nold\n{README_USAGE_END}\nafter\n");

        let replaced = replace_section(
            &original,
            README_USAGE_START,
            README_USAGE_END,
            "Usage: hstdb\n",
        )
        .expect("README markers should be replaced");

        assert!(replaced.starts_with("before\n"));
        assert!(replaced.contains("```text\nUsage: hstdb"));
        assert!(replaced.contains("\n```"));
        assert!(replaced.ends_with("after\n"));
    }

    #[test]
    fn normalize_help_keeps_help_intact_when_no_paths_match() {
        assert_eq!("help", normalize_help_for_docs("help".to_string()));
    }

    #[test]
    fn help_command_path_includes_parent_commands() {
        assert_eq!("hstdb", help_command_path(&[]));
        assert_eq!(
            "hstdb import histfile",
            help_command_path(&["import", "histfile"])
        );
    }
}
