use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::cli::Options;
use crate::errors::SnowberryError;

/// Magic string for detecting that a file is serialized [`Package`]
pub const MAGIC_PACKAGE: &[u8; 5] = b"MASP\0";

/// The maximum supported format version.
pub const MAX_VERSION: [u8; 3] = [4, 0, 0];

pub type FieldName = String;
pub type FieldValue = String;

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
        // We append these since they have been already read.
        data.extend_from_slice(&magic);
        data.extend_from_slice(&version_bytes);
        // Append the remaining bytes.
        reader
            .read_to_end(&mut data)
            .map_err(|_| SnowberryError::NotAPackage { found: magic })?;

        Ok(Self { version, data })
    }

    pub fn from_file(path: &Path, options: &Options) -> Result<Self, SnowberryError> {
        if options.verbose && path.extension().is_none_or(|ext| ext != "masp") {
            eprintln!("warning: file extension is not .masp");
        }
        let mut file =
            File::open(path).map_err(|_| SnowberryError::NotAPackage { found: [0; 5] })?;
        Self::from_bytes(&mut file)
    }
}

impl PackageWrapper {
    /// Returns package fields as key-value string pairs.
    ///
    /// We use `Vec<(String, String)>` because different package versions expose different
    /// fields, so a flexible representation lets each version report only the fields it has.
    pub fn info(&self) -> Result<Vec<(FieldName, FieldValue)>, SnowberryError> {
        match (self.version.major, self.version.minor, self.version.patch) {
            (0, 0, 0) => self.info_v13(),
            (1, 0, 0) => self.info_v17(),
            (2, 0, 0) => self.info_v18(),
            (3, 0, 0) => self.info_v20(),
            (4, 0, 0) => self.info_v22(),
            _ => unreachable!(),
        }
    }

    fn info_v13(&self) -> Result<Vec<(FieldName, FieldValue)>, SnowberryError> {
        use miden_core_v13::utils::{Deserializable, SliceReader};

        let mut reader = SliceReader::new(&self.data);
        let package = miden_mast_package_v13::Package::read_from(&mut reader)
            .map_err(|e| SnowberryError::DeserializationFailed { reason: e.to_string() })?;

        let name = ("name".into(), package.name.clone());
        let digest = ("digest".into(), format!("{:?}", package.digest()));
        let exports = ("exports".into(), package.manifest.exports.len().to_string());
        let dependencies = (
            "dependencies".into(),
            package.manifest.dependencies.len().to_string(),
        );

        Ok(vec![name, digest, exports, dependencies])
    }

    fn info_v17(&self) -> Result<Vec<(FieldName, FieldValue)>, SnowberryError> {
        use miden_core_v17::utils::{Deserializable, SliceReader};

        let mut reader = SliceReader::new(&self.data);
        let package = miden_mast_package_v17::Package::read_from(&mut reader)
            .map_err(|e| SnowberryError::DeserializationFailed { reason: e.to_string() })?;

        let name = ("name".into(), package.name.clone());
        let digest = ("digest".into(), format!("{:x?}", package.digest()));
        let exports = ("exports".into(), package.manifest.num_exports().to_string());
        let dependencies = (
            "dependencies".into(),
            package.manifest.num_dependencies().to_string(),
        );

        Ok(vec![name, digest, exports, dependencies])
    }

    fn info_v18(&self) -> Result<Vec<(FieldName, FieldValue)>, SnowberryError> {
        use miden_core_v18::utils::{Deserializable, SliceReader};

        let mut reader = SliceReader::new(&self.data);
        let package = miden_mast_package_v18::Package::read_from(&mut reader)
            .map_err(|e| SnowberryError::DeserializationFailed { reason: e.to_string() })?;

        let name = ("name".into(), package.name.clone());
        let version = (
            "version".into(),
            package
                .version
                .as_ref()
                .map_or("none".into(), |v| v.to_string()),
        );
        let description = (
            "description".into(),
            package.description.clone().unwrap_or("none".into()),
        );
        let digest = ("digest".into(), format!("{:x?}", package.digest()));
        let exports = ("exports".into(), package.manifest.num_exports().to_string());
        let dependencies = (
            "dependencies".into(),
            package.manifest.num_dependencies().to_string(),
        );
        let sections = (
            "sections".into(),
            package
                .sections
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );

        Ok(vec![
            name,
            version,
            description,
            digest,
            exports,
            dependencies,
            sections,
        ])
    }

    fn info_v20(&self) -> Result<Vec<(FieldName, FieldValue)>, SnowberryError> {
        use miden_core_v20::utils::{Deserializable, SliceReader};

        let mut reader = SliceReader::new(&self.data);
        let package = miden_mast_package_v20::Package::read_from(&mut reader)
            .map_err(|e| SnowberryError::DeserializationFailed { reason: e.to_string() })?;

        let name = ("name".into(), package.name.clone());
        let version = (
            "version".into(),
            package
                .version
                .as_ref()
                .map_or("none".into(), |v| v.to_string()),
        );
        let description = (
            "description".into(),
            package.description.clone().unwrap_or("none".into()),
        );
        let kind = ("kind".into(), package.kind.to_string());
        let digest = ("digest".into(), format!("{:x?}", package.digest()));
        let exports = ("exports".into(), package.manifest.num_exports().to_string());
        let dependencies = (
            "dependencies".into(),
            package.manifest.num_dependencies().to_string(),
        );
        let sections = (
            "sections".into(),
            package
                .sections
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );

        Ok(vec![
            name,
            version,
            description,
            kind,
            digest,
            exports,
            dependencies,
            sections,
        ])
    }

    fn info_v22(&self) -> Result<Vec<(FieldName, FieldValue)>, SnowberryError> {
        use miden_core_v22::serde::{Deserializable, SliceReader};

        let mut reader = SliceReader::new(&self.data);
        let package = miden_mast_package_v22::Package::read_from(&mut reader)
            .map_err(|e| SnowberryError::DeserializationFailed { reason: e.to_string() })?;

        let name = ("name".into(), package.name.to_string());
        let version = ("version".into(), package.version.to_string());
        let description = (
            "description".into(),
            package.description.clone().unwrap_or("none".into()),
        );
        let kind = ("kind".into(), package.kind.to_string());
        let digest = ("digest".into(), format!("{:x?}", package.digest()));
        let exports = ("exports".into(), package.manifest.num_exports().to_string());
        let dependencies = (
            "dependencies".into(),
            package.manifest.num_dependencies().to_string(),
        );
        let sections = (
            "sections".into(),
            package
                .sections
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );

        Ok(vec![
            name,
            version,
            description,
            kind,
            digest,
            exports,
            dependencies,
            sections,
        ])
    }
}
