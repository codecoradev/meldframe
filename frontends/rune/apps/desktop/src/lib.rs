//! The shell of the native desktop app (E8.2.2). It owns what a front end owns: one [`App`], the network half that carries
//! out the app's effects (`meldframe-net`), and the cached shape of the canvas. It holds no editor rule (U2): a click, a
//! button and the state it shows are the app layer's, sent as actions and read back from `AppState`.
//!
//! The server is reached by [`State::connect`] (or [`State::from_environment`]: the address, file, actor and token come from
//! the environment and the git-ignored `.dev-token`, never from an argument). The network thread loads the file and Rune's
//! `setup` and `poll` hooks hand its answers to the app layer as actions, so the window opens at once on an empty file and
//! the status says what is going on. A state made with [`State::with_nodes`] has no network: the app's effects wait in
//! [`State::outbox`], which the headless tests read.
//!
//! What is not done: no window was opened on macOS, the token is not in the system keychain, and a file that diverged from the
//! server's events halts the app and is not loaded again.

use canvas_ui_rune::SceneShape;
use meldframe_app::{Action, App, Connection, Effect};
use meldframe_net::{Config, ConfigError, Net, Output, Problem, Waker};
use meldframe_protocol::Node;
use meldframe_ui_rune::{BAR_HEIGHT, button, colours, space, status_line};
use rune::{AppSpec, Cx, El, Tx, col, row};

/// The size the window opens at, in points.
pub const WINDOW_SIZE: (f32, f32) = (900.0, 640.0);

/// How far the Move button moves the selection to the right, in the units of the file. A PLACEHOLDER: the app layer has no
/// nudge rule (an arrow key reorders in a flow and moves nothing), so the skeleton asks for a move by a number of its own to have
/// one change a person can send without the pointer. The moves a person makes are the pointer's and are decided in the app
/// layer; this goes when a nudge rule exists there.
pub const MOVE_STEP: f64 = 20.0;

/// The actor of a state with no server: only the tests have one.
const ACTOR: &str = "user:desktop";

/// What is wrong with reaching the server, as the status line words it. The words are fixed: no text of the server is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// No usable address, token or file name in the environment.
    Config(ConfigError),
    /// The network half could not load the file or open the socket.
    Net(Problem),
}

impl Notice {
    pub fn words(&self) -> String {
        match self {
            Notice::Config(e) => e.to_string(),
            Notice::Net(Problem::Refused { status }) => {
                format!("the server did not accept the token ({status})")
            }
            Notice::Net(Problem::Server { status: 0 }) => "the server did not answer".into(),
            Notice::Net(Problem::Server { status }) => format!("the server answered {status}"),
        }
    }
}

/// The state of the window: the app layer, the network half or what it has been asked to do, and the cached shape of the canvas.
#[derive(Debug)]
pub struct State {
    app: App,
    /// The effects of the app when there is no network half.
    outbox: Vec<Effect>,
    /// What to connect to, until `setup` starts the network half (it is given the waker there).
    config: Option<Config>,
    net: Option<Net>,
    notice: Option<Notice>,
    shape: SceneShape,
}

/// A number no earlier run of this client used for the same actor: the app's keys and node ids start with it.
fn session() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u32(std::process::id());
    h.finish() & 0xFFFF_FFFF
}

impl State {
    /// An app on a file and the connection it would have after the socket opens. `connected` stands for the socket having
    /// opened: this slice has none, so only a test sets it.
    pub fn with_nodes(nodes: Vec<Node>, connected: bool) -> State {
        let app = App::new(ACTOR, "desktop", 1, nodes, 0).expect("the nodes load");
        let mut state = State::of(app);
        if connected {
            state.act(Action::Connected { now_ms: 0 });
        }
        state
    }

    fn of(app: App) -> State {
        State {
            app,
            outbox: Vec::new(),
            config: None,
            net: None,
            notice: None,
            shape: SceneShape::default(),
        }
    }

    /// An empty file and no connection, and no way to make one.
    pub fn offline() -> State {
        State::with_nodes(Vec::new(), false)
    }

    /// An empty file that the network half fills once the window has started (`setup`): it loads the file named in `config`
    /// and the app layer takes it as a reload.
    pub fn connect(config: Config) -> State {
        let app = App::new(&config.actor, "desktop", session(), Vec::new(), 0)
            .expect("an empty file loads");
        State {
            config: Some(config),
            ..State::of(app)
        }
    }

