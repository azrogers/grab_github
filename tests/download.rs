use std::path::Path;

use grab_github::{
    DownloadConfig, DownloadEvent, DownloadReporter, Downloader, Error, Filter, GithubBranchPath,
    SourceTree,
};
use sha1::{Digest, Sha1};

struct TestReporter;

impl DownloadReporter for TestReporter {
    fn on_event<'p>(&'p self, event: DownloadEvent<'p>) -> () {
        eprintln!("download reported event {:?}", event);
    }
}

pub async fn download_and_test<'p>(
    path: GithubBranchPath<'p>,
    filter: Filter<'p>,
    test: fn(&Path, &Vec<SourceTree>) -> Result<(), Error>,
) -> Result<(), Error> {
    let reporter = TestReporter {};
    let output_path = Path::new("./tests/test_output_dir/");
    if output_path.is_dir() {
        std::fs::remove_dir_all(output_path)?;
    }
    let result = async {
        let config = DownloadConfig::new_with_reporter(&output_path, &reporter);
        let files = Downloader::download(&config, &path, &filter).await?;
        test(&config.output_path, &files)
    }
    .await;

    if output_path.is_dir() {
        std::fs::remove_dir_all(output_path)?;
    }

    result
}

fn check_hash(dir: &Path, path: &Path, expected_hash: &'static str) -> Result<(), Error> {
    let mut combined = dir.to_path_buf();
    combined.push(path);

    assert!(combined.is_file());

    let bytes = std::fs::read(combined)?;

    let mut hash = Sha1::new();
    hash.update(&bytes);
    let result = hash.finalize();
    let hex = result
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("");

    if hex != expected_hash {
        return Err(Error::Other(format!(
            "hash {} != expected hash {}",
            hex, expected_hash
        )));
    }

    return Ok(());
}

#[tokio::test]
pub async fn hello_git_world() -> Result<(), Error> {
    download_and_test(
        GithubBranchPath::new("githubtraining", "hellogitworld", "master"),
        Filter::new(vec!["build.gradle"], vec![]),
        |path, _files| {
            check_hash(
                path,
                Path::new("build.gradle"),
                "d8a738144623ca437e35d781992cc75e1ee3b79c",
            )
        },
    )
    .await?;

    Ok(())
}
