use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::cli::Options;
use crate::errors::SnowberryError;

/// Magic string for detecting that a file is serialized [`Package`]
pub const MAGIC_PACKAGE: &[u8; 5] = b"MASP\0";

/// The maximum supported format version.
pub const MAX_VERSION: [u8; 3] = [4, 0, 0];

pub struct PackageWrapper {
    pub version: semver::Version,
    pub data: Vec<u8>,
}

impl PackageWrapper {
    pub fn from_bytes<R: Read>(reader: &mut R) -> Result<Self, SnowberryError> {
        let mut magic = [0u8; MAGIC_PACKAGE.len()];
        let bytes_read = reader
            .read(&mut magic)
            .map_err(|_| SnowberryError::WrongMagic { found: 0 })?;

        if bytes_read < MAGIC_PACKAGE.len() {
            return Err(SnowberryError::WrongMagic { found: bytes_read });
        }

        if &magic != MAGIC_PACKAGE {
            return Err(SnowberryError::NotAPackage { found: magic });
        }

        let mut version_bytes = [0u8; MAX_VERSION.len()];
        reader
            .read_exact(&mut version_bytes)
            .map_err(|_| SnowberryError::NotAPackage { found: magic })?;

        if version_bytes > MAX_VERSION {
            return Err(SnowberryError::UnsupportedVersion {
                version: semver::Version::new(
                    version_bytes[0] as u64,
                    version_bytes[1] as u64,
                    version_bytes[2] as u64,
                ),
                max_supported: semver::Version::new(
                    MAX_VERSION[0] as u64,
                    MAX_VERSION[1] as u64,
                    MAX_VERSION[2] as u64,
                ),
            });
        }

        let version = semver::Version::new(
            version_bytes[0] as u64,
            version_bytes[1] as u64,
            version_bytes[2] as u64,
        );

        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|_| SnowberryError::NotAPackage { found: magic })?;

        Ok(Self { version, data })
    }

    pub fn from_file(path: &Path, options: &Options) -> Result<Self, SnowberryError> {
        if options.verbose && path.extension().is_none_or(|ext| ext != "masp") {
            eprintln!("warning: file extension is not .masp");
        }
        let mut file = File::open(path).map_err(|_| SnowberryError::NotAPackage {
            found: [0; 5],
        })?;
        Self::from_bytes(&mut file)
    }
}
