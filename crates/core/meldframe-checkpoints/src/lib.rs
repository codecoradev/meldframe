//! The rules of checkpoints kept by the server (E5.3, ADR-0023), with no I/O: what a row holds, which kinds there
//! are, what a label may say and how many rows a file keeps.
//!
//! A checkpoint is a row beside a file's log, never a copy of the file and never an op: `seq` is a number in the log
//! (ADR-0016). The store that keeps the rows is `checkpoint-store`; this crate is what the server, the store and the
//! app layer all agree on, so the rules live once (ADR-0008). It depends on `meldframe-activity` (the hidden-text
//! rule, the actor limit) only for now.
//!
//! The label is the person's text: [`Label::parse`] refuses what it cannot show and never cuts. The bounds are shares
//! of a file ([`Share`]): a `manual` row is refused at its share and never rolled out; the automatic kinds roll
//! inside themselves, so a checkpoint the server takes on its own is never refused and one caller cannot
//! push another's rows out.

mod label;

pub use label::{ActorError, Label, LabelError, check_actor};

/// The most rows one file keeps.
pub const MAX_PER_FILE: usize = 100;
/// The share of `manual` rows in a file: refused over it, never rolled.
pub const MAX_MANUAL: usize = 50;
/// The share of `beforeRevert` and `beforeRestore` rows together: the oldest rolls over it.
pub const MAX_BEFORE: usize = 25;
/// The share of `runStart` rows: the oldest rolls over it.
pub const MAX_RUN_START: usize = 25;
/// The most `manual` rows one actor has in a file: refused over it.
pub const MAX_MANUAL_PER_ACTOR: usize = 20;
/// The most `runStart` rows about one actor in a file: that actor's oldest rolls over it.
pub const MAX_RUN_START_PER_ACTOR: usize = 10;
/// The longest label, in characters (Unicode scalar values).
pub const MAX_LABEL_CHARS: usize = 128;

// The shares are the file: an automatic row never needs a room that a share does not give it.
const _: () = assert!(MAX_MANUAL + MAX_BEFORE + MAX_RUN_START == MAX_PER_FILE);

/// What a checkpoint was taken for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The server's own, when an agent's run starts.
    RunStart,
    /// A person's, before a revert of a batch.
    BeforeRevert,
    /// A person's, before a restore.
    BeforeRestore,
    /// A person's, with a label.
    Manual,
}

/// The part of a file's rows a kind counts against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Share {
    Manual,
    Before,
    Run,
}

impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::RunStart,
        Kind::BeforeRevert,
        Kind::BeforeRestore,
        Kind::Manual,
    ];

    /// The name on the wire and in the column.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::RunStart => "runStart",
            Kind::BeforeRevert => "beforeRevert",
            Kind::BeforeRestore => "beforeRestore",
            Kind::Manual => "manual",
        }
    }

    /// The kind a wire name stands for; the match is exact.
    pub fn parse(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == name)
    }

    /// Whether only the server takes it, by its own rule.
    pub fn is_automatic(self) -> bool {
        self == Kind::RunStart
    }

    pub fn share(self) -> Share {
        match self {
            Kind::Manual => Share::Manual,
            Kind::BeforeRevert | Kind::BeforeRestore => Share::Before,
            Kind::RunStart => Share::Run,
        }
    }
}

impl Share {
    /// The most rows of the share a file keeps.
    pub fn limit(self) -> usize {
        match self {
            Share::Manual => MAX_MANUAL,
            Share::Before => MAX_BEFORE,
            Share::Run => MAX_RUN_START,
        }
    }

    /// What a refusal at the share counts, for the message.
    pub fn what(self) -> &'static str {
        match self {
            Share::Manual => "manual checkpoints in a file",
            Share::Before => "checkpoints before a revert or a restore in a file",
            Share::Run => "checkpoints at the start of a run in a file",
        }
    }
}

/// A row of a file, as the store holds it. `n` is per file, from 1 and never reused; `seq` is a number in the file's
/// log: the file as it was after op `seq`. `about` is the actor a checkpoint is about, `by` the actor that asked
/// for it (none for a run start); `at_ms` is the server's clock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    pub n: u32,
    pub seq: u64,
    pub kind: Kind,
    pub label: Option<Label>,
    pub about: Option<String>,
    pub by: Option<String>,
    pub at_ms: u64,
}

/// Why a new row was refused. Every one is `invalid`: nothing is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Invalid {
    /// A caller asked for a run start, which the server alone takes.
    ServerOnly,
    /// A label on a row that is not `manual`.
    LabelNotAllowed(Kind),
    /// An `about` on a row that is not a revert or a run start.
    AboutNotAllowed(Kind),
    /// An actor that is not one (`field` says which).
    Actor(&'static str, ActorError),
}

impl Invalid {
    pub fn code(&self) -> &'static str {
        "invalid"
    }
}

impl std::fmt::Display for Invalid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Invalid::ServerOnly => write!(f, "a run start is taken by the server, not asked for"),
            Invalid::LabelNotAllowed(k) => write!(f, "a {} checkpoint has no label", k.as_str()),
            Invalid::AboutNotAllowed(k) => {
                write!(f, "a {} checkpoint is not about an actor", k.as_str())
            }
            Invalid::Actor(field, why) => write!(f, "{field}: {why}"),
        }
    }
}

impl std::error::Error for Invalid {}

/// A row about to be made: its shape is checked, so a store holds no row of another shape. A `manual` row may carry
/// a label; a revert may name the actor of the batch; a run start is about an agent and by nobody; and `by` is
/// always the principal's actor, never a field of a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCheckpoint {
    kind: Kind,
    seq: u64,
    label: Option<Label>,
    about: Option<String>,
    by: Option<String>,
}

impl NewCheckpoint {
    /// What a person's call makes: `manual`, `beforeRevert` or `beforeRestore`, by `actor`. `seq` is the caller's to
    /// bound by the head of the file; this checks the shape only.
    pub fn by_person(
        kind: Kind,
        seq: u64,
        label: Option<Label>,
        about: Option<String>,
        actor: &str,
    ) -> Result<Self, Invalid> {
        if kind.is_automatic() {
            return Err(Invalid::ServerOnly);
        }
        if label.is_some() && kind != Kind::Manual {
            return Err(Invalid::LabelNotAllowed(kind));
        }
        if about.is_some() && kind != Kind::BeforeRevert {
            return Err(Invalid::AboutNotAllowed(kind));
        }
        if let Some(a) = &about {
            check_actor(a).map_err(|e| Invalid::Actor("about", e))?;
        }
        check_actor(actor).map_err(|e| Invalid::Actor("by", e))?;
        Ok(NewCheckpoint {
            kind,
            seq,
            label,
            about,
            by: Some(actor.to_owned()),
        })
    }

    /// The server's run start for `about`, at `seq` the `seq` just before the first op of the run.
    pub fn run_start(about: &str, seq: u64) -> Result<Self, Invalid> {
        check_actor(about).map_err(|e| Invalid::Actor("about", e))?;
        Ok(NewCheckpoint {
            kind: Kind::RunStart,
            seq,
            label: None,
            about: Some(about.to_owned()),
            by: None,
        })
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn label(&self) -> Option<&Label> {
        self.label.as_ref()
    }

    pub fn about(&self) -> Option<&str> {
        self.about.as_deref()
    }

    pub fn by(&self) -> Option<&str> {
        self.by.as_deref()
    }
}
