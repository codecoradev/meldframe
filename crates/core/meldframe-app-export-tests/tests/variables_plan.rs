//! D8: the plan of the Import variables dialog, `variables_import_dialog::plan`: the cap of the dialog, the refusals that
//! name what the file refuses, and the text that is read. It reads no `App`: a function of the text and the file.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_app_model::{ImportStatus, VariableEntity};
use meldframe_protocol::{Command, Node, Variables};
use serde_json::json;
use variables_import_dialog::{MAX_COMMANDS, MAX_VARIABLES, TOO_MANY, plan};

const THEME: &str = include_str!("fixtures/theme.css");

fn empty() -> (Vec<Node>, Variables) {
    (Vec::new(), Variables::default())
}

/// `:root { --n0: 0px; ... }` with `n` variables.
fn root(n: usize) -> String {
    let mut t = String::from(":root {");
    for i in 0..n {
        t.push_str(&format!(" --n{i}: {i}px;"));
    }
    t.push('}');
    t
}

#[test]
fn a_theme_is_planned_as_collections_and_variables_only() {
    let (nodes, vars) = empty();
    let p = plan(THEME, &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Imported);
    assert_eq!(p.commands.len(), 2 + 6);
    assert!(p.commands.iter().all(|c| matches!(
        c,
        Command::AddCollection { .. } | Command::AddVariable { .. }
    )));
}

#[test]
fn the_cap_is_50_variables_in_at_most_55_commands_and_the_two_fit_together() {
    assert_eq!((MAX_VARIABLES, MAX_COMMANDS), (50, 55));
    let (nodes, vars) = empty();
    // 50 variables and the collection they sit in: the most the dialog takes.
    let p = plan(&root(50), &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Imported, "{:?}", p.result);
    assert!(p.commands.len() <= MAX_COMMANDS);
    // Both collections a text can make (root and theme) and 50 variables are still within it.
    let theme: String = (0..25).map(|i| format!("--color-c{i}: #fff;")).collect();
    let two = format!("{} @theme {{ {theme} }}", root(25));
    let p = plan(&two, &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Imported, "{:?}", p.result);
    assert_eq!(
        (
            p.result.report.as_ref().unwrap().collections,
            p.result.report.as_ref().unwrap().variables
        ),
        (2, 50)
    );
    assert!(p.commands.len() <= MAX_COMMANDS);
}

#[test]
fn a_text_of_51_variables_is_refused_with_its_own_code_and_the_counts_and_makes_no_command() {
    let (nodes, vars) = empty();
    let p = plan(&root(51), &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Refused);
    assert_eq!(p.result.code.as_deref(), Some(TOO_MANY));
    assert_eq!(TOO_MANY, "E_IMPORT_TOO_MANY");
    assert_eq!(
        (
            p.result.report.as_ref().unwrap().variables,
            p.result.max_variables
        ),
        (51, 50)
    );
    assert!(p.commands.is_empty());
    assert_eq!(p.result.at, None);
}

#[test]
fn a_text_of_more_than_the_reader_counts_is_refused_with_the_count_it_made_as_a_lower_bound() {
    // The reader stops at 5,000 variables (the limit of a file), so a text of 5,100 reports 5,000: the front end says
    // "more than 50" and never quotes the count as the text's own.
    let (nodes, vars) = empty();
    let p = plan(&root(5_100), &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Refused);
    assert_eq!(p.result.code.as_deref(), Some(TOO_MANY));
    assert_eq!(p.result.report.as_ref().unwrap().variables, 5_000);
    assert!(p.result.report.as_ref().unwrap().variables > p.result.max_variables);
    assert!(p.commands.is_empty());
}

