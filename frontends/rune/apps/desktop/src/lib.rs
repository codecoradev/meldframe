//! The shell of the native desktop app (E8.2.2, first slice). It owns what a front end owns: one [`App`], the
//! transport's effects waiting to be carried out, and the cached shape of the canvas. It holds no editor rule (U2): a click,
//! a button and the state it shows are the app layer's, sent as actions and read back from `AppState`.
//!
//! What this slice does not do: open a connection (the socket and the token are not built), so the effects of the app
//! (`SendOps`, the subscription) are kept in [`State::outbox`] and not sent; and no window was opened on macOS.

use canvas_ui_rune::SceneShape;
use meldframe_app::{Action, App, Connection, Effect};
use meldframe_protocol::Node;
use meldframe_ui_rune::{BAR_HEIGHT, button, colours, space, status_line};
use rune::{AppSpec, Cx, El, col, row};

/// The size the window opens at, in points.
pub const WINDOW_SIZE: (f32, f32) = (900.0, 640.0);

/// How far the Move button moves the selection to the right, in the units of the file. The button stands for the
/// first action a person can send without the pointer; the moves a person makes are the pointer's, and decided in the app layer.
pub const MOVE_STEP: f64 = 20.0;

/// The actor the shell edits as until sign-in gives it one.
const ACTOR: &str = "user:desktop";

/// The state of the window: the app layer, what it has asked the transport to do, and the cached shape of the canvas.
#[derive(Debug)]
pub struct State {
    app: App,
    outbox: Vec<Effect>,
    shape: SceneShape,
}

impl State {
    /// An app on a file and the connection it would have after the socket opens. `connected` stands for the socket having
    /// opened: this slice has none, so only a test sets it.
    pub fn with_nodes(nodes: Vec<Node>, connected: bool) -> State {
        let app = App::new(ACTOR, "desktop", 1, nodes, 0).expect("the nodes load");
        let mut state = State {
            app,
            outbox: Vec::new(),
            shape: SceneShape::default(),
        };
        if connected {
            state.act(Action::Connected { now_ms: 0 });
        }
        state
    }

    /// An empty file and no connection: what the window starts on while the socket is not built.
    pub fn offline() -> State {
        State::with_nodes(Vec::new(), false)
    }

    /// Send an action to the app layer and keep the effects it returns for the transport.
    pub fn act(&mut self, action: Action) {
        let effects = self.app.dispatch(action);
        self.outbox.extend(effects);
    }

    /// The app layer, for reading.
    pub fn app(&self) -> &App {
        &self.app
    }

    /// The effects the app asked the transport for, not carried out yet.
    pub fn outbox(&self) -> &[Effect] {
        &self.outbox
    }

    /// The cached shape of the canvas, for its counters.
    pub fn shape(&self) -> &SceneShape {
        &self.shape
    }
}

/// The words of a connection state, for the status line.
fn connection_words(connection: &Connection) -> String {
    match connection {
        Connection::Connecting => "Connecting".into(),
        Connection::Live => "Connected".into(),
        Connection::Reconnecting { attempt } => format!("Reconnecting, attempt {attempt}"),
        Connection::Offline => "Offline".into(),
        Connection::Halted => "Stopped, reload the file".into(),
    }
}

/// The window: a bar with Move, Undo and the connection state over the canvas.
pub fn view(state: &State, _: &mut Cx<State>) -> El {
    let shown = state.app.state();
    let bar = row()
        .key("bar")
        .h(BAR_HEIGHT)
        .w_full()
        .center_y()
        .gap(space::S2)
        .px(space::S3)
        .fill(colours().card)
        .child(button(
            "move",
            "Move",
            !shown.selection.is_empty(),
            |s: &mut State| {
                s.act(Action::MoveBy {
                    dx: MOVE_STEP,
                    dy: 0.0,
                });
            },
        ))
        .child(button(
            "undo",
            "Undo",
            shown.history.can_undo,
            |s: &mut State| s.act(Action::Undo {}),
        ))
        .child(status_line("status", connection_words(&shown.connection)));
    let canvas = canvas_ui_rune::view(&state.app, &state.shape, |s: &mut State, actions| {
        for action in actions {
            s.act(action);
        }
    });
    col().w_full().h_full().child(bar).child(canvas)
}

/// The app spec of the window, made from how its state is built.
pub fn spec(init: fn() -> State) -> AppSpec<State> {
    AppSpec::new("Meldframe", WINDOW_SIZE, init, view)
}
