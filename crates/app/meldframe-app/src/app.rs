//! The state machine: one action in, state changes, events and effects out. No I/O, clock or randomness.

use crate::action::Action;
use crate::canvas::CanvasLayout;
use crate::effect::Effect;
use crate::event::Event;
use crate::panel::PanelCache;
use crate::state::{AppState, Connection};
use meldframe_activity::Feed;
use meldframe_app_parts::document::{history_of, out_ops};
use meldframe_app_parts::properties::selected_properties;
use meldframe_app_parts::selection::toggled;
use meldframe_app_rules::level::{self, Level};
use meldframe_app_rules::link::Link;
use meldframe_app_rules::retry::Retries;
use meldframe_app_rules::review::Review;
use meldframe_protocol::{Doc, Node, Variables};
use meldframe_render::PaintedBox;
use meldframe_sync::{LoadError, Replica};
use std::collections::HashSet;

/// Owns the [`AppState`] and the document replica, and applies actions to them.
#[derive(Debug)]
pub struct App {
    pub(crate) state: AppState,
    pub(crate) events: Vec<Event>,
    pub(crate) replica: Replica,
    pub(crate) link: Link,
    pub(crate) retries: Retries,
    pub(crate) review: Review,
    pub(crate) feed: Feed,
    pub(crate) panel: PanelCache,
    pub(crate) level: Level,
    pub(crate) prefix: String,
    pub(crate) next_id: u64,
    /// What the Export dialog was built from, and how often (`App::export_builds`).
    pub(crate) export_cache: export_dialog::Cache,
    /// The host's text measurer and the `layout_on_canvas` flag (`crate::canvas`).
    pub(crate) canvas: CanvasLayout,
}

impl App {
    /// An app for the file `nodes` as of server sequence number `seq` (from a snapshot). `actor` is the
    /// name the server stamps on the person's events.
    ///
    /// Idempotency keys and node ids start with `key_prefix` and `session`. The server remembers every key
    /// of an actor, so the pair must never repeat for one actor: the shell picks a fresh random `session`
    /// (a nonce) every time it makes an `App`, for example on every page load, and keeps `key_prefix`
    /// stable per client kind. An `App` reusing an old pair would have its edits refused (a different
    /// command under an old key) or silently taken for duplicates.
    pub fn new(
        actor: &str,
        key_prefix: &str,
        session: u64,
        nodes: Vec<Node>,
        seq: u64,
    ) -> Result<Self, LoadError> {
        Self::with_doc(actor, key_prefix, session, Doc::of_nodes(nodes), seq)
    }

