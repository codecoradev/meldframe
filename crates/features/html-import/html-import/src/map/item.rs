//! The mapping's working tree: a node with its children, and what the mapping has to say about it. It is flattened
//! into `addNode` Commands in the order of the text (a parent before its children, siblings as written).

use meldframe_protocol::{
    Align, Bind, Border, Command, Justify, LayoutMode, Node, NodeKind, ShaderPreset, SizeMode,
    TextAlign,
};

/// A property the mapping carried over only in part: what it was and what was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approx {
    pub property: String,
    pub from: String,
    pub to: String,
}

/// A property of the source that the mapping did not read, with its value cut to 80 characters and the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub property: String,
    pub value: String,
    pub code: &'static str,
}

/// What the mapping says about one node it wrote, in the order of the Commands: `commands[i]` adds `nodes[i]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedNode {
    pub id: String,
    pub name: String,
    pub kind: NodeKind,
    /// The element it came from: `tag.first-class#id`, cut to 200 characters (an anonymous run of text has the tag
    /// of the element it is in).
    pub source: String,
    /// The byte of the text where the element starts, the `at` of its report entries.
    pub at: usize,
    pub approximated: Vec<Approx>,
    pub unsupported: Vec<Unsupported>,
}

/// An image the source names and the mapping did not fetch: its `src`, cut to 200 characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub node: String,
    pub src: String,
}

/// What a node collects while it is built.
#[derive(Debug, Default)]
pub(crate) struct Notes {
    pub approx: Vec<Approx>,
    pub unsupported: Vec<Unsupported>,
}

/// A node of the plan under construction.
#[derive(Debug)]
pub(crate) struct Item {
    pub node: Node,
    pub kids: Vec<Item>,
    pub source: String,
    pub at: usize,
    pub notes: Notes,
    pub src: Option<String>,
    /// Its name is the element's own `data-name`: nothing but the call's `name` may replace it.
    pub named: bool,
}

/// A node with the values section 3.1 of `docs/import-html.md` says the importer writes: the model's own defaults are
/// not CSS's, so every one of them is set here.
pub(crate) fn blank(kind: NodeKind) -> Node {
    Node {
        id: String::new(),
        name: String::new(),
        kind,
        parent_id: None,
        pos: String::new(),
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
        sw: SizeMode::Fixed,
        sh: SizeMode::Fixed,
        fill: Bind::Lit("none".into()),
        fill2: "#22d3ee".into(),
        radius: Bind::Lit(0.0),
        layout: LayoutMode::None,
        gap: 0.0,
        padding: 0.0,
        pad_t: None,
        pad_r: None,
        pad_b: None,
        pad_l: None,
        align: Align::Start,
        justify: Justify::Start,
        clip: false,
        text: String::new(),
        font_size: 16.0,
        shader: ShaderPreset::Aurora,
        font_weight: 400,
        text_align: TextAlign::Left,
        border: Border::default(),
        shadows: Vec::new(),
        mar_t: 0.0,
        mar_r: 0.0,
        mar_b: 0.0,
        mar_l: 0.0,
        absolute: false,
        opacity: Bind::Lit(1.0),
        gradient: None,
        image: None,
    }
}

/// The plan's output, flattened: the Commands, the notes of each and the assets.
pub(crate) struct Flat {
    pub commands: Vec<Command>,
    pub nodes: Vec<PlannedNode>,
    pub assets: Vec<Asset>,
}

/// Number the items in the order of the text and turn each into an `addNode`. The id of the `i`th is `{prefix}-{i}`.
pub(crate) fn flatten(root: Item, prefix: &str) -> Flat {
    let mut out = Flat {
        commands: Vec::new(),
        nodes: Vec::new(),
        assets: Vec::new(),
    };
    put(root, None, prefix, &mut out);
    out
}

fn put(item: Item, parent: Option<&str>, prefix: &str, out: &mut Flat) {
    let id = format!("{prefix}-{}", out.commands.len());
    let Item {
        mut node,
        kids,
        source,
        at,
        notes,
        src,
        named: _,
    } = item;
    node.id = id.clone();
    node.parent_id = parent.map(str::to_owned);
    out.nodes.push(PlannedNode {
        id: id.clone(),
        name: node.name.clone(),
        kind: node.kind,
        source,
        at,
        approximated: notes.approx,
        unsupported: notes.unsupported,
    });
    if let Some(src) = src {
        out.assets.push(Asset {
            node: id.clone(),
            src,
        });
    }
    out.commands.push(Command::AddNode { node, index: None });
    for k in kids {
        put(k, Some(&id), prefix, out);
    }
}
