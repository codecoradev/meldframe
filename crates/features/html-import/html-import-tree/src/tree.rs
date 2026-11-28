//! The tree the builder makes: elements with their attributes and children, held flat and named by their place.

use super::report::Report;

/// The most bytes of a tag or attribute name; a longer one is cut.
pub const MAX_NAME_BYTES: usize = 64;

/// An element of a [`Tree`], by its place in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ElementId(pub(crate) usize);

impl ElementId {
    /// The place, from 0 for the root, in the order the start tags came in the text.
    pub fn index(self) -> usize {
        self.0
    }
}

/// An attribute: the name in lower case, the value with its references read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    pub name: String,
    pub value: String,
}

/// What an element holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Child {
    Text(String),
    Element(ElementId),
}

/// An HTML element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// The tag in lower case; empty for the root.
    pub tag: String,
    pub attrs: Vec<Attr>,
    pub children: Vec<Child>,
    /// The element it is in (the root's parent is the root).
    pub parent: ElementId,
    /// The byte of the text where its start tag begins.
    pub at: usize,
}

impl Element {
    /// The value of the attribute `name` (lower case), if the element has it.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.value.as_str())
    }
}

/// The elements of a text, held flat: the root, which stands for the text itself and has the top-level elements
/// and texts as its children, then every element in the order of its start tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tree {
    pub(crate) elements: Vec<Element>,
}

impl Tree {
    pub fn root_id(&self) -> ElementId {
        ElementId(0)
    }

    pub fn root(&self) -> &Element {
        &self.elements[0]
    }

    /// The element `id` of this tree.
    pub fn element(&self, id: ElementId) -> &Element {
        &self.elements[id.0]
    }

    /// How many elements the tree has, the root not counted.
    pub fn len(&self) -> usize {
        self.elements.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Every element, the root not counted, in the order of the start tags.
    pub fn ids(&self) -> impl Iterator<Item = ElementId> {
        (1..self.elements.len()).map(ElementId)
    }

    /// The content of every `<style>`, in the order of the text, as written: the CSS for the next stage.
    pub fn style_texts(&self) -> impl Iterator<Item = &str> {
        self.elements
            .iter()
            .filter(|e| e.tag == "style")
            .flat_map(|e| {
                e.children.iter().filter_map(|c| match c {
                    Child::Text(t) => Some(t.as_str()),
                    Child::Element(_) => None,
                })
            })
    }
}

/// A read: the tree and what the builder had to say about the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub tree: Tree,
    pub report: Report,
}
