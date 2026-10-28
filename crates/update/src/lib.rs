//! Levare's updater, with no UI dependencies: reads the release's
//! `latest.json`, decides whether it is newer than the running app, and
//! downloads the archive, accepting it only when its SHA-256 and its minisign
//! signature (made by the release workflow, see
//! `.github/scripts/update-manifest.sh`) both match, and replaces the
//! installation with it: `Levare.app` on macOS, the executable and the sfizz
//! library in Levare's folder on Windows and Linux.
//!
//! Everything here blocks; the app calls it from a background thread.

mod archive;
mod client;
mod error;
mod install;
mod manifest;
mod relaunch;
mod verify;

pub use client::{Download, Updater};
pub use error::Error;
pub use install::Installation;
pub use manifest::{Asset, Manifest, Update, platform};
pub use relaunch::relaunch_after_exit;
pub use verify::verify_file;

/// The manifest the app reads: always the newest release's.
pub const MANIFEST_URL: &str =
    "https://github.com/angel-afonso/levare/releases/latest/download/latest.json";
