//! The kind, the name and the tone an actor wears (finding T2 of the thin-UI audit, #320), derived once. The Layers
//! panel, the lanes, the touch rings and the checkpoints carry this value; a front end maps a [`Tone`] to a colour and
//! derives nothing from the actor string.

use crate::LaneKind;
use meldframe_text::{MAX_ACTOR, label};

const AGENT: &str = "agent:";
const PERSON: &str = "person:";

/// Which colour an actor wears. A person has one; an agent keeps one of three, from a stable hash of its id, so two
/// agents are told apart by colour as well as by name (the name is always on offer: colour is never the only signal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tone {
    Person,
    Agent1,
    Agent2,
    Agent3,
}

/// One actor as a screen shows it. `actor` is the identifier the app layer keys by (already made safe: it is cut and
/// cleaned where it enters), `name` is what the person reads and is safe to draw as it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorView {
    pub actor: String,
    pub kind: LaneKind,
    pub name: String,
    pub tone: Tone,
}

impl LaneKind {
    /// `agent:` is an agent, anything else a person: for display only, it decides nothing about who may review.
    pub fn of(actor: &str) -> Self {
        if actor.starts_with(AGENT) {
            Self::Agent
        } else {
            Self::Person
        }
    }
}

/// The tone of an agent: `h = h * 31 + code point` over every character of the actor (the kind prefix included), modulo
/// 2^32, then `h % 3`. Code points, not UTF-16 units, so every text has one answer whatever its characters.
fn agent_tone(actor: &str) -> Tone {
    let h = actor
        .chars()
        .fold(0u32, |h, c| h.wrapping_mul(31).wrapping_add(u32::from(c)));
    match h % 3 {
        0 => Tone::Agent1,
        1 => Tone::Agent2,
        _ => Tone::Agent3,
    }
}

impl ActorView {
    /// The view of an actor string (`agent:claude`, `person:ana`). The part after `agent:` or `person:` is the name; an
    /// actor of no known kind is a person named by the whole text, and a name with nothing in it is the actor itself.
    pub fn of(actor: &str) -> Self {
        let kind = LaneKind::of(actor);
        let rest = [AGENT, PERSON]
            .iter()
            .find_map(|p| actor.strip_prefix(p))
            .unwrap_or(actor);
        let mut name = label(rest, MAX_ACTOR);
        if name.is_empty() {
            name = label(actor, MAX_ACTOR);
        }
        if name.is_empty() {
            name = actor.to_owned();
        }
        ActorView {
            actor: actor.to_owned(),
            kind,
            name,
            tone: match kind {
                LaneKind::Agent => agent_tone(actor),
                LaneKind::Person => Tone::Person,
            },
        }
    }
}
