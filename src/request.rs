use const_format::formatcp;
use reqwest::{header, Client, ClientBuilder, RequestBuilder};

use crate::Error;

const USER_AGENT: &'static str =
    formatcp!("azrogers/grab_github version {}", env!("CARGO_PKG_VERSION"));

pub struct HttpRequest {}

impl HttpRequest {
    /// Creates a GET request for the given URL.
    pub fn get(url: &str) -> Result<RequestBuilder, Error> {
        Ok(Self::client()?.get(url))
    }

    /// Creates a [reqwest::Client] with the default settings.
    pub fn client() -> Result<Client, Error> {
        let mut headers = header::HeaderMap::new();
        headers.insert("User-Agent", header::HeaderValue::from_static(USER_AGENT));

        Ok(ClientBuilder::new().default_headers(headers).build()?)
    }
}