    /// Like [`App::new`] for a file with variables (`doc` holds its nodes and its collections and variables).
    pub fn with_doc(
        actor: &str,
        key_prefix: &str,
        session: u64,
        doc: Doc,
        seq: u64,
    ) -> Result<Self, LoadError> {
        let prefix = format!("{key_prefix}-{session:x}");
        let review_nodes = doc.nodes.clone();
        let replica = Replica::from_doc(actor, &prefix, doc, seq)?;
        let mut app = App {
            state: AppState::default(),
            events: Vec::new(),
            replica,
            link: Link::default(),
            retries: Retries::default(),
            review: Review::new(review_nodes, seq),
            feed: Feed::new(actor, seq),
            panel: PanelCache::default(),
            level: Level::Full,
            prefix,
            next_id: 0,
            export_cache: Default::default(),
            canvas: CanvasLayout::default(),
        };
        app.refresh(&mut Vec::new());
        app.events.clear();
        Ok(app)
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// Refuse a document action (`Event::Refused`, code `outbox_full`) while this many ops wait for the
    /// server; the replica's own default is 10,000.
    pub fn set_max_pending(&mut self, max: usize) {
        self.replica.set_max_pending(max);
    }

    /// The document as the person sees it: the confirmed file plus their pending ops.
    pub fn nodes(&self) -> &[Node] {
        self.replica.nodes()
    }

    /// The boxes the canvas paints, back to front: the one resolve step of the render model (E3.1) over
    /// [`App::nodes`], with the variables resolved in the modes of `AppState::modes` (E3.3). The canvas reads
    /// this and computes nothing of its own. The step is `resolve_with` (the stored boxes) unless the host turned
    /// `layout_on_canvas` on: then it is `resolve_laid_out` with the host's text measurer (`crate::canvas`).
    pub fn scene(&self) -> Vec<PaintedBox> {
        self.canvas.scene(self.nodes(), self.vars(), &self.choice())
    }

    /// The collections and variables of the document as the person sees them.
    pub fn vars(&self) -> &Variables {
        self.replica.vars()
    }

    /// The confirmed document.
    pub fn base(&self) -> &[Node] {
        self.replica.base()
    }

    /// The events raised since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Applies one action and returns the work the shell has to do. The same actions in the same order
    /// always give the same state, events and effects.
    pub fn dispatch(&mut self, action: Action) -> Vec<Effect> {
        let ui = level::is_view(&action);
        let reload = matches!(action, Action::Reload { .. });
        if self.state.connection == Connection::Halted && !ui && !reload {
            return Vec::new();
        }
        self.level = level::of(&action, ui);
        let mut fx = Vec::new();
        match action {
            Action::Select { ids } => self.select(ids),
            Action::ToggleSelect { id } => self.select(toggled(&self.state.selection, id)),
            Action::ClearSelection {} => self.select(Vec::new()),
            Action::SetTool { tool } => self.set_tool(tool),
            Action::SetPanel { panel, open } => self.set_panel(panel, open),
            Action::TogglePanel { panel } => {
                let open = !self.is_open(panel);
                self.set_panel(panel, open);
            }
            Action::ToggleCollapsed { id } => self.toggle_collapsed(id),
            Action::SetMode { collection, mode } => self.set_mode(collection, mode, &mut fx),
            Action::SetVariablesQuery { query } => self.set_variables_query(query),
            Action::OpenPicker { id, field } => self.open_picker(id, field),
            Action::ClosePicker {} => self.state.picker = None,
            Action::SetPickerQuery { query } => self.set_picker_query(query),
            Action::OpenExport {} => self.open_export(),
            Action::SetExportFormat { format } => self.set_export_format(format),
            Action::CloseExport {} => self.close_export(),
            Action::Start { now_ms } => self.start(now_ms, &mut fx),
            Action::Connected { now_ms } => self.connected(now_ms, &mut fx),
            Action::Disconnected {} => self.disconnected(&mut fx),
            Action::Reconnect { now_ms } => self.reconnect(now_ms, &mut fx),
            Action::Tick { now_ms } => self.tick(now_ms, &mut fx),
            Action::Frame { now_ms, frame } => self.frame(now_ms, frame, &mut fx),
            a if a.is_activity() => self.activity_action(a, &mut fx),
            Action::Reload {
                now_ms,
                nodes,
                variables,
                seq,
                applied,
            } => {
                let doc = Doc {
                    nodes,
                    vars: variables,
                };
                self.reload(now_ms, doc, seq, &applied, &mut fx);
            }
            doc => self.document(doc, &mut fx),
        }
        match self.level {
            Level::Nothing => self.activity_changed(),
            Level::Ui => {
                self.refresh_properties();
                self.refresh_variables();
                self.refresh_picker();
                self.refresh_export();
            }
            Level::Full => self.refresh(&mut fx),
        }
        fx
    }

    /// A node id nobody uses yet.
    pub(crate) fn fresh_id(&mut self) -> String {
        loop {
            self.next_id += 1;
            let id = format!("{}-n{}", self.prefix, self.next_id);
            if !self.replica.nodes().iter().any(|n| n.id == id) {
                return id;
            }
        }
    }

    pub(crate) fn set_connection(&mut self, connection: Connection) {
        if self.state.connection != connection {
            self.state.connection = connection;
            self.events.push(Event::ConnectionChanged { connection });
        }
    }

    /// The document changed: tell the UI and ask for a redraw of `nodes` (empty: everything).
    pub(crate) fn changed(
        &mut self,
        actor: String,
        optimistic: bool,
        nodes: Vec<String>,
        fx: &mut Vec<Effect>,
    ) {
        self.export_cache.changed();
        self.events
            .push(Event::DocumentChanged { actor, optimistic });
        fx.push(Effect::Render { nodes });
    }

    /// The properties panel follows the selection.
    pub(crate) fn refresh_properties(&mut self) {
        let drawn = self.drawn_selection();
        self.state.properties = selected_properties(
            self.vars(),
            &self.choice(),
            self.replica.nodes(),
            &self.state.selection,
            drawn,
        );
    }

    /// Bring the derived parts of the state in line with the replica, once per dispatch, and send what the
    /// replica queued.
    fn refresh(&mut self, fx: &mut Vec<Effect>) {
        let out = self.replica.take_outbox();
        if self.state.connection == Connection::Live && !out.is_empty() {
            fx.push(Effect::SendOps { ops: out_ops(out) });
        }
        let ids: HashSet<&str> = self.replica.nodes().iter().map(|n| n.id.as_str()).collect();
        self.state
            .last_actor
            .retain(|id, _| ids.contains(id.as_str()));
        self.state
            .panels
            .collapsed
            .retain(|id| ids.contains(id.as_str()));
        let rest: Vec<String> = self
            .state
            .selection
            .iter()
            .filter(|id| ids.contains(id.as_str()))
            .cloned()
            .collect();
        if rest != self.state.selection {
            self.state.selection = rest.clone();
            self.events.push(Event::SelectionChanged { ids: rest });
        }
        self.prune_modes();
        self.refresh_properties();
        self.refresh_variables();
        self.activity_changed();
        self.refresh_picker();
        self.refresh_export();
        let history = history_of(&self.replica);
        if self.state.history != history {
            self.state.history = history;
            self.events.push(Event::HistoryChanged {
                can_undo: history.can_undo,
                can_redo: history.can_redo,
            });
        }
        if self.replica.pending_len() > 0 || !self.state.pending.is_empty() {
            let pending: Vec<String> = self
                .replica
                .unconfirmed()
                .into_iter()
                .map(|o| o.key)
                .collect();
            if self.state.pending != pending {
                self.events.push(Event::PendingChanged {
                    count: pending.len(),
                });
                self.state.pending = pending;
                self.prune_retries();
            }
        }
        self.state.confirmed_seq = self.replica.seq();
    }
}
