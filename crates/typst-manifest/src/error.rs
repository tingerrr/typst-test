//! Error types that may be encountered during manifest discovery or parsing.

use std::fmt::Display;
use std::io;

pub use toml::de::Error as DeserializeError;
pub use toml::ser::Error as SerializeError;

/// An error that may occur during manifest discovery or parsing.
#[derive(Debug)]
pub enum Error {
    /// A generic I/O error occured.
    Io(io::Error),

    /// A serialization error occured.
    Ser(SerializeError),

    /// A deserialization error occured.
    De(DeserializeError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "an I/O error occured",
            Self::Ser(_) => "serialization failed",
            Self::De(_) => "deserialization failed",
        })
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Error::Io(err) => err,
            Error::Ser(err) => err,
            Error::De(err) => err,
        })
    }
}

macro_rules! impl_from {
    ($err:ty => $var:ident) => {
        impl From<$err> for Error {
            fn from(err: $err) -> Self {
                Self::$var(err)
            }
        }
    };
}

impl_from!(io::Error => Io);
impl_from!(SerializeError => Ser);
impl_from!(DeserializeError => De);
