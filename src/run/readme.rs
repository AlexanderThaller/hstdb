use std::{
    fs,
    path::PathBuf,
};

use askama::Template;
use clap::CommandFactory;
use thiserror::Error;

use crate::opt::{
    self,
    Opt,
};

#[derive(Template)]
#[template(path = "README.md", escape = "none")]
struct ReadmeTemplate {
    usage_help: String,
    import_histdb_help: String,
    import_histfile_help: String,
    completion_help: String,
    include_histdb_import: bool,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("can not write README file {path}: {source}")]
    WriteReadme {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("missing clap subcommand `{name}` while rendering README help")]
    MissingSubCommand { name: &'static str },

    #[error("can not render README template: {0}")]
    RenderTemplate(#[from] askama::Error),
}

pub fn generate(readme_path: PathBuf) -> Result<(), Error> {
    let rendered = render_readme()?;

    fs::write(&readme_path, rendered).map_err(|source| Error::WriteReadme {
        path: readme_path,
        source,
    })?;

    Ok(())
}

fn render_readme() -> Result<String, Error> {
    #[cfg(feature = "histdb-import")]
    let import_histdb_help = render_help(&["import", "histdb"])?;

    #[cfg(not(feature = "histdb-import"))]
    let import_histdb_help = String::new();

    Ok(ReadmeTemplate {
        usage_help: render_help(&[])?,
        import_histdb_help,
        import_histfile_help: render_help(&["import", "histfile"])?,
        completion_help: render_help(&["completion"])?,
        include_histdb_import: cfg!(feature = "histdb-import"),
    }
    .render()?)
}

fn render_help(command_path: &'static [&'static str]) -> Result<String, Error> {
    let mut command = Opt::command();
    let command = find_subcommand(&mut command, command_path)?;
    *command = command.clone().bin_name(help_command_path(command_path));

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
    use std::fs;

    use super::{
        generate,
        help_command_path,
        normalize_help_for_docs,
        render_help,
        render_readme,
    };

    #[test]
    fn render_help_uses_docs_friendly_placeholders() {
        let help = render_help(&[]).expect("top-level help should render");

        assert!(help.contains("$XDG_DATA_HOME/hstdb"));
        assert!(help.contains("$XDG_CONFIG_HOME/hstdb/config.toml"));
    }

    #[test]
    fn render_readme_contains_generated_help() {
        let readme = render_readme().expect("README should render");

        assert!(readme.contains("# hstdb"));
        assert!(readme.contains("Usage: hstdb [OPTIONS] [COMMAND]"));
        assert!(readme.contains("Usage: hstdb import histfile [OPTIONS]"));
        assert!(readme.contains("Usage: hstdb completion [SHELL]"));

        if cfg!(feature = "histdb-import") {
            assert!(readme.contains("Usage: hstdb import histdb [OPTIONS]"));
        }
    }

    #[test]
    fn generate_writes_rendered_readme() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let readme_path = temp_dir.path().join("README.md");

        generate(readme_path.clone()).expect("README generation should succeed");

        let written = fs::read_to_string(readme_path).expect("generated README should be readable");
        let expected = render_readme().expect("README should render");

        assert_eq!(expected, written);
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
