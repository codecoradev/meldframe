//! Writes through the transport: the [`Submitter`] of the HTTP adapter, one call or an atomic batch.

use super::{SharedServer, failure_of};
use crate::SubmitError;
use meldframe_batch::Stop;
use meldframe_host::{Accepted, Op, Principal};
use meldframe_http::{Failure, Guard, Submitter};

impl Submitter for SharedServer {
    fn submit(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Vec<Accepted>, Failure> {
        self.lock()
            .submit_tool(tool, file, who, ops)
            .map_err(|e| failure_of(&e))
    }

    /// All or nothing: the batch is checked against the room (`meldframe_batch::check`) under the same lock
    /// that then submits it, so no other writer can slip in between. Then it goes through the gate as usual.
    fn submit_atomic(
        &self,
        tool: &str,
        file: &str,
        who: &Principal,
        ops: Vec<Op>,
        guards: Vec<Guard>,
    ) -> Result<Vec<Accepted>, Failure> {
        let mut s = self.lock();
        if let Err(e) = s.precheck(tool, file, who, &ops) {
            s.record_call(tool, file, who, Some(&e));
            return Err(failure_of(&e));
        }
        let (ws, was_open) = (
            who.workspace.as_str(),
            s.with_room(&who.workspace, file, |_| ()),
        );
        let was_open = was_open.is_some();
        if let Err(e) = s.open(ws, file) {
            let e = SubmitError::Rooms(e);
            s.record_call(tool, file, who, Some(&e));
            return Err(failure_of(&e));
        }
        let checked = s
            .with_room(ws, file, |room| {
                meldframe_batch::check(room, &who.actor, &ops, &guards)
            })
            .expect("the room was just opened");
        let done = match checked {
            Err(Stop::Refused { index, refusal }) => {
                s.record_call(
                    tool,
                    file,
                    who,
                    Some(&SubmitError::Refused(refusal.clone())),
                );
                Err(Failure::Batch { index, refusal })
            }
            Err(Stop::Store(e)) => {
                let down = SubmitError::Rooms(meldframe_rooms::RegistryError::Room(
                    meldframe_host::RoomError::Store(e.clone()),
                ));
                s.record_call(tool, file, who, Some(&down));
                Err(Failure::Unavailable(e.to_string()))
            }
            Ok(()) => s
                .submit_tool(tool, file, who, ops)
                .map_err(|e| failure_of(&e)),
        };
        if done.is_err() && !was_open {
            s.close(ws, file);
        }
        done
    }
}
