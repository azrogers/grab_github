use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum Error {
    RequestError(Arc<reqwest::Error>),
    DeserializationError(Arc<serde_json::Error>),
    IOError(Arc<std::io::Error>),
    Base64Error(Arc<base64::DecodeError>),
    Other(String),
}

impl From<reqwest::Error> for Error {
    fn from(value: reqwest::Error) -> Self {
        Error::RequestError(value.into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Error::DeserializationError(value.into())
    }
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::IOError(value.into())
    }
}

impl From<base64::DecodeError> for Error {
    fn from(value: base64::DecodeError) -> Self {
        Error::Base64Error(value.into())
    }
}
