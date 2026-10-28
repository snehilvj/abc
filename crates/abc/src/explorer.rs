//! The file explorer's state for one worktree: abcd's file list as a tree, and
//! tui-tree-widget's `TreeState` for what's open, selected and scrolled. Drawing
//! is in `ui::explorer`. Nothing here reads the disk.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

use tui_tree_widget::{TreeItem, TreeState};

use crate::theme::Theme;

/// A worktree's files, as abcd lists them.
#[derive(Debug, Clone)]
pub enum Listing {
    Loading,
    Failed(String),
    Ready(Box<Explorer>),
}

/// A file or folder, by the names of the folders above it and its own.
pub type NodePath = Vec<String>;

/// The tree, sorted: folders first, then files, each by name without regard to case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    /// None for a file.
    pub children: Option<Vec<Node>>,
}

/// Tree items drawn for a theme and an open file, kept until either changes.
pub(crate) type Items = (Theme, Option<NodePath>, Vec<TreeItem<'static, String>>);

#[derive(Debug)]
pub struct Explorer {
    pub tree: Vec<Node>,
    folders: HashSet<NodePath>,
    /// The file open in the viewer.
    pub open: Option<NodePath>,
    /// Interior mutability: drawing scrolls the tree, and drawing takes the app by reference.
    pub(crate) state: RefCell<TreeState<String>>,
    pub(crate) items: RefCell<Option<Items>>,
}

impl Explorer {
    /// Builds the tree from paths relative to the worktree, such as `src/main.rs`.
    pub fn new<S: AsRef<str>>(files: &[S]) -> Self {
        #[derive(Default)]
        struct Dir(BTreeMap<String, Option<Dir>>);
        let mut root = Dir::default();
        for file in files {
            let parts: Vec<&str> = file.as_ref().split('/').filter(|p| !p.is_empty()).collect();
            let Some((name, folders)) = parts.split_last() else {
                continue;
            };
            let mut dir = &mut root;
            for folder in folders {
                let entry = dir
                    .0
                    .entry((*folder).to_string())
                    .or_insert_with(|| Some(Dir::default()));
                dir = entry.get_or_insert_with(Dir::default);
            }
            dir.0.entry((*name).to_string()).or_insert(None);
        }
        fn sorted(dir: Dir) -> Vec<Node> {
            let mut nodes: Vec<Node> = dir
                .0
                .into_iter()
                .map(|(name, sub)| Node {
                    name,
                    children: sub.map(sorted),
                })
                .collect();
            nodes.sort_by(|a, b| {
                (a.children.is_none(), a.name.to_lowercase(), &a.name).cmp(&(
                    b.children.is_none(),
                    b.name.to_lowercase(),
                    &b.name,
                ))
            });
            nodes
        }
        let tree = sorted(root);
        let mut folders = HashSet::new();
        fn collect(nodes: &[Node], prefix: &mut NodePath, folders: &mut HashSet<NodePath>) {
            for node in nodes {
                if let Some(children) = &node.children {
                    prefix.push(node.name.clone());
                    folders.insert(prefix.clone());
                    collect(children, prefix, folders);
                    prefix.pop();
                }
            }
        }
        collect(&tree, &mut Vec::new(), &mut folders);
        let mut state = TreeState::default();
        if let Some(first) = tree.first() {
            state.select(vec![first.name.clone()]);
        }
        Self {
            tree,
            folders,
            open: None,
            state: RefCell::new(state),
            items: RefCell::new(None),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }

    pub fn is_folder(&self, path: &[String]) -> bool {
        self.folders.contains(path)
    }

    pub fn selected(&self) -> NodePath {
        self.state.borrow().selected().to_vec()
    }

    /// Opens the folders above `path`, selects it, and opens it in the viewer.
    pub fn reveal(&mut self, path: &[String]) {
        let mut state = self.state.borrow_mut();
        for depth in 1..path.len() {
            state.open(path[..depth].to_vec());
        }
        state.select(path.to_vec());
        drop(state);
        self.open = Some(path.to_vec());
    }

    // Moving relies on the order of the rows last drawn, as tui-tree-widget does.

    pub fn down(&self) {
        self.state.borrow_mut().key_down();
    }

    pub fn up(&self) {
        self.state.borrow_mut().key_up();
    }

    pub fn first(&self) {
        self.state.borrow_mut().select_first();
    }

    pub fn last(&self) {
        self.state.borrow_mut().select_last();
    }

    /// A folder opens or closes; a file opens in the viewer, and is returned.
    pub fn activate(&mut self) -> Option<NodePath> {
        let selected = self.selected();
        if selected.is_empty() {
            return None;
        }
        if self.is_folder(&selected) {
            self.state.borrow_mut().toggle_selected();
            None
        } else {
            self.open = Some(selected.clone());
            Some(selected)
        }
    }
}

impl Clone for Explorer {
    /// `TreeState` isn't `Clone`, so the copy reopens the same folders and selection.
    fn clone(&self) -> Self {
        let state = self.state.borrow();
        let mut copy = TreeState::default();
        for opened in state.opened().iter().cloned() {
            copy.open(opened);
        }
        copy.select(state.selected().to_vec());
        Self {
            tree: self.tree.clone(),
            folders: self.folders.clone(),
            open: self.open.clone(),
            state: RefCell::new(copy),
            items: RefCell::new(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(nodes: &[Node]) -> Vec<&str> {
        nodes.iter().map(|n| n.name.as_str()).collect()
    }

    fn path(p: &str) -> NodePath {
        p.split('/').map(String::from).collect()
    }

    #[test]
    fn folders_come_first_and_names_sort_without_regard_to_case() {
        let e = Explorer::new(&[
            "README.md",
            "src/main.rs",
            "Cargo.toml",
            "assets/logo.png",
            ".gitignore",
            "build.rs",
        ]);
        assert_eq!(
            names(&e.tree),
            [
                "assets",
                "src",
                ".gitignore",
                "build.rs",
                "Cargo.toml",
                "README.md"
            ]
        );
    }

    #[test]
    fn nested_files_make_nested_folders() {
        let e = Explorer::new(&["a/b/c.txt", "a/d.txt"]);
        assert!(e.is_folder(&path("a")));
        assert!(e.is_folder(&path("a/b")));
        assert!(!e.is_folder(&path("a/d.txt")));
        assert_eq!(names(e.tree[0].children.as_ref().unwrap()), ["b", "d.txt"]);
    }

    #[test]
    fn the_first_entry_starts_selected_and_nothing_is_open() {
        let e = Explorer::new(&["src/main.rs", "README.md"]);
        assert_eq!(e.selected(), path("src"));
        assert_eq!(e.open, None);
        assert!(Explorer::new::<&str>(&[]).is_empty());
    }

    #[test]
    fn revealing_a_file_opens_its_folders_and_the_file() {
        let mut e = Explorer::new(&["packages/gst/verify.ts", "README.md"]);
        e.reveal(&path("packages/gst/verify.ts"));
        assert_eq!(e.selected(), path("packages/gst/verify.ts"));
        assert_eq!(e.open, Some(path("packages/gst/verify.ts")));
        let opened = e.state.borrow().opened().clone();
        assert!(opened.contains(&path("packages")) && opened.contains(&path("packages/gst")));
    }

    #[test]
    fn a_copy_keeps_open_folders_and_selection() {
        let mut e = Explorer::new(&["packages/gst/verify.ts", "README.md"]);
        e.reveal(&path("packages/gst/verify.ts"));
        let copy = e.clone();
        assert_eq!(copy.selected(), e.selected());
        assert_eq!(copy.open, e.open);
        assert_eq!(copy.state.borrow().opened(), e.state.borrow().opened());
    }
}
