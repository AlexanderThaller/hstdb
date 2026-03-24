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
  init           Print out shell functions needed by histdb and set current session id
  bench          Run benchmark against server
  completion     Generate autocomplete files for shells
  help           Print this message or the help of the given subcommand(s)

Options:
  -d, --data-dir <DATA_DIR>
          Path to folder in which to store the history files [env: HSTDB_DATA_DIR=] [default: /home/athaller/.local/share/hstdb]
  -e, --entries-count <ENTRIES_COUNT>
          How many entries to print [default: 25]
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
          Path to the config file [env: HSTDB_CONFIG_PATH=] [default: $XDG_CONFIG_HOME/hstdb/config.toml]
  -h, --help
          Print help
  -V, --version
          Print version
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
