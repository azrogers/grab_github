use base64::{prelude::BASE64_STANDARD, Engine};
use futures::{
    future::{self, BoxFuture},
    FutureExt,
};
use itertools::Itertools;
use serde::Deserialize;
use std::{
    borrow::Cow,
    env,
    path::{Path, PathBuf},
};

use crate::{request::HttpRequest, Error, Filter, GithubBranchPath, SourceTree, TreeEntryType};

/// An event that's occured involving a single download.
#[derive(Debug)]
pub enum DownloadEvent<'p> {
    DownloadStarted { path: &'p str },
    DownloadCompleted { path: &'p str },
    DownloadFailed { path: &'p str, error: Error },
}

/// Implement this trait to receive events on the status of each upload.
pub trait DownloadReporter: Sync {
    fn on_event<'p>(&'p self, _event: DownloadEvent<'p>) -> () {}
}

/// An empty download reporter that does nothing
pub struct NullDownloadReporter {}

impl DownloadReporter for NullDownloadReporter {}

const DEFAULT_MAX_DOWNLOADS: usize = 5;

/// Contains the configuration for a downloading operation.
pub struct DownloadConfig<'download, Reporter>
where
    Reporter: DownloadReporter,
{
    /// The directory that the tree will be downloaded into.
    pub output_path: &'download Path,
    /// If provided, the reporter will receive events on the status of each download.
    pub reporter: Option<&'download Reporter>,
    /// The maximum number of simultaneous downloads allowed at once.
    /// The default is 5.
    pub max_simultaneous_downloads: usize,
    /// Your GitHub personal access token, if you have one.
    pub access_token: Option<Cow<'download, str>>,
}

impl<'download, Reporter> DownloadConfig<'download, Reporter>
where
    Reporter: DownloadReporter,
{
    /// Creates a new [DownloadConfig] with the given output path and default values.
    ///
    /// `access_token` will be read from the environment variable `GITHUB_ACCESS_TOKEN` if available.
    pub fn new(output_path: &'download Path) -> DownloadConfig<'download, Reporter> {
        let access_token = env::var("GITHUB_ACCESS_TOKEN")
            .ok()
            .and_then(|s| Some(Cow::from(s)));

        DownloadConfig {
            output_path,
            reporter: None,
            max_simultaneous_downloads: DEFAULT_MAX_DOWNLOADS,
            access_token: access_token,
        }
    }

    /// Creates a new [DownloadConfig] with the given output path, reporter, and default values.
    ///
    /// `access_token` will be read from the environment variable `GITHUB_ACCESS_TOKEN` if available.
    pub fn new_with_reporter(
        output_path: &'download Path,
        reporter: &'download Reporter,
    ) -> DownloadConfig<'download, Reporter> {
        let access_token = env::var("GITHUB_ACCESS_TOKEN")
            .ok()
            .and_then(|s| Some(Cow::from(s)));

        DownloadConfig {
            output_path,
            reporter: Some(reporter),
            max_simultaneous_downloads: DEFAULT_MAX_DOWNLOADS,
            access_token,
        }
    }
}

/// A convenience type for a download config with no reporter.
pub type DownloadConfigNoReporting<'download> = DownloadConfig<'download, NullDownloadReporter>;

pub struct Downloader {}

impl<'p> Downloader {
    /// Downloads an entire GitHub tree specified by `path`.
    pub async fn download<Reporter: DownloadReporter>(
        config: &'p DownloadConfig<'p, Reporter>,
        path: &GithubBranchPath<'p>,
        filter: &Filter<'p>,
    ) -> Result<Vec<SourceTree>, Error> {
        let tree = SourceTree::get(path).await?;
        let files = Downloader::download_tree(config, &tree, filter).await?;
        Ok(files.into_iter().map(|s| s.clone()).collect())
    }

    /// Downloads an entire [SourceTree] to a directory.
    pub async fn download_tree<Reporter: DownloadReporter>(
        config: &'p DownloadConfig<'p, Reporter>,
        tree: &'p SourceTree,
        filter: &Filter<'p>,
    ) -> Result<Vec<&'p SourceTree>, Error> {
        Ok(Downloader::download_tree_iter(config, tree.iter(), filter).await?)
    }

    /// Downloads an iterator of [SourceTree] nodes to a directory.
    pub async fn download_tree_iter<Reporter, Iter>(
        config: &'p DownloadConfig<'p, Reporter>,
        iter: Iter,
        filter: &Filter<'p>,
    ) -> Result<Vec<&'p SourceTree>, Error>
    where
        Reporter: DownloadReporter,
        Iter: IntoIterator<Item = &'p SourceTree>,
    {
        let output_path = config.output_path;
        let access_token = &config.access_token;

        let files: Vec<&SourceTree> = iter
            .into_iter()
            .filter(|n| {
                n.entry_type == TreeEntryType::Blob && filter.check(n.path.to_str().unwrap_or(""))
            })
            .collect();

        let mut active: Vec<BoxFuture<'p, Result<(), Error>>> = Vec::new();

        for f in &files {
            if active.len() > config.max_simultaneous_downloads {
                // make sure some active downloads complete before starting new ones
                let (result, index, _) = future::select_all(&mut active).await;
                result?;
                let _future = active.remove(index);
            }

            let next = Downloader::download_node_wrapper(
                &config.reporter,
                &access_token,
                output_path.to_path_buf(),
                f,
            );
            active.push(next.boxed());
        }

        for r in future::join_all(active).await {
            if let Err(e) = r {
                return Err(e);
            }
        }

        Ok(files)
    }

    async fn download_node_wrapper<Reporter: DownloadReporter>(
        reporter: &'p Option<&'p Reporter>,
        access_token: &'p Option<Cow<'p, str>>,
        output_path: PathBuf,
        tree: &'p SourceTree,
    ) -> Result<(), Error> {
        let path = tree.path.to_str().unwrap();
        if let Some(reporter) = *reporter {
            reporter.on_event(DownloadEvent::DownloadStarted { path })
        }

        let result = Downloader::download_node(&access_token, &output_path, &tree).await;

        if let Some(reporter) = *reporter {
            match result {
                Ok(_) => reporter.on_event(DownloadEvent::DownloadCompleted { path }),
                Err(ref e) => reporter.on_event(DownloadEvent::DownloadFailed {
                    path,
                    error: e.clone(),
                }),
            }
        };

        result
    }

    async fn download_node(
        access_token: &'p Option<Cow<'p, str>>,
        output_path: &'p Path,
        tree: &'p SourceTree,
    ) -> Result<(), Error> {
        let client = HttpRequest::client(access_token)?;
        let request = client.get(&tree.url).build()?;
        let str = request
            .headers()
            .iter()
            .map(|(name, val)| format!("{} = {:?}", name, val))
            .join(", ");
        println!("{}", str);
        let response = client.execute(request).await?;
        let body = response.text().await?;

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum BlobOrError {
            Blob { content: String },
            Error { message: String },
        }

        let model: BlobOrError = serde_json::from_str(&body)?;
        match model {
            BlobOrError::Error { message } => Err(Error::GithubError(message)),
            BlobOrError::Blob { content } => {
                let base64_str: String = content.chars().filter(|c| *c != '\n').collect();
                let bytes = BASE64_STANDARD.decode(base64_str.as_bytes())?;

                let output_path = output_path.to_path_buf().join(&tree.path);

                Downloader::write_file(&output_path, &bytes).await?;
                Ok(())
            }
        }
    }

    async fn write_file(path: &Path, bytes: &[u8]) -> Result<(), Error> {
        Downloader::ensure_dir_exists(path).await?;

        tokio::fs::write(path, &bytes).await?;

        Ok(())
    }

    async fn ensure_dir_exists(path: &Path) -> Result<(), Error> {
        let dirname = path.parent();
        if dirname.is_none() {
            return Ok(());
        }

        let dirname = dirname.unwrap();
        if dirname.exists() {
            return Ok(());
        }

        tokio::fs::create_dir_all(dirname).await?;

        Ok(())
    }
}
