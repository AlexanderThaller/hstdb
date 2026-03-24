# hstdb

[![Rust](https://github.com/AlexanderThaller/hstdb/actions/workflows/rust.yml/badge.svg)](https://github.com/AlexanderThaller/hstdb/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/hstdb.svg)](https://crates.io/crates/hstdb)

Better history management for zsh. Based on ideas from
[https://github.com/larkery/zsh-histdb](https://github.com/larkery/zsh-histdb).

Licensed under MIT.

It was mainly written because the sqlite merging broke a few to many times for
me and using a sqlite database seemed overkill.

The tool is just writing CSV files for each host which makes syncing them via
git pretty painless.

Has pretty much the same feature set as zsh-histdb:

* Start and stop time of the command
* Working directory in which the command was run
* Hostname of the machine the command was run in
* Unique session ids based on UUIDs
* Exit status of the command
* Import from zsh histfile and zsh-histdb sqlite database

## Installation

You can either install the right binary from the releases page or run:

```
cargo install hstdb
```

## Archlinux

Install from AUR:
* https://aur.archlinux.org/packages/hstdb/
* https://aur.archlinux.org/packages/hstdb-git/

## First Start

After you installed hstdb you need to start the server:

```
hstdb server
```

By default the server will run in the foreground.

To stop the server you can run the following:

```
hstdb stop
```

Or send SIGTERM/SIGINT (Ctrl+C) to stop the server.

You can also use the systemd unit file in
[`hstdb.service`](resources/hstdb.service) which you can copy to
`$XDG_CONFIG_HOME/systemd` (usually `$HOME/.config/systemd`) and
enable/start with the following:

```
systemctl --user daemon-reload
systemctl --user enable hstdb.service
systemctl --user start hstdb.service
```

After that you can add the following to your `.zshrc` to enable hstdb for
your shell.

```
eval "$(hstdb init)"
```

You can run that in your current shell to enable hstdb or restart your
shell.

## Usage

The help output below is generated from `clap` and can be refreshed with
`cargo run --features generate-readme -- generate-readme`.

<!-- BEGIN GENERATED SECTION: usage-help -->
```text
Better history management for zsh. Based on ideas from [https://github.com/larkery/zsh-histdb](https://github.com/larkery/zsh-histdb).

Usage: hstdb [OPTIONS] [COMMAND]

Commands:
  zshaddhistory  Add new command for current session
  server         Start the server
  stop           Stop the server
  disable        Disable history recording for current session
  enable         Enable history recording for current session
  precmd         Finish command for current session
  session_id     Get new session id
  import         Import entries from existing histdb sqlite or zsh histfile
  init           Print out shell functions needed by hstdb and set current session id
  bench          Run benchmark against server
  completion     Generate autocomplete files for shells
  help           Print this message or the help of the given subcommand(s)

Options:
  -d, --data-dir <DATA_DIR>
          Path to folder in which to store the history files
          
          [env: HSTDB_DATA_DIR=]
          [default: $XDG_DATA_HOME/hstdb]

  -e, --entries-count <ENTRIES_COUNT>
          How many entries to print
          
          [default: 25]

  -c, --command <COMMAND>
          Only print entries beginning with the given command

  -t, --text <COMMAND_TEXT>
          Only print entries containing the given regex

  -T, --text-excluded <COMMAND_TEXT_EXCLUDED>
          Only print entries not containing the given regex

  -i, --in
          Only print entries that have been executed in the current directory

  -f, --folder <FOLDER>
          Only print entries that have been executed in the given directory

      --no-subdirs
          Exclude subdirectories when filtering by folder

      --hostname <HOSTNAME>
          Filter by given hostname

      --session <SESSION>
          Filter by given session

      --all-hosts
          Print all hosts

      --disable-formatting
          Disable fancy formatting

      --show-host
          Print host column

      --show-status
          Print returncode of command

      --show-duration
          Show how long the command ran

      --show-pwd
          Show directory in which the command was run

      --show-session
          Show session id for command

      --hide-header
          Disable printing of header

      --filter-failed
          Filter out failed commands (return code not 0)

      --find-status <FIND_STATUS>
          Find commands with the given return code

      --config-path <CONFIG_PATH>
          Path to the configuration file
          
          [env: HSTDB_CONFIG_PATH=]
          [default: $XDG_CONFIG_HOME/hstdb/config.toml]

  -h, --help
          Print help

  -V, --version
          Print version
```
<!-- END GENERATED SECTION: usage-help -->

The most basic command is just running `hstdb` without any arguments:

```
» hstdb
 tmn    cmd
 14:28  cargo +nightly install --path .
```

That will print the history for the current machine. By default only the last
25 entries will be printed.

## Git

hstdb was written to easily sync the history between multiple machines. For
that hstdb will write separate history files for each machine.

If you want to sync between machines go to the datadir (default is
`$XDG_DATA_HOME/hstdb`) and run the following commands:

```
git init
git add :/
git commit -m "Initial commit"
```

After that you can configure origins and start syncing the files between
machines. There is no autocommit/autosync implemented as we don't want to have
commits for each command run. This could be changed in the future.

## Configuration

There is also a way to configure `hstdb`. By default the configuration
is stored under `$XDG_CONFIG_HOME/hstdb/config.toml` (usually
`$HOME/.config/hstdb/config.toml`). A different path can be specified
using the `--config-path` option.

The default configuration looks like this:

```toml
# When true will not save commands that start with a space.
# Default: true
ignore_space = true

# The log level to run under.
# Default: Warn
log_level = "Warn"
```

An example with all configuration options can be found in
[config.toml](config.toml).

## Import

### zsh-histdb

<!-- BEGIN GENERATED SECTION: import-histdb-help -->
```text
Import entries from existing histdb sqlite file

Usage: hstdb import histdb [OPTIONS]

Options:
  -d, --data-dir <DATA_DIR>
          Path to folder in which to store the history files
          
          [env: HSTDB_DATA_DIR=]
          [default: $XDG_DATA_HOME/hstdb]

  -i, --import-file <IMPORT_FILE>
          Path to the existing histdb sqlite file
          
          [default: $HOME/.histdb/zsh-history.db]

  -h, --help
          Print help
```
<!-- END GENERATED SECTION: import-histdb-help -->

If the defaults for the `data-dir` and the `import-file` are fine you can just
run the following command:

```
hstdb import histdb
```

This will create CSV files for each `hostname` found in the sqlite database. It
will create a UUID for each unique session found in sqlite so command run in the
same session should still be grouped together.

### zsh histfile

<!-- BEGIN GENERATED SECTION: import-histfile-help -->
```text
Import entries from existing zsh histfile

Usage: hstdb import histfile [OPTIONS]

Options:
  -d, --data-dir <DATA_DIR>
          Path to folder in which to store the history files
          
          [env: HSTDB_DATA_DIR=]
          [default: $XDG_DATA_HOME/hstdb]

  -i, --import-file <IMPORT_FILE>
          Path to the existing zsh histfile file
          
          [default: $HOME/.histfile]

  -h, --help
          Print help
```
<!-- END GENERATED SECTION: import-histfile-help -->

If the defaults for the `data-dir` and the `import-file` are fine you can just
run the following command:

```
hstdb import histfile
```

As the information stored in the histfile is pretty limited the following
information will be stored:

* `time_finished` will be parsed from the histfile
* `result` (exit code) will be parsed from the histfile
* `command` will be parsed from the histfile
* `time_start` will be copied over from `time_finished`
* `hostname` will use the current machine's hostname
* `pwd` will be set to the current users home directory
* `session_id` will be generated and used for all commands imported from the
histfile
* `user` will use the current user that is running the import

## Completion
Completion generation is provided through a subcommand:

<!-- BEGIN GENERATED SECTION: completion-help -->
```text
Generate autocomplete files for shells

Usage: hstdb completion [SHELL]

Arguments:
  [SHELL]
          For which shell to generate the autocomplete
          
          [default: zsh]
          [possible values: bash, elvish, fish, powershell, zsh]

Options:
  -h, --help
          Print help
```
<!-- END GENERATED SECTION: completion-help -->

### Zsh
For zsh make sure your `$fpath` contains a folder you can write to:
```
# add .zsh_completion to load additional zsh stuff
export fpath=(~/.zsh_completion $fpath)
```

Then write the autocomplete file to that folder:
```
hstdb completion zsh > ~/.zsh_completion/_hstdb
```

After that restart your shell which should now have working
autocompletion.

## Contribution

I'm happy with how the tool works for me so I won't expand it further but
contributions for features and fixes are always welcome!

## Notes
* This tool follows the [XDG Base Directory
  Specification](https://specifications.freedesktop.org/basedir-spec/basedir-spec-latest.html)
  where possible.


## Alternatives

* https://github.com/ellie/atuin
