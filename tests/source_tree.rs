use std::path::{Path, PathBuf};

use grab_github::{Error, GithubBranchPath, SourceTree, TreeEntryType};

#[tokio::test]
pub async fn hello_git_world() -> Result<(), Error> {
    let tree = SourceTree::get(&GithubBranchPath::new(
        "githubtraining",
        "hellogitworld",
        "master",
    ))
    .await?;

    assert_eq!(
    tree.resolve_blob(Path::new("build.gradle")),
    Some(&SourceTree {
        path: PathBuf::from("build.gradle"),
        mode: String::from("100644"),
        entry_type: TreeEntryType::Blob,
        size: 112,
        sha: String::from("6058be211566308428ca6dcab3f08cf270cd9568"),
        url: String::from("https://api.github.com/repos/githubtraining/hellogitworld/git/blobs/6058be211566308428ca6dcab3f08cf270cd9568"),
        children: Vec::new()
    }));

    let dir = tree
        .resolve_tree(Path::new("src/test/java/com/github"))
        .unwrap();
    assert_eq!(dir.children[0], SourceTree {
        path: PathBuf::from("src/test/java/com/github/AppTest.java"),
        mode: String::from("100644"),
        entry_type: TreeEntryType::Blob,
        size: 750,
        sha: String::from("43767197a768385d97ce751c421ee9e7ceeda5a7"),
        url: String::from("https://api.github.com/repos/githubtraining/hellogitworld/git/blobs/43767197a768385d97ce751c421ee9e7ceeda5a7"),
        children: Vec::new()
	});

    Ok(())
}
