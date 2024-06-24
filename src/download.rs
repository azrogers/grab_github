use base64::{prelude::BASE64_STANDARD, Engine};
use futures::{
    future::{self, BoxFuture},
    FutureExt,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

use crate::{request::HttpRequest, Error, Filter, GithubBranchPath, SourceTree, TreeEntryType};

#[derive(Deserialize)]
struct GithubBlobModel {
    sha: String,
    node_id: String,
    size: usize,
    url: String,
    content: String,
    encoding: String,
}

/// An event that's occured involving a single download.
#[derive(Debug)]
pub enum DownloadEvent<'p> {
    DownloadStarted { path: &'p str },
    DownloadCompleted { path: &'p str },
    DownloadFailed { path: &'p str, error: Error },
}

/// Implement this trait to receive events on the status of each upload.
pub trait DownloadReporter: Sync {
    fn on_event<'p>(&'p self, event: DownloadEvent<'p>) -> ();
}

const DEFAULT_MAX_DOWNLOADS: usize = 10;

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
    /// The default is 10.
    pub max_simultaneous_downloads: usize,
}

impl<'download, Reporter> DownloadConfig<'download, Reporter>
where
    Reporter: DownloadReporter,
{
    /// Creates a new [DownloadConfig] with the given output path and default values.
    pub fn new(output_path: &'download Path) -> DownloadConfig<'download, Reporter> {
        DownloadConfig {
            output_path,
            reporter: None,
            max_simultaneous_downloads: DEFAULT_MAX_DOWNLOADS,
        }
    }

    /// Creates a new [DownloadConfig] with the given output path, reporter, and default values.
    pub fn new_with_reporter(
        output_path: &'download Path,
        reporter: &'download Reporter,
    ) -> DownloadConfig<'download, Reporter> {
        DownloadConfig {
            output_path,
            reporter: Some(reporter),
            max_simultaneous_downloads: DEFAULT_MAX_DOWNLOADS,
        }
    }
}

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
        let output_path = config.output_path;

        let files: Vec<&SourceTree> = tree
            .iter()
            .filter(|n| {
                n.entry_type == TreeEntryType::Blob && filter.check(n.path.to_str().unwrap_or(""))
            })
            .collect();

        let active_count = 0usize;
        let mut active: Vec<BoxFuture<'p, Result<(), Error>>> = Vec::new();

        for f in &files {
            if active_count > config.max_simultaneous_downloads {
                // make sure some active downloads complete before starting new ones
                let (_, index, _) = future::select_all(&mut active).await;
                active.remove(index).await?;
            }

            let next =
                Downloader::download_node_wrapper(&config.reporter, output_path.to_path_buf(), f);
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
        output_path: PathBuf,
        tree: &'p SourceTree,
    ) -> Result<(), Error> {
        let path = tree.path.to_str().unwrap();
        if let Some(reporter) = *reporter {
            reporter.on_event(DownloadEvent::DownloadStarted { path })
        }
        let result = Downloader::download_node(&output_path, &tree).await;

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

    async fn download_node(output_path: &'p Path, tree: &'p SourceTree) -> Result<(), Error> {
        let client = HttpRequest::client()?;
        let request = client.get(&tree.url).build()?;
        let response = client.execute(request).await?;
        let body = response.text().await?;

        let model: GithubBlobModel = serde_json::from_str(&body)?;
        let base64_str: String = model.content.chars().filter(|c| *c != '\n').collect();
        let bytes = BASE64_STANDARD.decode(base64_str.as_bytes())?;

        let mut output_path = output_path.to_path_buf();
        output_path.push(&tree.path);

        Downloader::write_file(&output_path, &bytes).await?;
        Ok(())
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
