//! One actor as a screen shows it, as plain data: the same value on a layer row, a lane, a touch ring and a checkpoint,
//! so one actor wears one colour everywhere (finding T2 of the thin-UI audit, #320). The rule is
//! `meldframe_activity::ActorView`; this file only converts it into serialisable data.

use crate::activity::LaneKind;
use meldframe_activity as act;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which colour an actor wears: a person has one, an agent one of three. A front end maps this to a colour and to a
/// class; it hashes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub enum ActorTone {
    Person,
    Agent1,
    Agent2,
    Agent3,
}

/// An actor: the identifier the app layer keys it by, its kind (read from the actor string, for display only), the name
/// a person reads (safe to draw as it is, rule U9) and its tone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActorView {
    pub actor: String,
    pub kind: LaneKind,
    pub name: String,
    pub tone: ActorTone,
}

impl ActorView {
    /// The view of an actor string that the app layer already made safe as an identifier.
    pub fn of(actor: &str) -> Self {
        act::ActorView::of(actor).into()
    }
}

impl From<act::Tone> for ActorTone {
    fn from(t: act::Tone) -> Self {
        match t {
            act::Tone::Person => Self::Person,
            act::Tone::Agent1 => Self::Agent1,
            act::Tone::Agent2 => Self::Agent2,
            act::Tone::Agent3 => Self::Agent3,
        }
    }
}

impl From<act::ActorView> for ActorView {
    fn from(v: act::ActorView) -> Self {
        ActorView {
            actor: v.actor,
            kind: match v.kind {
                act::LaneKind::Agent => LaneKind::Agent,
                act::LaneKind::Person => LaneKind::Person,
            },
            name: v.name,
            tone: v.tone.into(),
        }
    }
}
