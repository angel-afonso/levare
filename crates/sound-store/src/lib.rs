//! Extra sound packs installed on demand. The catalog ships inside the app;
//! each pack is downloaded, checked against its published SHA-256, unpacked
//! into a staging folder and moved into place only once it validates.
//! Packs carry samples and SFZ text only: the sfizz engine ships with Levare.
mod catalog;
mod install;
#[cfg(test)]
mod tests;

pub use catalog::{Archive, Catalog, Entry, License};
pub use install::{
    Installed, Prepared, Progress, Stage, install, installed, prepare_install, size_label,
    uninstall,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreError(pub String);

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StoreError {}

fn error(message: impl Into<String>) -> StoreError {
    StoreError(message.into())
}
