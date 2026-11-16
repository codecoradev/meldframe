//! The placeholder feature registers through the module registry; a build without it lists nothing.

use example::ExampleModule;
use meldframe_module::{Policy, Registry, RegistryError, RiskTier, Surface, default_policy};

#[test]
fn a_module_registers_a_tool() {
    let mut reg = Registry::new();
    reg.register(&ExampleModule).unwrap();
    let tool = reg.agent_tool("example_ping").expect("tool is listed");
    assert_eq!(tool.module, "example");
    assert_eq!(tool.tier, RiskTier::Read);
    assert_eq!(reg.policy_for("example_ping"), Some(Policy::Auto));
}

#[test]
fn a_registry_built_without_the_example_module_does_not_list_its_tool() {
    use meldframe_module::{Module, Registrar};
    struct Elsewhere;
    impl Module for Elsewhere {
        fn name(&self) -> &'static str {
            "elsewhere"
        }
        fn register(&self, r: &mut Registrar) {
            r.agent_tool("elsewhere_read", "Read.", RiskTier::Read);
        }
    }
    // The composition root of this build leaves the example out: only the other module is registered.
    let mut reg = Registry::new();
    reg.register(&Elsewhere).unwrap();
    assert!(reg.agent_tool("example_ping").is_none());
    assert_eq!(reg.policy_for("example_ping"), None);
    assert_eq!(reg.module_names(), ["elsewhere"]);
}

#[test]
fn registering_it_twice_is_refused() {
    let mut reg = Registry::new();
    reg.register(&ExampleModule).unwrap();
    assert_eq!(
        reg.register(&ExampleModule),
        Err(RegistryError::DuplicateModule { module: "example" })
    );
    assert_eq!(reg.agent_tools().len(), 2);
}

#[test]
fn a_tool_name_taken_by_another_module_is_refused() {
    use meldframe_module::{Module, Registrar};
    struct Squatter;
    impl Module for Squatter {
        fn name(&self) -> &'static str {
            "squatter"
        }
        fn register(&self, r: &mut Registrar) {
            r.agent_tool("example_ping", "Taken.", RiskTier::Change);
        }
    }
    let mut reg = Registry::new();
    reg.register(&ExampleModule).unwrap();
    assert_eq!(
        reg.register(&Squatter),
        Err(RegistryError::DuplicateName {
            surface: Surface::AgentTool,
            name: "example_ping",
            module: "squatter",
            first_module: "example"
        })
    );
}

#[test]
fn every_tool_of_the_example_has_a_tier_and_a_policy() {
    let mut reg = Registry::new();
    reg.register(&ExampleModule).unwrap();
    assert!(!reg.agent_tools().is_empty());
    for tool in reg.agent_tools() {
        assert_eq!(reg.policy_for(tool.name), Some(default_policy(tool.tier)));
    }
}

#[test]
fn the_module_also_registers_a_change_tool_so_a_developer_can_write_through_the_gate() {
    let mut reg = Registry::new();
    reg.register(&ExampleModule).unwrap();
    let tool = reg.agent_tool("example_edit").expect("tool is listed");
    assert_eq!((tool.module, tool.tier), ("example", RiskTier::Change));
    assert_eq!(reg.policy_for("example_edit"), Some(Policy::Auto));
}
