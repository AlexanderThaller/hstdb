# hstdb

[![Build Status](https://github.com/AlexanderThaller/hstdb/workflows/Rust/badge.svg?branch=main)](https://github.com/AlexanderThaller/hstdb/actions?query=workflow%3ARusteain)
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
`"$XDG_CONFIG_HOME/systemd` (usually `$HOME/.config/systemd`) and
enable/start with the following:

```
systemctl --user daemon-reload
systemctl --user enable hstdb.service
systemctl --user start hstdb.service
```

After that you can add the following to your `.zshrc` to enable hstdb for
you shell.

```
eval "$(hstdb init)"
```

You can run that in your current shell to enable hstdb or restart your
shell.

## Usage

Help output of default command:

```
hstdb 2.1.0
Better history management for zsh. Based on ideas from
[https://github.com/larkery/zsh-histdb](https://github.com/larkery/zsh-histdb).

USAGE:
    hstdb [OPTIONS] [SUBCOMMAND]

OPTIONS:
        --all-hosts
            Print all hosts

    -c, --command <COMMAND>
            Only print entries beginning with the given command

        --config-path <CONFIG_PATH>
            Path to the socket for communication with the server [env: HISTDBRS_CONFIG_PATH=]
            [default: $XDG_CONFIG_HOME/hstdb/config.toml]

    -d, --data-dir <DATA_DIR>
            Path to folder in which to store the history files [default:
            $XDG_DATA_HOME/hstdb]

        --disable-formatting
            Disable fancy formatting

    -e, --entries-count <ENTRIES_COUNT>
            How many entries to print [default: 25]

    -f, --folder <FOLDER>
            Only print entries that have been executed in the given directory

        --filter-failed
            Filter out failed commands (return code not 0)

        --find-status <FIND_STATUS>
            Find commands with the given return code

    -h, --help
            Print help information

        --hide-header
            Disable printing of header

        --hostname <HOSTNAME>
            Filter by given hostname

    -i, --in
            Only print entries that have been executed in the current directory

        --no-subdirs
            Exclude subdirectories when filtering by folder

        --session <SESSION>
            Filter by given session

        --show-duration
            Show how long the command ran

        --show-host
            Print host column

        --show-pwd
            Show directory in which the command was run

        --show-session
            Show session id for command

        --show-status
            Print returncode of command

    -t, --text <COMMAND_TEXT>
            Only print entries containing the given regex

    -T, --text_excluded <COMMAND_TEXT_EXCLUDED>
            Only print entries not containing the given regex

    -V, --version
            Print version information

SUBCOMMANDS:
    bench
            Run benchmark against server
    completion
            Generate autocomplete files for shells
    disable
            Disable history recording for current session
    enable
            Enable history recording for current session
    help
            Print this message or the help of the given subcommand(s)
    import
            Import entries from existing histdb sqlite or zsh histfile
    init
            Print out shell functions needed by histdb and set current session id
    precmd
            Finish command for current session
    server
            Start the server
    session_id
            Get new session id
    stop
            Stop the server
    zshaddhistory
            Add new command for current session
```

The most basic command ist just running `hstdb` without any arguments:

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

```
» histdb import histdb -h
hstdb-import-histdb 0.1.0
Import entries from existing histdb sqlite file

USAGE:
    hstdb import histdb [OPTIONS]

FLAGS:
    -h, --help
            Prints help information


OPTIONS:
    -d, --data-dir <data-dir>
            Path to folder in which to store the history files [default: $XDG_DATA_HOME/hstdb]

    -i, --import-file <import-file>
            Path to the existing histdb sqlite file [default: $HOME/.histdb/zsh-history.db]
```

If the defaults for the `data-dir` and the `import-file` are fine you can just
run the following command:

```
histdb import histdb
```

This will create CSV files for each `hostname` found in the sqlite database. It
will create a UUID for each unique session found in sqlite so command run in the
same session should still be grouped together.

### zsh histfile

```
» histdb import histfile -h
hstdb-import-histfile 0.1.0
Import entries from existing zsh histfile

USAGE:
    hstdb import histfile [OPTIONS]

FLAGS:
    -h, --help
            Prints help information


OPTIONS:
    -d, --data-dir <data-dir>
            Path to folder in which to store the history files [default: $XDG_DATA_HOME/hstdb]

    -i, --import-file <import-file>
            Path to the existing zsh histfile file [default: $HOME/.histfile]
```

If the defaults for the `data-dir` and the `import-file` are fine you can just
run the following command:

```
histdb import histfile
```

As the information stored in the histfile is pretty limited the following
information will be stored:

* `time_finished` will be parsed from the histfile
* `result` (exit code) will be parsed from the histfile
* `command` will be parsed from the histfile
* `time_start` will be copied over from `time_finished`
* `hostname` will use the current machines hostname
* `pwd` will be set to the current users home directory
* `session_id` will be generated and used for all commands imported from the
histfile
* `user` will use the current user thats running the import

## Completion
Currentyl only zsh generation is enabled as other shells don't make
sense at the moment.

Completion generation is provided through a subcommand:

```
» hstdb completion -h
hstdb-completion 2.1.0
Generate autocomplete files for shells

USAGE:
    hstdb completion <SHELL>

ARGS:
    <SHELL>
            For which shell to generate the autocomplete [default: zsh] [possible values: zsh]

OPTIONS:
    -h, --help
            Print help information

    -V, --version
            Print version information
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