    /// The server of the environment (`MELDFRAME_SERVER`, `MELDFRAME_FILE`, `MELDFRAME_ACTOR`, and the token of
    /// `MELDFRAME_TOKEN` or `.dev-token`), or an offline window that says why there is none.
    pub fn from_environment() -> State {
        match Config::from_process() {
            Ok(config) => State::connect(config),
            Err(e) => State {
                notice: Some(Notice::Config(e)),
                ..State::offline()
            },
        }
    }

    /// Send an action to the app layer and hand its effects to the network half, or keep them when there is none. True when
    /// the app raised an event, which is how it says something the view shows has changed (a tick or a heartbeat raises none).
    ///
    /// The events themselves are dropped: this shell draws from the state and would otherwise keep them all. So an
    /// `Event::Refused` (a refused op or request) and an `Event::Diverged` are not shown as such; the connection state shows
    /// the second one as Halted, and nothing offers to load the file again yet.
    pub fn apply(&mut self, action: Action) -> bool {
        let effects = self.app.dispatch(action);
        let changed = !self.app.take_events().is_empty();
        match &mut self.net {
            Some(net) => net.sync(effects, &self.app.state().connection),
            None => self.outbox.extend(effects),
        }
        changed
    }

    /// [`State::apply`] for a caller that does not ask whether the view changed.
    pub fn act(&mut self, action: Action) {
        self.apply(action);
    }

    /// Start the network half. Rune calls this once, with the waker the network threads use to ask for a frame.
    fn start(&mut self, wake: Waker) {
        if let Some(config) = self.config.take() {
            self.net = Some(Net::start(config, wake));
        }
    }

    /// Take what the network half has and give it to the app layer. True when something the view shows changed: an event of
    /// the app, or the notice. The tick every second is not one.
    fn drain(&mut self) -> bool {
        let Some(net) = &self.net else { return false };
        let mut changed = false;
        for output in net.poll() {
            match output {
                Output::Action(action) => changed |= self.apply(action),
                Output::Problem(problem) => {
                    let notice = problem.map(Notice::Net);
                    changed |= notice != self.notice;
                    self.notice = notice;
                }
            }
        }
        changed
    }

    /// What is wrong with reaching the server, if anything.
    pub fn notice(&self) -> Option<&Notice> {
        self.notice.as_ref()
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

/// The status line: the connection state, and what is wrong with reaching the server when something is.
fn status_words(state: &State) -> String {
    let connection = connection_words(&state.app.state().connection);
    match &state.notice {
        Some(notice) => format!("{connection}: {}", notice.words()),
        None => connection,
    }
}

/// The window: a bar with Move, Undo and the connection state over the canvas.
pub fn view(state: &State, _: &mut Cx<State>) -> El {
    let shown = state.app.state();
    // What the app layer takes: a halted connection drops every change, and a move needs something selected.
    let live = shown.connection != Connection::Halted;
    let can_move = live && !shown.selection.is_empty();
    let bar = row()
        .key("bar")
        .h(BAR_HEIGHT)
        .w_full()
        .center_y()
        .gap(space::S2)
        .px(space::S3)
        .fill(colours().card)
        .child(button("move", "Move", can_move, |s: &mut State| {
            s.act(Action::MoveBy {
                dx: MOVE_STEP,
                dy: 0.0,
            });
        }))
        .child(button(
            "undo",
            "Undo",
            live && shown.history.can_undo,
            |s: &mut State| s.act(Action::Undo {}),
        ))
        .child(status_line("status", status_words(state)));
    let canvas = canvas_ui_rune::view(&state.app, &state.shape, |s: &mut State, actions| {
        for action in actions {
            s.act(action);
        }
    });
    col().w_full().h_full().child(bar).child(canvas)
}

/// The app spec of the window, made from how its state is built. Rune's `setup` starts the network half with its waker, and
/// `poll` (run on every frame and on every wake) hands what the network brought to the app layer.
pub fn spec(init: fn() -> State) -> AppSpec<State> {
    AppSpec {
        setup: Some(|state: &mut State, wake: Waker| state.start(wake)),
        poll: Some(|state: &mut State, _: f64, _: &mut Tx| state.drain()),
        ..AppSpec::new("Meldframe", WINDOW_SIZE, init, view)
    }
}
