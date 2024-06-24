use futures::future::{BoxFuture, FutureExt};
use std::{
    cell::RefCell,
    collections::{HashMap, LinkedList},
    path::{Component, Path, PathBuf},
    rc::Rc,
};

use serde::Deserialize;

use crate::{request::HttpRequest, Error};

/// A GitHub branch URL.
/// The fields should complete the URL `https://github.com/{user}/{repo}/tree/{branch}`.
#[derive(Debug)]
pub struct GithubBranchPath<'g> {
    /// The GitHub username of the repository owner.
    pub user: &'g str,
    /// The repository name.
    pub repo: &'g str,
    /// The branch or SHA1 hash of the commit tree to fetch.
    pub branch: &'g str,
}

impl<'g> GithubBranchPath<'g> {
    /// Creates a new [GithubBranchPath] to the given user, repo, and branch.
    pub fn new(user: &'g str, repo: &'g str, branch: &'g str) -> GithubBranchPath<'g> {
        GithubBranchPath { user, repo, branch }
    }

    /// Creates a new [GithubBranchPath] with the given branch and the same user and repo as this path.
    pub fn with_branch(&self, branch: &'g str) -> GithubBranchPath<'g> {
        GithubBranchPath {
            user: self.user,
            repo: self.repo,
            branch,
        }
    }

    /// Returns the URL of the tree API for this branch path.
    fn to_tree_url(&self) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}",
            self.user, self.repo, self.branch
        )
    }
}

/// The type of a single entry in a [SourceTree].
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum TreeEntryType {
    /// A blob (file) entry.
    #[serde(rename = "blob")]
    Blob,
    /// A tree (directory) entry.
    #[serde(rename = "tree")]
    Tree,
}

/// A tree representing the directories and files of a GitHub repo.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceTree {
    /// The path of the file relative to the root of the repository.
    pub path: PathBuf,
    /// The unix permissions mode of the file, in numeric notation.  
    pub mode: String,
    /// The SHA1 hash identifying this blob or tree.
    ///
    /// This is NOT the same as the sha1 hash of the contents.
    pub sha: String,
    /// The type of the entry.
    pub entry_type: TreeEntryType,
    /// The size of the entry in bytes, or 0 for blob entries.
    pub size: u32,
    /// The API URL to call to get more information on this object.
    ///
    /// - For [TreeEntryType::Blob], this is the URL of the `Get a blob` API call for this entry.
    /// - For [TreeEntryType::Tree], this is the URL of the `Get a tree` API call for this entry.
    pub url: String,
    /// The children of this entry, if any.
    pub children: Vec<SourceTree>,
}

/// A type used while building a [SourceTree] from a [TreeModel].
#[derive(Clone)]
struct SourceTreeInter {
    pub path: PathBuf,
    pub mode: String,
    pub sha: String,
    pub entry_type: TreeEntryType,
    pub size: u32,
    pub url: String,
    pub children: Vec<Rc<RefCell<SourceTreeInter>>>,
}

impl SourceTree {
    /// Create a new empty [SourceTree] with the given [TreeEntryType].
    pub fn new(entry_type: TreeEntryType) -> SourceTree {
        SourceTree {
            url: String::new(),
            sha: String::new(),
            path: PathBuf::new(),
            mode: String::new(),
            entry_type,
            size: 0,
            children: Vec::new(),
        }
    }

