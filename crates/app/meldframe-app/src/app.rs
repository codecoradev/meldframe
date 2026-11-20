//! The state machine: one action in, state changes, events and effects out. No I/O, clock or randomness.

use crate::action::{Action, Frame};
use crate::effect::{Effect, OutOp};
use crate::event::Event;
use crate::link::Link;
use crate::panel::PanelCache;
use crate::retry::Retries;
use crate::review::Review;
use crate::state::{AppState, Connection, History, Panel};
use crate::touched::{render_set, touched};
use meldframe_activity::Feed;
use meldframe_protocol::{Command, Doc, Node, Variables};
use meldframe_render::{PaintedBox, resolve_with};
use meldframe_sync::{EventOutcome, LoadError, Outgoing, Replica, ReplicaError};
use std::collections::HashSet;

/// How much of the derived state one dispatch has to bring up to date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// Nothing derived can have changed (a heartbeat, a timer, a repeated frame).
    Nothing,
    /// Only the selection or a panel changed.
    Ui,
    /// The document or the ops may have changed.
    Full,
}

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

    /// The boxes the canvas paints, back to front: the one `resolve_with` step of the render model (E3.1) over
    /// [`App::nodes`], with the variables resolved in the modes of `AppState::modes` (E3.3). The canvas reads
    /// this and computes nothing of its own.
    pub fn scene(&self) -> Vec<PaintedBox> {
        resolve_with(self.nodes(), self.vars(), &self.choice())
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
        let ui = matches!(
            action,
            Action::Select { .. }
                | Action::ToggleSelect { .. }
                | Action::ClearSelection {}
                | Action::SetTool { .. }
                | Action::SetPanel { .. }
                | Action::TogglePanel { .. }
                | Action::ToggleCollapsed { .. }
                | Action::SetMode { .. }
                | Action::SetVariablesQuery { .. }
        );
        let reload = matches!(action, Action::Reload { .. });
        if self.state.connection == Connection::Halted && !ui && !reload {
            return Vec::new();
        }
        self.level = match &action {
            _ if ui => Level::Ui,
            a if a.only_activity() => Level::Nothing,
            Action::Start { .. }
            | Action::Disconnected {}
            | Action::Reconnect { .. }
            | Action::Tick { .. }
            | Action::Frame {
                frame: Frame::Heartbeat {} | Frame::Batch { .. } | Frame::Touch { .. },
                ..
            } => Level::Nothing,
            _ => Level::Full,
        };
        let mut fx = Vec::new();
        match action {
            Action::Select { ids } => self.select(ids),
            Action::ToggleSelect { id } => {
                let mut ids = self.state.selection.clone();
                match ids.iter().position(|current| *current == id) {
                    Some(at) => {
                        ids.remove(at);
                    }
                    None => ids.push(id),
                }
                self.select(ids);
            }
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
            } => self.reload(
                now_ms,
                Doc {
                    nodes,
                    vars: variables,
                },
                seq,
                &applied,
                &mut fx,
            ),
            doc => self.document(doc, &mut fx),
        }
        match self.level {
            Level::Nothing => self.activity_changed(),
            Level::Ui => {
                self.refresh_properties();
                self.refresh_variables();
            }
            Level::Full => self.refresh(&mut fx),
        }
        fx
    }

    pub(crate) fn set_tool(&mut self, tool: crate::state::Tool) {
        if self.state.tool != tool {
            self.state.tool = tool;
            self.events.push(Event::ToolChanged { tool });
        }
    }

    pub(crate) fn select(&mut self, ids: Vec<String>) {
        let mut keep: Vec<String> = Vec::with_capacity(ids.len());
        for id in ids {
            if !keep.contains(&id) && self.replica.nodes().iter().any(|n| n.id == id) {
                keep.push(id);
            }
        }
        if self.state.selection != keep {
            self.state.selection = keep.clone();
            self.events.push(Event::SelectionChanged { ids: keep });
        }
    }

    fn is_open(&self, panel: Panel) -> bool {
        match panel {
            Panel::Layers => self.state.panels.layers,
            Panel::Properties => self.state.panels.properties,
            Panel::Variables => self.state.panels.variables,
        }
    }

    fn set_panel(&mut self, panel: Panel, open: bool) {
        if self.is_open(panel) != open {
            match panel {
                Panel::Layers => self.state.panels.layers = open,
                Panel::Properties => self.state.panels.properties = open,
                Panel::Variables => self.state.panels.variables = open,
            }
            self.events.push(Event::PanelChanged { panel, open });
        }
    }

    fn toggle_collapsed(&mut self, id: String) {
        let known = self.replica.nodes().iter().any(|n| n.id == id);
        if !known && !self.state.panels.collapsed.contains(&id) {
            return;
        }
        let list = &mut self.state.panels.collapsed;
        let collapsed = match list.iter().position(|c| *c == id) {
            Some(at) => {
                list.remove(at);
                false
            }
            None => {
                list.push(id.clone());
                true
            }
        };
        self.events.push(Event::CollapsedChanged { id, collapsed });
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

    fn frame(&mut self, now_ms: u64, frame: Frame, fx: &mut Vec<Effect>) {
        self.heard(now_ms);
        match frame {
            Frame::Heartbeat {} => {}
            Frame::Resync { after_seq, .. } => {
                // A marker ahead of the replica (a stale one) must not skip ops it has not got.
                self.resubscribe(after_seq.min(self.replica.seq()), fx);
                self.fetched(|f, _| f.connected(now_ms), fx);
            }
            Frame::Batch { .. } | Frame::Touch { .. } => self.activity_frame(now_ms, frame, fx),
            Frame::Rejected {
                key,
                code,
                retryable,
            } => {
                if self.refused(now_ms, key, code, retryable, fx) {
                    self.changed(self.replica.actor().to_string(), true, Vec::new(), fx);
                }
            }
            Frame::Acked { key, seq } => {
                if let Some(dropped) = self.replica.on_ack(&key, seq) {
                    self.dropped(dropped);
                    self.changed(self.replica.actor().to_string(), false, Vec::new(), fx);
                }
            }
            Frame::Event {
                seq,
                actor,
                key,
                command,
            } => self.server_event(now_ms, seq, &actor, key.as_deref(), &command, fx),
        }
    }

    fn server_event(
        &mut self,
        now_ms: u64,
        seq: u64,
        actor: &str,
        key: Option<&str>,
        command: &Command,
        fx: &mut Vec<Effect>,
    ) {
        match self.replica.on_event_keyed(seq, actor, key, command) {
            Ok(EventOutcome::Confirmed) => {
                self.gap_closed();
                self.record_op(seq, actor, key, command, now_ms);
                self.stamp(actor, command);
            }
            Ok(EventOutcome::Rebased { dropped }) => {
                self.gap_closed();
                self.record_op(seq, actor, key, command, now_ms);
                self.stamp(actor, command);
                let nodes = if dropped > 0 {
                    Vec::new()
                } else {
                    render_set(std::slice::from_ref(command))
                };
                self.changed(actor.to_string(), false, nodes, fx);
            }
            Ok(EventOutcome::Duplicate) => self.level = Level::Nothing,
            Err(ReplicaError::Gap { .. }) => {
                self.level = Level::Nothing;
                self.on_gap(fx);
            }
            Err(ReplicaError::Diverged(_)) => {
                self.set_connection(Connection::Halted);
                self.events.push(Event::Diverged {});
            }
        }
    }

    pub(crate) fn dropped(&mut self, count: usize) {
        if count > 0 {
            self.events.push(Event::OpsDropped { count });
        }
    }

    /// Remember who changed the nodes a command touches.
    fn stamp(&mut self, actor: &str, command: &Command) {
        if !matches!(command, Command::RemoveNode { .. }) {
            for id in touched(command) {
                self.state.last_actor.insert(id, actor.to_string());
            }
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
        self.events
            .push(Event::DocumentChanged { actor, optimistic });
        fx.push(Effect::Render { nodes });
    }

    /// The properties panel follows the selection.
    fn refresh_properties(&mut self) {
        self.state.properties = match self.state.selection.as_slice() {
            [id] => self
                .replica
                .nodes()
                .iter()
                .find(|n| n.id == *id)
                .map(|n| self.properties(n)),
            _ => None,
        };
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
        let history = History {
            can_undo: self.replica.can_undo(),
            can_redo: self.replica.can_redo(),
            undo_steps: self.replica.undo_depth(),
            redo_steps: self.replica.redo_depth(),
        };
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

pub(crate) fn out_ops(ops: Vec<Outgoing>) -> Vec<OutOp> {
    ops.into_iter()
        .map(|o| OutOp {
            key: o.key,
            command: o.command,
            deps: o.deps,
        })
        .collect()
}
