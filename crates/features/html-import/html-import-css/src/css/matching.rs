//! Does a selector match an element of the tree? Right to left, with backtracking for the descendant combinator,
//! and a budget of steps for the whole import so that no text makes it quadratic.
//!
//! A fragment has no `html` or `body` of its own, but a browser would put its content in them, so rules like
//! `:root{--c:red}` or `body{font-size:14px}` must reach it: the top-level elements of a tree with no `html` (and
//! no `body`) have a virtual `body` and `html` above them ([`Node::Body`], [`Node::Html`]). They match as the
//! elements `body` and `html` with no attributes, and nothing else.

use super::selector::{Combinator, Compound, Selector, Simple};
use html_import_tree::{ElementId, Tree};

/// An element a selector is tested against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Node {
    El(ElementId),
    /// The `body` a fragment has no element for.
    Body,
    /// The `html` a fragment has no element for.
    Html,
}

pub(super) struct Matcher<'t> {
    tree: &'t Tree,
    virtual_body: bool,
    virtual_html: bool,
    /// Steps taken, and the most that may be taken.
    pub steps: usize,
    budget: usize,
}

impl<'t> Matcher<'t> {
    pub fn new(tree: &'t Tree, budget: usize) -> Self {
        let has = |tag: &str| tree.ids().any(|i| tree.element(i).tag == tag);
        let has_html = has("html");
        let has_body = has("body");
        Matcher {
            tree,
            virtual_html: !has_html,
            virtual_body: !has_html && !has_body,
            steps: 0,
            budget,
        }
    }

    pub fn virtual_body(&self) -> bool {
        self.virtual_body
    }

    pub fn virtual_html(&self) -> bool {
        self.virtual_html
    }

    pub fn exhausted(&self) -> bool {
        self.steps > self.budget
    }

    /// The element `n` is in: for a top-level element, the virtual `body` and `html` when it has them.
    pub fn parent(&self, n: Node) -> Option<Node> {
        match n {
            Node::Html => None,
            Node::Body => self.virtual_html.then_some(Node::Html),
            Node::El(e) => {
                let p = self.tree.element(e).parent;
                if p != self.tree.root_id() {
                    Some(Node::El(p))
                } else if self.virtual_body {
                    Some(Node::Body)
                } else {
                    self.virtual_html.then_some(Node::Html)
                }
            }
        }
    }

    fn simple(&self, s: &Simple, n: Node) -> bool {
        let Node::El(e) = n else {
            return match s {
                Simple::Tag(t) => t == if n == Node::Body { "body" } else { "html" },
                Simple::Root => n == Node::Html,
                _ => false,
            };
        };
        let el = self.tree.element(e);
        match s {
            Simple::Tag(t) => el.tag == *t,
            Simple::Class(c) => el
                .attr("class")
                .is_some_and(|v| v.split_ascii_whitespace().any(|w| w == c)),
            Simple::Id(i) => el.attr("id") == Some(i.as_str()),
            Simple::Attr(name, None) => el.attr(name).is_some(),
            Simple::Attr(name, Some(v)) => el.attr(name) == Some(v.as_str()),
            Simple::Root => el.tag == "html" && el.parent == self.tree.root_id(),
        }
    }

    fn compound(&mut self, c: &Compound, n: Node) -> bool {
        self.steps += 1 + c.simples.len();
        !self.exhausted() && c.simples.iter().all(|s| self.simple(s, n))
    }

    fn from(&mut self, sel: &Selector, i: usize, n: Node) -> bool {
        self.compound(&sel.compounds[i], n) && self.left(sel, i, n)
    }

    /// `n` matches compound `i`: does what is to the left of it match? A descendant combinator tries each ancestor,
    /// nearest first; when only descendant combinators lie to the left of the one tried, the nearest ancestor that
    /// matches is the best one (any farther one has fewer ancestors above it), so a failure there is final. That
    /// keeps a chain of descendants linear; a child combinator to the left can still make it try them all, and the
    /// budget bounds that.
    fn left(&mut self, sel: &Selector, i: usize, n: Node) -> bool {
        if i == 0 {
            return true;
        }
        match sel.combinators[i - 1] {
            Combinator::Child => self.parent(n).is_some_and(|p| self.from(sel, i - 1, p)),
            Combinator::Descendant => {
                let final_failure = sel.combinators[..i - 1]
                    .iter()
                    .all(|c| *c == Combinator::Descendant);
                let mut up = self.parent(n);
                while let Some(p) = up {
                    if self.compound(&sel.compounds[i - 1], p) {
                        if self.left(sel, i - 1, p) {
                            return true;
                        }
                        if final_failure {
                            return false;
                        }
                    }
                    if self.exhausted() {
                        return false;
                    }
                    up = self.parent(p);
                }
                false
            }
        }
    }

    /// Does `sel` pick `n`? The pseudo-element of the selector is not looked at: the caller says what it styles.
    pub fn matches(&mut self, sel: &Selector, n: Node) -> bool {
        self.from(sel, sel.compounds.len() - 1, n)
    }
}
