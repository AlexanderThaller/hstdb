# Changelog

## 2.0.1 [2021-09-02]
* No longer show an error when piping the output of histdb-rs and the
  pipe has been closed. [#19]

## 2.0.0 [2021-08-31]
* Add flag `--session`. Allows to filter entries by the given
  session. The session of a history entry can be found using
  `--show-session`.
* Add flag `--filter-failed`. Enables filtering of failed commands
  when listing the history. Will filter out all commands that had a
  return code that is not 0.
* Add option `--find-status`. When specified will find all commands
  with the given return code.
* Ignore commands starting with ' ' (space). This should make it
  easier to not record sensitive commands. This is configurable in a
  configuration file with the option `ignore_space`. By default this
  is enabled.
* Add configuration option `log_level` to change the default log level
  to run under.

## 1.0.0 [2021-06-01]
* No big changes just updated the dependencies.
* Automatic binaries created through github actions.

## 0.1.0

### Changed

* Command filter will now only match if entry command matches exactly
[[a5c3785](https://github.com/AlexanderThaller/histdb-rs/commit/b4a89c2f109b68b901e4610ebe2f39834ffe8d6f)]
