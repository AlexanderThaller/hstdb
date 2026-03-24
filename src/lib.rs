//! Core library for recording and querying zsh command history with `hstdb`.
//!
//! The crate exposes the same building blocks used by the `hstdb` binary:
//! client-side message sending, server-side recording, import helpers, and
//! CSV-backed history storage.

/// Client for sending control and history messages to a running `hstdb`
/// server.
pub(crate) mod client;
/// Configuration loading and defaults for `hstdb`.
pub(crate) mod config;
/// Normalized history entry type written to persistent storage.
pub(crate) mod entry;
/// Message types exchanged between the shell hooks, client, and server.
pub(crate) mod message;
/// Command-line option types for the `hstdb` binary.
pub(crate) mod opt;
/// High-level runtime entry points that implement CLI subcommands.
pub(crate) mod run;
/// Server-side socket handling and transient command state management.
pub(crate) mod server;
/// Persistent history storage and filtering utilities.
pub(crate) mod store;
