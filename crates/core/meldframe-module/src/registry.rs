//! The module registry: one list of everything feature modules contribute.

use crate::tier::{Policy, RiskTier, default_policy};

/// A feature module. It registers its contributions and never imports another feature module.
pub trait Module {
    /// The unique name of the module.
    fn name(&self) -> &'static str;
    /// Declare commands, settings, jobs, agent tools and panels.
    fn register(&self, registrar: &mut Registrar);
}

/// The kind of contribution; names are unique per surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// A command the editor can run.
    Command,
    /// A setting.
    Setting,
    /// A background job.
    Job,
    /// An agent tool.
    AgentTool,
    /// A panel.
    Panel,
}

/// A command, setting, job or panel contributed by a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The module that registered it.
    pub module: &'static str,
    /// Its unique name on its surface.
    pub name: &'static str,
    /// A one-line English description.
    pub description: &'static str,
}

/// An agent tool contributed by a module. It always has a risk tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTool {
    /// The module that registered it.
    pub module: &'static str,
    /// Its unique name.
    pub name: &'static str,
    /// A one-line English description.
    pub description: &'static str,
    /// How much a call can change user data.
    pub tier: RiskTier,
}

/// Why a module was refused. A refused module adds nothing to the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// The module name is empty.
    EmptyModuleName,
    /// A module with this name is already registered.
    DuplicateModule {
        /// The module name.
        module: &'static str,
    },
    /// A name is used twice on one surface, in one module or across modules.
    DuplicateName {
        /// The surface.
        surface: Surface,
        /// The repeated name.
        name: &'static str,
        /// The module that tried to register it.
        module: &'static str,
        /// The module that registered it first.
        first_module: &'static str,
    },
    /// A name is empty.
    EmptyName {
        /// The surface.
        surface: Surface,
        /// The module that tried to register it.
        module: &'static str,
    },
}

/// Collects what one module registers.
#[derive(Debug)]
pub struct Registrar {
    module: &'static str,
    commands: Vec<Entry>,
    settings: Vec<Entry>,
    jobs: Vec<Entry>,
    panels: Vec<Entry>,
    tools: Vec<AgentTool>,
}

impl Registrar {
    fn new(module: &'static str) -> Self {
        Registrar {
            module,
            commands: Vec::new(),
            settings: Vec::new(),
            jobs: Vec::new(),
            panels: Vec::new(),
            tools: Vec::new(),
        }
    }

    fn entry(&self, name: &'static str, description: &'static str) -> Entry {
        Entry {
            module: self.module,
            name,
            description,
        }
    }

    /// Register a command.
    pub fn command(&mut self, name: &'static str, description: &'static str) {
        let e = self.entry(name, description);
        self.commands.push(e);
    }

    /// Register a setting.
    pub fn setting(&mut self, name: &'static str, description: &'static str) {
        let e = self.entry(name, description);
        self.settings.push(e);
    }

    /// Register a job.
    pub fn job(&mut self, name: &'static str, description: &'static str) {
        let e = self.entry(name, description);
        self.jobs.push(e);
    }

    /// Register a panel.
    pub fn panel(&mut self, name: &'static str, description: &'static str) {
        let e = self.entry(name, description);
        self.panels.push(e);
    }

    /// Register an agent tool. The tier is required, so no tool lacks one.
    pub fn agent_tool(&mut self, name: &'static str, description: &'static str, tier: RiskTier) {
        self.tools.push(AgentTool {
            module: self.module,
            name,
            description,
            tier,
        });
    }
}

/// The single list of everything modules contribute, read by the editor, MCP and REST.
#[derive(Debug, Default)]
pub struct Registry {
    modules: Vec<&'static str>,
    commands: Vec<Entry>,
    settings: Vec<Entry>,
    jobs: Vec<Entry>,
    panels: Vec<Entry>,
    tools: Vec<AgentTool>,
}

fn check<'a, T: 'a>(
    surface: Surface,
    module: &'static str,
    existing: &'a [T],
    new: &'a [T],
    key: impl Fn(&T) -> (&'static str, &'static str),
) -> Result<(), RegistryError> {
    for (i, item) in new.iter().enumerate() {
        let (name, _) = key(item);
        if name.is_empty() {
            return Err(RegistryError::EmptyName { surface, module });
        }
        let earlier = existing
            .iter()
            .chain(&new[..i])
            .map(&key)
            .find(|(n, _)| *n == name);
        if let Some((_, first_module)) = earlier {
            return Err(RegistryError::DuplicateName {
                surface,
                name,
                module,
                first_module,
            });
        }
    }
    Ok(())
}

impl Registry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a module. On any error nothing from the module is added.
    pub fn register(&mut self, module: &dyn Module) -> Result<(), RegistryError> {
        let name = module.name();
        if name.is_empty() {
            return Err(RegistryError::EmptyModuleName);
        }
        if self.modules.contains(&name) {
            return Err(RegistryError::DuplicateModule { module: name });
        }
        let mut r = Registrar::new(name);
        module.register(&mut r);
        let entry = |e: &Entry| (e.name, e.module);
        check(Surface::Command, name, &self.commands, &r.commands, entry)?;
        check(Surface::Setting, name, &self.settings, &r.settings, entry)?;
        check(Surface::Job, name, &self.jobs, &r.jobs, entry)?;
        check(Surface::Panel, name, &self.panels, &r.panels, entry)?;
        check(Surface::AgentTool, name, &self.tools, &r.tools, |t| {
            (t.name, t.module)
        })?;
        self.modules.push(name);
        self.commands.extend(r.commands);
        self.settings.extend(r.settings);
        self.jobs.extend(r.jobs);
        self.panels.extend(r.panels);
        self.tools.extend(r.tools);
        Ok(())
    }

    /// The registered module names, in registration order.
    pub fn module_names(&self) -> &[&'static str] {
        &self.modules
    }

    /// Every command.
    pub fn commands(&self) -> &[Entry] {
        &self.commands
    }

    /// Every setting.
    pub fn settings(&self) -> &[Entry] {
        &self.settings
    }

    /// Every job.
    pub fn jobs(&self) -> &[Entry] {
        &self.jobs
    }

    /// Every panel.
    pub fn panels(&self) -> &[Entry] {
        &self.panels
    }

    /// Every agent tool, each with its tier.
    pub fn agent_tools(&self) -> &[AgentTool] {
        &self.tools
    }

    /// One agent tool by name.
    pub fn agent_tool(&self, name: &str) -> Option<&AgentTool> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// The default policy of a registered tool, or `None` if no such tool is registered. The gate must treat
    /// `None` as Deny: a tool that is not registered is never let through.
    pub fn policy_for(&self, tool: &str) -> Option<Policy> {
        self.agent_tool(tool).map(|t| default_policy(t.tier))
    }
}
