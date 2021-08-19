# Changelog

## 1.1.0 [UNRELEASED]
* Add flag `--session`. Allows to filter entries by the given
  session. The session of a history entry can be found using
  `--show-session`.
* Add flag `--filter-failed`. Enables filtering of failed commands
  when listing the history. Will filter out all commands that had a
  return code that is not 0.
* Add option `--find-status`. When specified will find all commands
  with the given return code.

## 1.0.0 [2021-06-01]
* No big changes just updated the dependencies.
* Automatic binaries created through github actions.

## 0.1.0

### Changed

* Command filter will now only match if entry command matches exactly
[[a5c3785](https://github.com/AlexanderThaller/histdb-rs/commit/b4a89c2f109b68b901e4610ebe2f39834ffe8d6f)]
