/// Why an update check or download failed. None of these install anything:
/// a download is only kept once it is fully verified. The messages end up in
/// the status bar, so they are in Spanish.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("las actualizaciones deben llegar por https: {0}")]
    InsecureUrl(String),
    #[error("no se pudo contactar con el servidor de actualizaciones: {0}")]
    Http(#[from] reqwest::Error),
    #[error("el servidor de actualizaciones respondió {status} para {url}")]
    Status { status: u16, url: String },
    #[error("el manifiesto de actualizaciones no es válido: {0}")]
    Manifest(String),
    #[error("«{0}» no es una versión")]
    InvalidVersion(String),
    #[error("no hay actualización publicada para {0}")]
    UnsupportedPlatform(String),
    #[error("la descarga supera {0} bytes")]
    TooLarge(u64),
    #[error("el SHA-256 de la descarga no coincide con el manifiesto")]
    Sha256Mismatch,
    #[error("la firma de la actualización no es válida: {0}")]
    Signature(String),
    #[error("la firma es de «{found}», no de «{expected}»")]
    SignedFor { expected: String, found: String },
    #[error("no se pudo instalar la actualización: {0}")]
    Install(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
