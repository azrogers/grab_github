# grab-github

A library to interact with GitHub's Get Tree and Get Blob APIs for the purposes of downloading a repository without git.

Downloading functionality is only suitable for small repositories at the moment as it quickly runs up against GitHub's secondary rate limits.

## Example Usage

```rust
use grab_github::{DownloadConfigNoReporting, Downloader, Filter, GithubBranchPath, SourceTree};
use std::path::Path;

// Specify the user, repository name, and branch (or commit hash)
let repo = GithubBranchPath::new("githubtraining", "hellogitworld", "master");

// Obtain the directory listing of githubtraining/hellogitworld
let tree = SourceTree::get(&repo).await?;

// Find a file in the directory tree with the given path.
let file = tree.resolve_blob(Path::new("build.gradle")).unwrap();

// GitHub personal access token will be filled from environment 
// variable `GITHUB_ACCESS_TOKEN` if set.
let config = DownloadConfigNoReporting::new(Path::new("output/data"));

// Download this file into the output directory.
Downloader::download_tree(&config, &file, &Filter::all()).await?;

// ...or you can do
Downloader::download_tree(&config, &tree, &Filter::new(vec!["build.gradle"], vec![])).await?;

// ...or just
Downloader::download(&config, &repo, &Filter::new(vec!["build.gradle"], vec![])).await?;
```