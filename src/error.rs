use std::sync::Arc;

/// Encapsulates an error value from grab_github or one of its dependencies.
#[derive(Debug, Clone)]
pub enum Error {
    /// An error occurred during an HTTP request.
    RequestError(Arc<reqwest::Error>),
    /// An error occurred while deserializing a JSON response.
    DeserializationError(Arc<serde_json::Error>),
    /// An error occured when trying to perform a filesystem operation.
    IOError(Arc<std::io::Error>),
    /// An error occurred while trying to decode base64 obtained from GitHub.
    Base64Error(Arc<base64::DecodeError>),
    /// An error occurred with a GitHub API request (usually a rate limit error).
    GithubError(String),
    /// Some other error occurred.
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