    /// Obtain the entire [SourceTree] for a given [GithubBranchPath].
    pub async fn get<'p>(path: &'p GithubBranchPath<'p>) -> Result<SourceTree, Error> {
        let tree = TreeModel::get_tree(path).await?;
        Ok(tree.into())
    }

    /// Walks the tree to find a blob at the given path, if any.
    /// Equivalent to [resolve](SourceTree::resolve) with `find_blob` as `Some(true)`.
    pub fn resolve_blob(&self, path: &Path) -> Option<&SourceTree> {
        self.resolve(path, Some(true))
    }

    /// Walks the tree to find a tree (directory) at the given path, if any.
    /// Equivalent to [resolve](SourceTree::resolve) with `find_blob` as `Some(false)`.
    pub fn resolve_tree(&self, path: &Path) -> Option<&SourceTree> {
        self.resolve(path, Some(false))
    }

    /// Walks the tree to find a node at the given path, if any.
    /// Equivalent to [resolve](SourceTree::resolve) with `find_blob` as `None`.
    pub fn resolve_any(&self, path: &Path) -> Option<&SourceTree> {
        self.resolve(path, Some(false))
    }

    /// Walks the tree to find an entry at the given path, if any.
    ///
    /// - If `find_blob` is `Some(true)`, only blob entries will be returned.
    /// - If `find_blob` is `Some(false)`, only tree entries will be returned.
    /// - If `find_blob` is `None`, the first type of entry found will be returned.
    pub fn resolve(&self, path: &Path, find_blob: Option<bool>) -> Option<&SourceTree> {
        // we reverse the path because going parent->parent->parent is easier
        let components: Vec<Component> = path.components().into_iter().collect();
        self.resolve_inner(&components[..], find_blob)
    }

    fn resolve_inner(&self, path: &[Component], find_blob: Option<bool>) -> Option<&SourceTree> {
        if path.is_empty() {
            if let Some(find_blob) = find_blob {
                if (find_blob && self.entry_type != TreeEntryType::Blob)
                    || (!find_blob && self.entry_type != TreeEntryType::Tree)
                {
                    return None;
                }
            }

            return Some(self);
        }

        let file_name = &path[0];
        for c in &self.children {
            if let Some(name) = c.path.file_name() {
                if name == file_name.as_os_str() {
                    return c.resolve_inner(&path[1..], find_blob);
                }
            }
        }

        return None;
    }

    /// Creates a SourceTreeIterator that will walk down this tree and return a pointer for each node found.
    pub fn iter<'tree>(&'tree self) -> SourceTreeIterator<'tree> {
        let mut list = LinkedList::new();
        list.push_back((self, -1));
        SourceTreeIterator(list)
    }

    /// Creates a new [SourceTree] from this tree, only including child nodes where `f` returns true.
    pub fn prune(&self, predicate: for<'a> fn(&'a &SourceTree) -> bool) -> SourceTree {
        let new_children: Vec<SourceTree> = self
            .children
            .iter()
            .filter(predicate)
            .map(|c| c.prune(predicate))
            .collect();

        SourceTree {
            path: self.path.clone(),
            mode: self.mode.clone(),
            sha: self.sha.clone(),
            entry_type: self.entry_type.clone(),
            size: self.size,
            url: self.url.clone(),
            children: new_children,
        }
    }
}

/// An iterator for a [SourceTree] that walks the tree and returns a pointer to each node found.
pub struct SourceTreeIterator<'tree>(LinkedList<(&'tree SourceTree, isize)>);

impl<'tree> Iterator for SourceTreeIterator<'tree> {
    type Item = &'tree SourceTree;

    fn next(&mut self) -> Option<Self::Item> {
        let state = self.0.pop_back();
        if state.is_none() {
            return None;
        }

        let (node, mut pos) = state.unwrap();
        if pos >= (node.children.len() as isize) && self.0.is_empty() {
            // no children
            return None;
        }

        let ptr = match pos == -1 {
            true => node,
            false => &node.children[pos as usize],
        };

        pos = pos + 1;
        if pos < (node.children.len() as isize) {
            self.0.push_back((node, pos));
        }

        if !ptr.children.is_empty() {
            self.0.push_back((ptr, 0));
        }

        return Some(ptr);
    }
}

impl From<SourceTreeInter> for SourceTree {
    fn from(value: SourceTreeInter) -> Self {
        SourceTree {
            path: value.path,
            mode: value.mode,
            sha: value.sha,
            url: value.url,
            entry_type: value.entry_type,
            size: value.size,
            children: value
                .children
                .into_iter()
                .map(|s| s.borrow().clone().into())
                .collect(),
        }
    }
}

impl From<TreeEntryModel> for SourceTreeInter {
    fn from(value: TreeEntryModel) -> Self {
        SourceTreeInter {
            path: PathBuf::from(value.path),
            mode: value.mode,
            sha: value.sha,
            entry_type: value.entry_type,
            size: value.size,
            url: value.url,
            children: Vec::new(),
        }
    }
}

