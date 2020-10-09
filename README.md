# histdb-rs

https://github.com/larkery/zsh-histdb replacement written in rust.

## Installation

Currently you need nightly to build histdb-rs. We are using the strip
functinality to decrease the binary size automatically.

```
cargo +nightly install --path .
```

After that you need to start the server. This might change in the future.

```
histdb-rs server
```

to stop the server you have to run

```
histdb-rs stop
```

In the future `CTRL+C` should also work.

You can also use the systemd unit file in `histdb-rs.service` which you can
copy to `"$HOME/.config/systemd` and enable/start with the following:

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
that histdb-rs will write sepperate history files for each machine.
