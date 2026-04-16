use thiserror::Error;

#[derive(Debug, Error)]
pub enum SnowberryError {
    #[error("failed to read magic bytes: only read {found} of 5 bytes")]
    WrongMagic { found: usize },
    #[error("not a valid package: expected magic bytes MASP\\0, found {found:02x?}")]
    NotAPackage { found: [u8; 5] },
    #[error("unsupported package version: {version} (max supported: {max_supported})")]
    UnsupportedVersion {
        version: semver::Version,
        max_supported: semver::Version,
    },
    #[error("failed to deserialize package: {reason}")]
    DeserializationFailed { reason: String },
}