impl From<TreeModel> for SourceTree {
    fn from(value: TreeModel) -> Self {
        let mut dirs_for_path: HashMap<PathBuf, Rc<RefCell<SourceTreeInter>>> = HashMap::new();
        let mut nodes: LinkedList<Rc<RefCell<SourceTreeInter>>> = LinkedList::new();

        // read entries into nodes
        for entry in value.tree {
            match entry.entry_type {
                TreeEntryType::Blob => {
                    nodes.push_back(Rc::new(RefCell::new(entry.into())));
                }
                TreeEntryType::Tree => {
                    let path = entry.path.clone();
                    let entry = Rc::new(RefCell::new(entry.into()));
                    nodes.push_back(entry.clone());
                    dirs_for_path.insert(path.into(), entry);
                }
            }
        }

        let root = RefCell::new(SourceTreeInter {
            url: value.url,
            sha: value.sha,
            path: PathBuf::new(),
            mode: String::new(),
            entry_type: TreeEntryType::Tree,
            size: 0,
            children: Vec::new(),
        });

        // assign files to dir tree
        while let Some(node) = nodes.pop_front() {
            let node_ref = node.borrow();
            let path = Path::new(&node_ref.path);
            let dir_path: &Path = path.parent().unwrap_or(Path::new(""));
            let dir = match dir_path.as_os_str().len() {
                // use root node for
                0 => &root,
                _ => dirs_for_path.get(dir_path).unwrap(),
            };

            drop(node_ref);

            dir.borrow_mut().children.push(node);
        }

        root.into_inner().into()
    }
}

#[derive(Deserialize)]
struct TreeEntryModel {
    pub path: String,
    pub mode: String,
    #[serde(rename = "type")]
    pub entry_type: TreeEntryType,
    #[serde(default)]
    pub size: u32,
    pub sha: String,
    pub url: String,
}

/// The result of a call to the GitHub `Get a tree` API
#[derive(Deserialize)]
struct TreeModel {
    pub sha: String,
    pub url: String,
    pub tree: Vec<TreeEntryModel>,
    pub truncated: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TreeOrError {
    Tree(TreeModel),
    Error { message: String },
}

impl<'path> TreeModel {
    /// Obtains a tree first recursively, and then non-recursively if truncated.
    async fn get_tree(path: &GithubBranchPath<'path>) -> Result<TreeModel, Error> {
        let recursive_tree = TreeModel::get_tree_request(path, true).await?;
        if !recursive_tree.truncated {
            return Ok(recursive_tree);
        }

        let initial_tree = TreeModel::get_tree_request(path, false).await?;
        let mut entries: Vec<TreeEntryModel> = Vec::new();
        for entry in &initial_tree.tree {
            if entry.entry_type == TreeEntryType::Tree {
                TreeModel::get_tree_manual(path, &entry.path, &mut entries).await?;
            }
        }

        entries.extend(initial_tree.tree.into_iter());

        Ok(TreeModel {
            sha: initial_tree.sha,
            url: initial_tree.url,
            tree: entries,
            truncated: false,
        })
    }

    /// Recursively fills out the tree using the non-recursive version of the endpoint, collecting entries in `entries`.
    fn get_tree_manual<'a>(
        path: &'a GithubBranchPath<'path>,
        parent_entry_path: &'a str,
        entries: &'a mut Vec<TreeEntryModel>,
    ) -> BoxFuture<'a, Result<&'a mut Vec<TreeEntryModel>, Error>>
    where
        'path: 'a,
    {
        // have to use boxed async here because we're calling an async recursively
        async move {
            let model = TreeModel::get_tree_request(path, false).await?;
            for entry in &model.tree {
                if entry.entry_type == TreeEntryType::Tree {
                    TreeModel::get_tree_manual(
                        &path.with_branch(&entry.sha),
                        &format!("{}/{}", parent_entry_path, entry.path),
                        entries,
                    )
                    .await?;
                }
            }

            entries.extend(model.tree.into_iter());

            Ok(entries)
        }
        .boxed()
    }

    /// Makes a request to the get tree endpoint
    async fn get_tree_request(
        path: &GithubBranchPath<'path>,
        recursive: bool,
    ) -> Result<TreeModel, Error> {
        let url = path.to_tree_url();

        let client = HttpRequest::client(&None)?;
        let request = match recursive {
            true => client.get(url).query(&[("recursive", true)]),
            false => client.get(url),
        };

        let request = request
            .header("Accept", "application/vnd.github+json")
            .build()?;

        let response = client.execute(request).await?;
        let body = response.text().await?;

        let result = serde_json::from_str::<TreeOrError>(&body)?;
        match result {
            TreeOrError::Error { message } => Err(Error::GithubError(message)),
            TreeOrError::Tree(t) => Ok(t),
        }
    }
}
