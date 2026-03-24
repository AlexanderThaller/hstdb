//! Core library for recording and querying zsh command history with `hstdb`.
//!
//! The crate exposes the same building blocks used by the `hstdb` binary:
//! client-side message sending, server-side recording, import helpers, and
//! CSV-backed history storage.

/// Client for sending control and history messages to a running `hstdb`
/// server.
pub mod client;
/// Configuration loading and defaults for `hstdb`.
pub mod config;
/// Normalized history entry type written to persistent storage.
pub mod entry;
/// Message types exchanged between the shell hooks, client, and server.
pub mod message;
/// Command-line option types for the `hstdb` binary.
pub mod opt;
/// High-level runtime entry points that implement CLI subcommands.
pub mod run;
/// Server-side socket handling and transient command state management.
pub mod server;
/// Persistent history storage and filtering utilities.
pub mod store;
