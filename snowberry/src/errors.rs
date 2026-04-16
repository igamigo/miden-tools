use std::fmt;

#[derive(Debug)]
pub enum SnowberryError {
    NotAPackage,
}

impl fmt::Display for SnowberryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnowberryError::NotAPackage => write!(f, "file is not a valid package"),
        }
    }
}