#[test]
fn the_cap_comes_before_the_check_of_the_file() {
    // The file holds the ids of the text already: the text is too long first, whatever the file holds.
    let (nodes, _) = empty();
    let held: Vec<_> = (0..10)
        .map(|i| json!({"id": format!("var-root-n{i}"), "collection": "col-root", "name": format!("n{i}"), "type": "number", "values": {"m": 1}}))
        .collect();
    let vars: Variables = serde_json::from_value(json!({
        "collections": [{"id": "col-root", "name": "root", "modes": [{"id": "m", "name": "default"}]}],
        "variables": held
    }))
    .unwrap();
    assert_eq!(
        plan(&root(10), &nodes, &vars).result.code.as_deref(),
        Some("E_DUPLICATE_ID")
    );
    assert_eq!(
        plan(&root(51), &nodes, &vars).result.code.as_deref(),
        Some(TOO_MANY)
    );
}

#[test]
fn a_duplicate_variable_under_a_new_collection_is_named_by_its_place_above_zero() {
    // The file has a variable with the id the import gives `--background`, in another collection. The collection of the
    // import is new, so the first command is fine and the second is the first duplicate.
    let (nodes, _) = empty();
    let vars: Variables = serde_json::from_value(json!({
        "collections": [{"id": "mine", "name": "mine", "modes": [{"id": "m", "name": "default"}]}],
        "variables": [{"id": "var-root-background", "collection": "mine", "name": "x", "type": "color", "values": {"m": "#fff"}}]
    }))
    .unwrap();
    let p = plan(":root { --background: #fff; }", &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Refused);
    assert_eq!(p.result.code.as_deref(), Some("E_DUPLICATE_ID"));
    let at = p.result.at.unwrap();
    assert_eq!(
        (at.entity, at.name.as_str()),
        (VariableEntity::Variable, "background")
    );
    assert!(p.commands.is_empty());
}

#[test]
fn a_file_near_its_ceiling_refuses_the_import_at_the_variable_that_goes_over_and_names_it() {
    let (nodes, _) = empty();
    let existing: Vec<_> = (0..4_995)
        .map(|i| json!({"id": format!("e{i}"), "collection": "mine", "name": format!("e{i}"), "type": "number", "values": {"m": 1}}))
        .collect();
    let vars: Variables = serde_json::from_value(json!({
        "collections": [{"id": "mine", "name": "mine", "modes": [{"id": "m", "name": "default"}]}],
        "variables": existing
    }))
    .unwrap();
    // Five more fit; the sixth is over the 5,000 of a file.
    let p = plan(&root(10), &nodes, &vars);
    assert_eq!(p.result.status, ImportStatus::Refused);
    assert_eq!(p.result.code.as_deref(), Some("E_TOO_MANY_ENTRIES"));
    let at = p.result.at.unwrap();
    assert_eq!(
        (at.entity, at.name.as_str()),
        (VariableEntity::Variable, "n5")
    );
    assert!(p.commands.is_empty());
    assert_eq!(
        plan(&root(5), &nodes, &vars).result.status,
        ImportStatus::Imported
    );
}

#[test]
fn a_byte_order_mark_in_front_of_the_text_is_not_part_of_the_first_selector() {
    let (nodes, vars) = empty();
    let with = plan(&format!("\u{feff}{THEME}"), &nodes, &vars);
    assert_eq!(
        with.result.status,
        ImportStatus::Imported,
        "{:?}",
        with.result.report.as_ref().unwrap().entries
    );
    assert_eq!(
        with.commands.len(),
        plan(THEME, &nodes, &vars).commands.len()
    );
    // The bytes of the mark count against the limit of the text.
    assert_eq!(with.result.bytes, THEME.len() + 3);
}

#[test]
fn empty_text_and_text_over_the_limit_are_refused_before_anything_is_read() {
    let (nodes, vars) = empty();
    assert_eq!(
        plan("  \n", &nodes, &vars).result.code.as_deref(),
        Some("empty_text")
    );
    let big = format!(":root {{ --a: #fff; }}{}", " ".repeat(256 * 1024));
    assert_eq!(
        plan(&big, &nodes, &vars).result.code.as_deref(),
        Some("E_IMPORT_TOO_LARGE")
    );
}
