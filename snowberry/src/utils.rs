use std::io::Read;

use crate::errors::SnowberryError;

/// Magic string for detecting that a file is serialized [`Package`]
pub const MAGIC_PACKAGE: &[u8; 5] = b"MASP\0";

/// The format version.
///
/// If future modifications are made to this format, the version should be incremented by 1.
pub const VERSION: [u8; 3] = [0, 0, 0];

pub fn parse_package_version<R: Read>(reader: &mut R) -> Result<semver::Version, SnowberryError> {
    let mut magic = [0u8; MAGIC_PACKAGE.len()];
    reader.read_exact(&mut magic).map_err(|_| SnowberryError::NotAPackage)?;

    if &magic != MAGIC_PACKAGE {
        return Err(SnowberryError::NotAPackage);
    }

    let mut version = [0u8; VERSION.len()];
    reader.read_exact(&mut version).map_err(|_| SnowberryError::NotAPackage)?;

    Ok(semver::Version::new(
        version[0] as u64,
        version[1] as u64,
        version[2] as u64,
    ))
}
