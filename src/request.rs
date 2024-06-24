use std::borrow::Cow;

use const_format::formatcp;
use reqwest::{header, Client, ClientBuilder};

use crate::Error;

const USER_AGENT: &'static str =
    formatcp!("azrogers/grab_github version {}", env!("CARGO_PKG_VERSION"));

pub struct HttpRequest {}

impl HttpRequest {
    /// Creates a [reqwest::Client] with the default settings.
    pub fn client<'src>(access_token: &Option<Cow<'src, str>>) -> Result<Client, Error> {
        let mut headers = header::HeaderMap::new();
        headers.insert("User-Agent", header::HeaderValue::from_static(USER_AGENT));
        if let Some(access_token) = access_token {
            headers.insert(
                "Authorization",
                header::HeaderValue::from_str(&format!("token {}", access_token))
                    .map_err(|e| Error::Other(e.to_string()))?,
            );

            headers.insert(
                "X-GitHub-Api-Version",
                header::HeaderValue::from_static("2022-11-28"),
            );
        }

        Ok(ClientBuilder::new().default_headers(headers).build()?)
    }
}
