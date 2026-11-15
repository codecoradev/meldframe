//! Server composition root. It assembles the registry from the compiled-in feature modules and builds the
//! in-memory host. HTTP, MCP, REST and WebSocket transports (E1.3) arrive later; the binary only assembles
//! and holds no business logic. [`PgStore`] is the Postgres adapter of the store seam; wiring it into the
//! assembled server comes with the room per file (issue 133).

mod pg;

pub use pg::PgStore;

use meldframe_host::{Gate, MemoryStore, Op, Principal, Refusal, Report, Room};
use meldframe_module::{Module, Policy, Registry, RiskTier};
use meldframe_rooms::TreeFacts;

/// The assembled server: the registry of compiled-in modules and the in-memory host.
#[derive(Debug)]
pub struct Server {
    registry: Registry,
    gate: Gate,
    room: Room<MemoryStore>,
}

impl Server {
    /// Build the registry from the compiled-in feature modules and an in-memory host.
    ///
    /// # Panics
    ///
    /// When two modules clash on a name: that is a build error in the composition, not a runtime case.
    pub fn assemble() -> Self {
        Self::assemble_with(Vec::new())
    }

    /// Like [`Server::assemble`], with more modules registered after the compiled-in ones (tests and
    /// embedders; a clash is a panic as above).
    pub fn assemble_with(extra: Vec<Box<dyn Module>>) -> Self {
        let mut registry = Registry::new();
        for module in modules().into_iter().chain(extra) {
            registry
                .register(module.as_ref())
                .expect("modules do not clash");
        }
        Server {
            registry,
            gate: Gate::new(),
            // Placeholder ids until E1.2/E1.3 bring workspaces and files from the store.
            room: Room::open("ws_dev", "file_dev", MemoryStore::new()).expect("empty store opens"),
        }
    }

    /// Serve an already open room instead of the empty placeholder one (embedders and tests; the workspace
    /// and file ids come from the room).
    pub fn with_room(mut self, room: Room<MemoryStore>) -> Self {
        self.room = room;
        self
    }

    /// What the server was built with: modules, tools and their tiers.
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// The room that ops are applied to.
    pub fn room(&self) -> &Room<MemoryStore> {
        &self.room
    }

    /// Submit ops on behalf of an agent tool. The registry's tier for the tool gives its policy and is an
    /// upper bound on the tier of every op: a Read tool cannot change data and a Change tool cannot delete.
    /// A tool the registry does not know is `Deny` (fail closed), and `Ask` counts as forbidden until
    /// approval exists, as in the gate. The gate then applies its own tier policy to each op and is the
    /// only source of a permit. The tree of the room, as it is now, tells the gate which deletes are small
    /// (ADR-0010): a small delete is a Change, so a Change tool may submit it.
    pub fn submit_tool(
        &mut self,
        tool: &str,
        who: &Principal,
        ops: Vec<Op>,
    ) -> Result<Report, Refusal> {
        let facts = TreeFacts::new(self.room.nodes());
        let highest = ops
            .iter()
            .map(|op| op.tier_with(&facts))
            .max_by_key(|t| rank(*t));
        let Some(tool_tier) = self.registry.agent_tool(tool).map(|t| t.tier) else {
            return Err(forbidden(highest.unwrap_or(RiskTier::Read), Policy::Deny));
        };
        match self.registry.policy_for(tool) {
            Some(Policy::Auto) => {}
            Some(policy) => return Err(forbidden(tool_tier, policy)),
            None => return Err(forbidden(tool_tier, Policy::Deny)),
        }
        if let Some(op_tier) = highest.filter(|t| rank(*t) > rank(tool_tier)) {
            return Err(forbidden(op_tier, Policy::Deny));
        }
        self.gate.submit_with(who, ops, &facts)
    }
}

fn rank(tier: RiskTier) -> u8 {
    match tier {
        RiskTier::Read => 0,
        RiskTier::Change => 1,
        RiskTier::Delete => 2,
    }
}

fn forbidden(tier: RiskTier, policy: Policy) -> Refusal {
    Refusal::Forbidden { tier, policy }
}

/// The feature modules compiled into this build: one entry per enabled cargo feature.
fn modules() -> Vec<Box<dyn Module>> {
    vec![
        #[cfg(feature = "example")]
        Box::new(example::ExampleModule),
    ]
}
