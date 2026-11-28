//! The state machine: one action in, state changes, events and effects out, with no I/O, clock or randomness.

use crate::action::Action;
use crate::effect::Effect;
use crate::event::Event;
use crate::panel::PanelCache;
use crate::scene::SceneCache;
use crate::state::{AppState, Connection};
use meldframe_activity::Feed;
use meldframe_app_parts::answers::action_of;
use meldframe_app_parts::document::{history_of, out_ops};
use meldframe_app_parts::properties::selected_properties;
use meldframe_app_parts::selection::toggled;
use meldframe_app_pointer::Machine;
use meldframe_app_rules::canvas::CanvasLayout;
use meldframe_app_rules::checkpoints::Checkpoints;
use meldframe_app_rules::level::{self, Level};
use meldframe_app_rules::link::Link;
use meldframe_app_rules::png::{Rasters, one_export_event};
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
    pub(crate) cps: Checkpoints,
    pub(crate) feed: Feed,
    pub(crate) panel: PanelCache,
    pub(crate) level: Level,
    pub(crate) prefix: String,
    pub(crate) next_id: u64,
    /// What the Export dialog was built from, and how often (`App::export_builds`).
    pub(crate) export_cache: export_dialog::Cache,
    /// The ids of the variables the last applied import added: the result stays while the file still holds them all.
    pub(crate) import_ids: Vec<String>,
    /// The host's text measurer and the `layout_on_canvas` flag (`crate::canvas`).
    pub(crate) canvas: CanvasLayout,
    /// The painted boxes for the last epoch (`crate::scene`).
    pub(crate) scene_cache: SceneCache,
    /// The pointer state machine (`crate::pointer`).
    pub(crate) pointer: Machine,
    /// The images being made for a PNG export and for agents' captures, and the plans of the export (`crate::raster`).
    pub(crate) raster: Rasters,
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
            cps: Checkpoints::default(),
            feed: Feed::new(actor, seq),
            panel: PanelCache::default(),
            level: Level::Full,
            prefix,
            next_id: 0,
            export_cache: Default::default(),
            import_ids: Vec::new(),
            canvas: CanvasLayout::default(),
            scene_cache: SceneCache::default(),
            raster: Rasters::default(),
            pointer: Machine::default(),
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
    ///
    /// The boxes are kept for the [`App::scene_epoch`] they were worked out at and worked out again only when it moves; this
    /// copies the kept ones, and [`App::with_scene`] reads them without a copy.
    pub fn scene(&self) -> Vec<PaintedBox> {
        self.with_scene(|_, boxes| boxes.to_vec())
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
        // A raw answer or frame is checked here and carried on as the action it stands for (`meldframe_app_parts::answers`).
        if let Action::Wire { wire } = action {
            return action_of(wire).map_or_else(Vec::new, |a| self.dispatch(a));
        }
        // A key is decided here and carried on as the action it makes (`crate::keys`).
        if let Action::Key { input } = action {
            return self.key(input);
        }
        if let Action::Pointer { input } = action {
            return self.pointer(input);
        }
        // A view action that touches none of the derived state, so it skips the refresh of every panel (a wheel sends many).
        if let Action::Camera { op } = action {
            return self.camera_op(op);
        }
        let ui = level::is_view(&action);
        let reload = matches!(action, Action::Reload { .. });
        if self.state.connection == Connection::Halted && !ui && !reload {
            // The one document action that closes something: a commit of the text being edited writes nothing now, and the
            // field that reported it is done, so the edit closes with the text dropped.
            if matches!(action, Action::TextEdit { .. }) {
                let edit = self.state.text_edit.clone();
                self.drop_text_edit();
                self.text_edit_changed(&edit);
            }
            return Vec::new();
        }
        self.level = level::of(&action, ui);
        let raised = self.events.len();
        let manage = self.state.manage.clone();
        let confirm = self.state.confirm.clone();
        let text_edit = self.state.text_edit.clone();
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
            Action::Attribute { actors } => self.attribute(actors),
            Action::SetMode { collection, mode } => self.set_mode(collection, mode, &mut fx),
            Action::SetVariablesQuery { query } => self.set_variables_query(query),
            Action::Manage { op } if op.is_view() => self.manage_view(op),
            Action::TextEdit { op } if op.is_view() => self.text_edit_view(op),
            Action::OpenPicker { id, field } => self.open_picker(id, field),
            Action::ClosePicker {} => self.close_picker(),
            Action::SetPickerQuery { query } => self.set_picker_query(query),
            Action::OpenExport {} => self.open_export(),
            Action::SetExportFormat { format } => self.set_export_format(format),
            Action::CloseExport {} => self.close_export(),
            Action::OpenVariablesExport {} => self.open_variables_export(),
            Action::CloseVariablesExport {} => self.close_variables_export(),
            Action::OpenVariablesImport {} => self.open_variables_import(),
            Action::CloseVariablesImport {} => self.close_variables_import(),
            Action::ChooseVariablesImportSource { name } => self.choose_import_source(&name),
            Action::ClearVariablesImport {} => self.clear_variables_import_field(),
            Action::Overlay { op } => self.overlay_action(op),
            Action::Start { now_ms } => self.start(now_ms, &mut fx),
            Action::Connected { now_ms } => self.connected(now_ms, &mut fx),
            Action::Disconnected {} => self.disconnected(&mut fx),
            Action::Reconnect { now_ms } => self.reconnect(now_ms, &mut fx),
            Action::Tick { now_ms } => self.tick(now_ms, &mut fx),
            Action::Frame { now_ms, frame } => {
                // Any frame brings the time: a capture that is out of time ends before the frame is read.
                self.captures_moved(Some(now_ms), &mut fx);
                self.frame(now_ms, frame, &mut fx)
            }
            Action::Activity { op } if op.is_view() => self.activity_view(op, &mut fx),
            Action::Png { op } => self.png_action(op, &mut fx),
            a if a.is_assets() => self.assets_action(a, &mut fx),
            a if a.is_activity() => self.activity_action(a, &mut fx),
            a if a.is_checkpoint() => self.checkpoint_action(a, &mut fx),
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
                self.refresh_layers();
                self.refresh_properties();
                self.refresh_variables();
                self.refresh_manage();
                self.refresh_picker();
                self.refresh_assets();
                self.refresh_export();
                self.refresh_variables_export();
            }
            Level::Full => self.refresh(&mut fx),
        }
        self.captures_moved(None, &mut fx);
        self.settle_confirm();
        one_export_event(&mut self.events, raised);
        self.settle_overlay();
        self.settle_text_edit();
        self.text_edit_changed(&text_edit);
        if self.state.manage != manage {
            self.events.push(Event::ManageChanged {});
        }
        if self.state.confirm != confirm {
            self.events.push(Event::ConfirmChanged {});
        }
        self.pointer_settle(raised);
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
        self.refresh_layers();
        self.refresh_properties();
        self.refresh_variables();
        self.refresh_manage();
        self.activity_changed();
        self.refresh_picker();
        self.refresh_assets();
        self.refresh_export();
        self.refresh_variables_export();
        self.refresh_variables_import();
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
