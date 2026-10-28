//! Why a file could not be opened or saved, in words for the musician. The
//! system's or the parser's own English text never reaches `Display`; it stays
//! in [`FileError::detail`] for logs.
use std::{fmt, io};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileErrorKind {
    NotFound,
    PermissionDenied,
    ReadOnly,
    /// Unreadable, cut short, or not a score at all.
    Damaged,
    /// Written by a newer Levare, with a format this one does not know.
    NewerVersion,
    TooLarge,
    DiskFull,
    /// Any other failure reading or writing.
    Io,
    /// Readable, but refused for a reason the message gives.
    Invalid,
}

#[derive(Debug)]
pub struct FileError {
    kind: FileErrorKind,
    message: String,
    detail: Option<String>,
}

impl FileError {
    pub(crate) fn new(kind: FileErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl fmt::Display) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn kind(&self) -> FileErrorKind {
        self.kind
    }

    /// The underlying technical cause, in English, when there is one.
    pub fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    pub(crate) fn damaged(detail: impl fmt::Display) -> Self {
        Self::damaged_with(
            "el archivo está dañado o no es una partitura de Levare",
            detail,
        )
    }

    pub(crate) fn damaged_with(message: impl Into<String>, detail: impl fmt::Display) -> Self {
        Self::new(FileErrorKind::Damaged, message).with_detail(detail)
    }

    pub(crate) fn newer(version: u64) -> Self {
        Self::new(
            FileErrorKind::NewerVersion,
            format!(
                "la partitura se creó con una versión más reciente de Levare (formato {version}); actualiza Levare para abrirla"
            ),
        )
    }

    pub(crate) fn too_large(message: impl Into<String>) -> Self {
        Self::new(FileErrorKind::TooLarge, message)
    }

    /// A failure of Levare itself rather than of the file.
    pub(crate) fn internal(detail: impl fmt::Display) -> Self {
        Self::new(FileErrorKind::Io, "Levare no pudo completar la operación").with_detail(detail)
    }
}

impl From<io::Error> for FileError {
    fn from(error: io::Error) -> Self {
        if let Some(explained) = atomic_write::explanation(&error) {
            let kind = if explained.read_only {
                FileErrorKind::ReadOnly
            } else {
                kind_of(error.kind())
            };
            return Self::new(kind, explained.message.clone()).with_detail(&error);
        }
        let kind = kind_of(error.kind());
        let message = match error.kind() {
            io::ErrorKind::IsADirectory => "es una carpeta, no un archivo",
            io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
                "el archivo está dañado o incompleto"
            }
            _ => match kind {
                FileErrorKind::NotFound => "el archivo o su carpeta ya no existe",
                FileErrorKind::PermissionDenied => {
                    "Levare no tiene permiso para acceder al archivo o a su carpeta"
                }
                FileErrorKind::ReadOnly => "el disco o la carpeta es de solo lectura",
                FileErrorKind::DiskFull => "no queda espacio en el disco",
                FileErrorKind::TooLarge => "el archivo es demasiado grande para este disco",
                _ => "el sistema no pudo leer o escribir el archivo",
            },
        };
        Self::new(kind, message).with_detail(&error)
    }
}

fn kind_of(kind: io::ErrorKind) -> FileErrorKind {
    match kind {
        io::ErrorKind::NotFound => FileErrorKind::NotFound,
        io::ErrorKind::PermissionDenied => FileErrorKind::PermissionDenied,
        io::ErrorKind::ReadOnlyFilesystem => FileErrorKind::ReadOnly,
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => FileErrorKind::DiskFull,
        io::ErrorKind::FileTooLarge => FileErrorKind::TooLarge,
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => FileErrorKind::Damaged,
        _ => FileErrorKind::Io,
    }
}

impl From<score_core::ScoreError> for FileError {
    fn from(error: score_core::ScoreError) -> Self {
        Self::new(FileErrorKind::Invalid, error.0)
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for FileError {}

/// A refusal whose reason is already written for the musician, in Spanish.
pub(crate) fn error(message: impl Into<String>) -> FileError {
    FileError::new(FileErrorKind::Invalid, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_errors_are_explained_without_their_english_text() {
        for (kind, expected) in [
            (io::ErrorKind::NotFound, FileErrorKind::NotFound),
            (
                io::ErrorKind::PermissionDenied,
                FileErrorKind::PermissionDenied,
            ),
            (io::ErrorKind::ReadOnlyFilesystem, FileErrorKind::ReadOnly),
            (io::ErrorKind::StorageFull, FileErrorKind::DiskFull),
            (io::ErrorKind::Other, FileErrorKind::Io),
        ] {
            let system = io::Error::from(kind);
            let error = FileError::from(system);
            assert_eq!(error.kind(), expected);
            let english = io::Error::from(kind).to_string();
            assert!(!error.to_string().contains(&english), "{error}");
            assert_eq!(error.detail(), Some(english.as_str()));
        }
        #[cfg(unix)]
        {
            // EACCES and ENOENT, as File::open reports them.
            for code in [13, 2] {
                let error = FileError::from(io::Error::from_raw_os_error(code));
                assert!(!error.to_string().contains("os error"), "{error}");
                assert!(!error.to_string().contains("Permission"), "{error}");
                assert!(!error.to_string().contains("No such file"), "{error}");
            }
        }
    }

    #[test]
    fn files_that_cannot_be_opened_say_why_in_spanish() {
        let directory = tempfile::tempdir().unwrap();
        let missing = crate::load(&directory.path().join("Perdida.levare")).unwrap_err();
        assert_eq!(missing.kind(), FileErrorKind::NotFound);
        assert_eq!(missing.to_string(), "el archivo o su carpeta ya no existe");
        let damaged = crate::decode(b"{\"format\": \"partitura\", \"version\"").unwrap_err();
        assert_eq!(damaged.kind(), FileErrorKind::Damaged);
        assert!(!damaged.to_string().contains("EOF"), "{damaged}");
        assert!(damaged.detail().is_some());
    }
}
