//! The page the elements sit in: the `body` (or the `html`), and the title.

use html_import_tree::{Child, ElementId, Tree};

pub(crate) struct Page {
    /// The element whose children are the content: the `body`, else the `html`, else the root of the tree.
    pub container: ElementId,
    /// The text of the first `<title>`, collapsed.
    pub title: Option<String>,
}

fn child_with(tree: &Tree, of: ElementId, tag: &str) -> Option<ElementId> {
    tree.element(of).children.iter().find_map(|c| match c {
        Child::Element(e) if tree.element(*e).tag == tag => Some(*e),
        _ => None,
    })
}

pub(crate) fn find(tree: &Tree) -> Page {
    let html = child_with(tree, tree.root_id(), "html");
    let body = match html {
        Some(h) => child_with(tree, h, "body"),
        None => child_with(tree, tree.root_id(), "body"),
    };
    let title = tree
        .ids()
        .find(|i| tree.element(*i).tag == "title")
        .and_then(|t| {
            tree.element(t).children.iter().find_map(|c| match c {
                Child::Text(s) => Some(super::text::collapse_plain(s)),
                Child::Element(_) => None,
            })
        })
        .filter(|t| !t.is_empty());
    Page {
        container: body.or(html).unwrap_or_else(|| tree.root_id()),
        title,
    }
}
