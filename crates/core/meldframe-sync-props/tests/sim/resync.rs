//! A replica that loses its connection loads the file again (`Replica::resync`).

use super::Net;

impl Net {
    /// Replica `i` lost its connection and the events in flight with it (a refusal too). It loads the
    /// file again, as the server holds it now, and replays the ops the file does not hold yet.
    pub fn resync(&mut self, i: usize) {
        let applied = self.server.applied_keys(&self.names[i]);
        self.down[i].clear();
        let (nodes, seq) = (self.server.nodes.clone(), self.server.seq);
        if let Err(e) = self.reps[i].resync(nodes, seq, &applied) {
            self.fail(i, &format!("resync: {e:?}"));
        }
        self.trace[i].push(format!("RESYNC seq{seq}"));
        self.mirrors[i].clear();
        self.stats.resyncs += 1;
        if let Err(e) = self.reps[i].check_consistency() {
            self.fail(i, &e);
        }
        self.flush(i);
    }
}
