//! Levare's updater, with no UI dependencies: reads the release's
//! `latest.json`, decides whether it is newer than the running app, and
//! downloads the archive, accepting it only when its SHA-256 and its minisign
//! signature (made by the release workflow, see
//! `.github/scripts/update-manifest.sh`) both match, and replaces the
//! installed `Levare.app` with it.
//!
//! Everything here blocks; the app calls it from a background thread.

mod client;
mod error;
mod install;
mod manifest;
mod verify;

pub use client::{Download, Updater};
pub use error::Error;
pub use install::Installation;
pub use manifest::{Asset, Manifest, Update, platform};
pub use verify::verify_file;

/// The manifest the app reads: always the newest release's.
pub const MANIFEST_URL: &str =
    "https://github.com/angel-afonso/levare/releases/latest/download/latest.json";
