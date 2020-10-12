# histdb-rs

![Build Status](https://github.com/AlexanderThaller/histdb-rs/workflows/Rust/badge.svg?branch=main)
[![crates.io](https://img.shields.io/crates/v/histdb-rs.svg)](https://crates.io/crates/histdb-rs)

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

Currently you need nightly to build histdb-rs. We are using the strip
functionality to decrease the binary size automatically.

```
cargo +nightly install --path .
```

Or install it from crates.io
```
cargo +nightly install histdb-rs
```

After that you need to start the server. This might change in the future.

```
histdb-rs server
```

To stop the server you have to run

```
histdb-rs stop
```

In the future `CTRL+C` should also work.

You can also use the systemd unit file in
[`histdb-rs.service`](histdb-rs.service) which you can copy to
`"$HOME/.config/systemd` and enable/start with the following:

```
systemctl --user daemon-reload
systemctl --user enable histdb-rs.service
systemctl --user start histdb-rs.service
```

After that you can add the following to your `.zshrc` to enable histdb-rs for
you shell.

```
eval "$(histdb-rs init)"
```

You can run that in your current shell to enable histdb-rs or restart your
shell.

## Usage

Help output of default command:

```
» histdb -h
histdb-rs 0.1.0

USAGE:
    histdb-rs [FLAGS] [OPTIONS] [SUBCOMMAND]

FLAGS:
        --all-hosts
            Print all hosts

        --disable-formatting
            Disable fancy formatting

    -h, --help
            Prints help information

        --hide-header
            Disable printing of header

    -i, --in
            Only print entries that have been executed in the current directory

        --no-subdirs
            Exclude subdirectories when filtering by folder

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

    -V, --version
            Prints version information


OPTIONS:
    -c, --command <command>
            Only print entries beginning with the given command

    -t, --text <command-text>
            Only print entries containing the given regex

    -d, --data-dir <data-dir>
            Path to folder in which to store the history files [default: $HOME/.local/share/histdb-rs]

    -e, --entries-count <entries-count>
            How many entries to print [default: 25]

    -f, --folder <folder>
            Only print entries that have been executed in the given directory

        --hostname <hostname>
            Filter by given hostname


SUBCOMMANDS:
    disable
            Disable history recording for current session

    enable
            Enable history recording for current session

    help
            Prints this message or the help of the given subcommand(s)

    import
            Import entries from existing histdb sqlite file

    init
            Print out shell functions needed by histdb and set current session id

    precmd
            Finish command for current session

    running
            Tell server to print currently running command

    server
            Start the server

    session_id
            Get new session id

    stop
            Stop the server

    zshaddhistory
            Add new command for current session
```

The most basic command ist just running `histdb-rs` without any arguments:

```
» histdb-rs
 tmn    cmd
 14:28  cargo +nightly install --path .
```

That will print the history for the current machine. By default only the last
25 entries will be printed.

## Git

Histdb-rs was written to easily sync the history between multiple machines. For
that histdb-rs will write separate history files for each machine.

If you want to sync between machines go to the datadir (default is
`$HOME/.local/share/histdb-rs`) and run the following commands:

```
git init
git add :/
git commit -m "Initial commit"
```

After that you can configure origins and start syncing the files between
machines. There is no autocommit/autosync implemented as we don't want to have
commits for each command run. This could be changed in the future.

## Import

### zsh-histdb

```
» histdb import histdb -h
histdb-rs-import-histdb 0.1.0
Import entries from existing histdb sqlite file

USAGE:
    histdb-rs import histdb [OPTIONS]

FLAGS:
    -h, --help
            Prints help information


OPTIONS:
    -d, --data-dir <data-dir>
            Path to folder in which to store the history files [default: $HOME/.local/share/histdb-rs]

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
histdb-rs-import-histfile 0.1.0
Import entries from existing zsh histfile

USAGE:
    histdb-rs import histfile [OPTIONS]

FLAGS:
    -h, --help
            Prints help information


OPTIONS:
    -d, --data-dir <data-dir>
            Path to folder in which to store the history files [default: $HOME/.local/share/histdb-rs]

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
* `command` will be parse from the histfile
* `time_start` will be copied over from `time_finished`
* `hostname` will use the current machines hostname
* `pwd` will be set to the current users home directory
* `session_id` will be generated and used for all commands imported from the
histfile
* `user` will use the current user thats running the import
