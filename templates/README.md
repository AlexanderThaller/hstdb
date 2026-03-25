# hstdb

[![Rust](https://github.com/AlexanderThaller/hstdb/actions/workflows/rust.yml/badge.svg)](https://github.com/AlexanderThaller/hstdb/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/hstdb.svg)](https://crates.io/crates/hstdb)

Better history management for zsh. Based on ideas from
[https://github.com/larkery/zsh-histdb](https://github.com/larkery/zsh-histdb).

Licensed under MIT.

It was mainly written because the sqlite merging broke a few too many
times for me and using an sqlite database seemed overkill.

The tool is just writing CSV files for each host which makes syncing
them via git pretty painless.

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

If [`skim`](https://github.com/lotabout/skim) is installed and `sk` is on
your `PATH`, the init script also binds `Ctrl-R` in `zsh` to an interactive
history picker backed by `hstdb`.

The picker queries the newest 10,000 history entries by default, shows the
latest matches first, and inserts the selected command into the current
command line.

You can customize the history size, the `hstdb` query filters, or the `sk`
UI with:

```zsh
export HSTDB_SKIM_HISTORY_ENTRIES_COUNT=10000
export HSTDB_SKIM_HISTORY_ARGS='--all-hosts --filter-failed'
export HSTDB_SKIM_CTRL_R_OPTS='--preview "echo {}"'
```

Set `HSTDB_SKIM_HISTORY_ENTRIES_COUNT=0` if you want the picker to query the
full history.

## Usage

```text
{{ usage_help }}
```

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

# The hostname that should be used when writing an entry. If unset
# will dynamically get the hostname from the system.
# Default: None
hostname = "thaller-desktop-linux"

# A list of regexes that will be used to filter out commands. If a
# command matches any of the regexes in this list, it will not be saved.
# Default: []
blacklist_regex = ["^ls$", "^cd$"]
```

An example with all configuration options can be found in
[config.toml](config.toml).

## Import

{% if include_histdb_import %}
### zsh-histdb

```text
{{ import_histdb_help }}
```

If the defaults for the `data-dir` and the `import-file` are fine you can just
run the following command:

```
hstdb import histdb
```

This will create CSV files for each `hostname` found in the sqlite database. It
will create a UUID for each unique session found in sqlite so commands run in the
same session are still grouped together.

{% endif %}
### zsh histfile

```text
{{ import_histfile_help }}
```

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
* `pwd` will be set to the current user's home directory
* `session_id` will be generated and used for all commands imported from the
histfile
* `user` will use the current user that is running the import

## Completion
Completion generation is provided through a subcommand:

```text
{{ completion_help }}
```

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
