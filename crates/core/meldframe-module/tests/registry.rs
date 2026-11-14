//! The registry at its public seam: modules register, front ends read.

use meldframe_module::{Module, Registrar, Registry, RegistryError, RiskTier, Surface};

struct Notes;
impl Module for Notes {
    fn name(&self) -> &'static str {
        "notes"
    }
    fn register(&self, r: &mut Registrar) {
        r.command("notes.add", "Add a note.");
        r.setting("notes.max", "Most notes per file.");
        r.job("notes.sweep", "Remove stale notes.");
        r.agent_tool("list_notes", "List notes.", RiskTier::Read);
        r.agent_tool("delete_note", "Delete a note.", RiskTier::Delete);
        r.panel("notes.panel", "Notes panel.");
    }
}

struct Other(&'static str, &'static str);
impl Module for Other {
    fn name(&self) -> &'static str {
        self.0
    }
    fn register(&self, r: &mut Registrar) {
        r.agent_tool(self.1, "A tool.", RiskTier::Change);
    }
}

#[test]
fn an_empty_registry_lists_nothing() {
    let reg = Registry::new();
    assert!(reg.module_names().is_empty());
    assert!(reg.commands().is_empty());
    assert!(reg.settings().is_empty());
    assert!(reg.jobs().is_empty());
    assert!(reg.agent_tools().is_empty());
    assert!(reg.panels().is_empty());
}

#[test]
fn a_module_registers_all_five_kinds() {
    let mut reg = Registry::new();
    reg.register(&Notes).unwrap();
    assert_eq!(reg.module_names(), ["notes"]);
    assert_eq!(reg.commands()[0].name, "notes.add");
    assert_eq!(reg.commands()[0].module, "notes");
    assert_eq!(reg.settings()[0].name, "notes.max");
    assert_eq!(reg.jobs()[0].name, "notes.sweep");
    assert_eq!(reg.panels()[0].name, "notes.panel");
    let names: Vec<_> = reg.agent_tools().iter().map(|t| t.name).collect();
    assert_eq!(names, ["list_notes", "delete_note"]);
}

#[test]
fn every_registered_tool_has_a_tier_and_a_policy() {
    let mut reg = Registry::new();
    reg.register(&Notes).unwrap();
    assert_eq!(reg.agent_tool("list_notes").unwrap().tier, RiskTier::Read);
    assert_eq!(
        reg.agent_tool("delete_note").unwrap().tier,
        RiskTier::Delete
    );
    assert!(reg.agent_tool("missing").is_none());
    assert_eq!(
        reg.policy_for("delete_note"),
        Some(meldframe_module::Policy::Ask)
    );
    assert_eq!(reg.policy_for("missing"), None);
}

#[test]
fn a_duplicate_module_name_is_refused() {
    let mut reg = Registry::new();
    reg.register(&Other("a", "t1")).unwrap();
    let err = reg.register(&Other("a", "t2")).unwrap_err();
    assert_eq!(err, RegistryError::DuplicateModule { module: "a" });
    assert_eq!(reg.agent_tools().len(), 1);
}

#[test]
fn a_duplicate_name_across_modules_is_refused_and_nothing_is_added() {
    let mut reg = Registry::new();
    reg.register(&Other("a", "same")).unwrap();
    let err = reg.register(&Other("b", "same")).unwrap_err();
    assert_eq!(
        err,
        RegistryError::DuplicateName {
            surface: Surface::AgentTool,
            name: "same",
            module: "b",
            first_module: "a"
        }
    );
    assert_eq!(reg.module_names(), ["a"]);
    assert_eq!(reg.agent_tools().len(), 1);
}

#[test]
fn a_duplicate_name_inside_one_module_is_refused_and_registers_nothing() {
    struct Twice;
    impl Module for Twice {
        fn name(&self) -> &'static str {
            "twice"
        }
        fn register(&self, r: &mut Registrar) {
            r.command("c1", "One.");
            r.command("c1", "Again.");
        }
    }
    let mut reg = Registry::new();
    let err = reg.register(&Twice).unwrap_err();
    assert!(matches!(
        err,
        RegistryError::DuplicateName {
            surface: Surface::Command,
            name: "c1",
            ..
        }
    ));
    assert!(reg.commands().is_empty());
    assert!(reg.module_names().is_empty());
}

#[test]
fn the_same_name_may_exist_on_different_surfaces() {
    struct Mixed;
    impl Module for Mixed {
        fn name(&self) -> &'static str {
            "mixed"
        }
        fn register(&self, r: &mut Registrar) {
            r.command("x", "Command.");
            r.setting("x", "Setting.");
        }
    }
    let mut reg = Registry::new();
    reg.register(&Mixed).unwrap();
    assert_eq!(reg.commands().len(), 1);
    assert_eq!(reg.settings().len(), 1);
}

#[test]
fn an_empty_name_is_refused() {
    let mut reg = Registry::new();
    let err = reg.register(&Other("a", "")).unwrap_err();
    assert_eq!(
        err,
        RegistryError::EmptyName {
            surface: Surface::AgentTool,
            module: "a"
        }
    );
}

fn nothing_registered(reg: &Registry) {
    assert!(reg.module_names().is_empty());
    assert!(reg.commands().is_empty());
    assert!(reg.settings().is_empty());
    assert!(reg.jobs().is_empty());
    assert!(reg.panels().is_empty());
    assert!(reg.agent_tools().is_empty());
}

#[test]
fn an_empty_module_name_is_refused_and_registers_nothing() {
    let mut reg = Registry::new();
    let err = reg.register(&Other("", "tool")).unwrap_err();
    assert_eq!(err, RegistryError::EmptyModuleName);
    nothing_registered(&reg);
}

#[test]
fn a_late_refusal_leaves_every_surface_untouched() {
    // Valid entries on four surfaces come first; the duplicate tool at the end must undo all of them.
    struct Late(&'static str);
    impl Module for Late {
        fn name(&self) -> &'static str {
            self.0
        }
        fn register(&self, r: &mut Registrar) {
            r.command("late.c", "Command.");
            r.setting("late.s", "Setting.");
            r.job("late.j", "Job.");
            r.panel("late.p", "Panel.");
            r.agent_tool("late_tool", "Tool.", RiskTier::Read);
            r.agent_tool("late_tool", "Tool again.", RiskTier::Read);
        }
    }
    let mut reg = Registry::new();
    assert!(reg.register(&Late("late")).is_err());
    nothing_registered(&reg);
}

#[test]
fn a_refused_duplicate_module_changes_no_list() {
    let mut reg = Registry::new();
    reg.register(&Notes).unwrap();
    let before = (
        reg.module_names().to_vec(),
        reg.commands().to_vec(),
        reg.settings().to_vec(),
        reg.jobs().to_vec(),
        reg.panels().to_vec(),
        reg.agent_tools().to_vec(),
    );
    assert!(reg.register(&Notes).is_err());
    let after = (
        reg.module_names().to_vec(),
        reg.commands().to_vec(),
        reg.settings().to_vec(),
        reg.jobs().to_vec(),
        reg.panels().to_vec(),
        reg.agent_tools().to_vec(),
    );
    assert_eq!(before, after);
}
