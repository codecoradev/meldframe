//! D8: the Import variables action. CSS text is read by the `variables-import` feature and its commands are applied as
//! one undo step, the person's own edit, through the code of `Action::EditVariables`. A refusal applies nothing, keeps its
//! reason in `AppState::variables_io.import` and sends nothing.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "../../meldframe-app-unit-tests/tests/common/mod.rs"]
mod common;
use common::*;
use meldframe_app::{Action, ImportStatus, VariableEntity, VariablesImportResult};
use meldframe_protocol::Command;
use serde_json::json;

const THEME: &str = include_str!("fixtures/theme.css");

fn result(l: &Loopback) -> VariablesImportResult {
    l.app.state().variables_io.import.clone().unwrap()
}

fn import(l: &mut Loopback, text: &str) -> Vec<meldframe_app::Effect> {
    l.send(Action::ImportVariables { text: text.into() })
}

#[test]
fn a_theme_is_added_as_the_persons_own_ops_and_the_report_counts_it() {
    let mut l = Loopback::live();
    let fx = import(&mut l, THEME);
    let r = result(&l);
    assert_eq!(r.status, ImportStatus::Imported);
    assert_eq!(r.code, None);
    let rep = r.report.as_ref().expect("the text was read");
    assert_eq!(
        (rep.collections, rep.modes),
        (2, 3),
        "root: light and dark; theme: default"
    );
    assert_eq!((rep.variables, rep.aliases), (6, 1), "{rep:?}");
    assert_eq!(rep.colors + rep.numbers, rep.variables);
    assert_eq!(rep.skipped, 1, "the shadow: {rep:?}");
    assert!(
        rep.entries.iter().any(|e| e.name.contains("shadow")),
        "{rep:?}"
    );
    assert_eq!(l.app.vars().variables.len(), 6);
    // Every command went out as an op of the person, nothing else.
    let ops = sent(&fx);
    assert_eq!(ops.len(), 2 + 6);
    assert!(
        l.app.state().pending.is_empty(),
        "the loopback confirmed them"
    );
}

#[test]
fn one_undo_takes_the_whole_import_back_and_redo_puts_it_again() {
    let mut l = Loopback::live();
    import(&mut l, THEME);
    assert!(l.app.state().history.can_undo);
    l.send(Action::Undo {});
    assert!(l.app.vars().collections.is_empty() && l.app.vars().variables.is_empty());
    assert!(!l.app.state().history.can_undo, "it was one step");
    l.send(Action::Redo {});
    assert_eq!(l.app.vars().variables.len(), 6);
}

#[test]
fn a_second_import_of_names_that_exist_is_refused_naming_the_first_and_applies_nothing() {
    let mut l = Loopback::live();
    import(&mut l, THEME);
    let (vars, seq) = (l.app.vars().clone(), l.seq);
    let fx = import(&mut l, THEME);
    let r = result(&l);
    assert_eq!(r.status, ImportStatus::Refused);
    assert_eq!(r.code.as_deref(), Some("E_DUPLICATE_ID"));
    let at = r.at.unwrap();
    assert_eq!(at.entity, VariableEntity::Collection);
    assert_eq!(
        at.name, "root",
        "the first command of the plan is the first duplicate"
    );
    assert!(sent(&fx).is_empty());
    assert_eq!((l.app.vars().clone(), l.seq), (vars, seq));
    // The undo step is still the first import's: one undo, and the file is empty.
    l.send(Action::Undo {});
    assert!(l.app.vars().variables.is_empty());
}

#[test]
fn a_second_import_with_one_more_variable_still_names_the_collection_first() {
    let mut l = Loopback::live();
    import(&mut l, ":root { --background: #fff; }");
    // The same variable and a new one in the same collection: the collection is a duplicate first, at place 0.
    import(&mut l, ":root { --background: #fff; --extra: #000; }");
    let r = result(&l);
    assert_eq!(r.code.as_deref(), Some("E_DUPLICATE_ID"));
    assert_eq!(r.at.unwrap().name, "root");
    assert_eq!(l.app.vars().variables.len(), 1);
}

#[test]
fn a_variable_of_the_file_with_the_id_of_an_imported_one_is_named_when_the_collection_is_new() {
    // The first duplicate is a variable, at a place above 0: the collection of the import is new.
    let mut l = Loopback::live();
    l.send(Action::EditVariables {
        commands: serde_json::from_value(json!([
            {"type": "addCollection", "collection": {"id": "mine", "name": "mine", "modes": [{"id": "m", "name": "default"}]}},
            {"type": "addVariable", "variable": {"id": "var-root-background", "collection": "mine", "name": "x", "type": "color", "values": {"m": "#ffffff"}}}
        ]))
        .unwrap(),
    });
    let before = l.app.vars().clone();
    let fx = import(&mut l, ":root { --background: #fff; --other: #000; }");
    let r = result(&l);
    assert_eq!(
        (r.status, r.code.as_deref()),
        (ImportStatus::Refused, Some("E_DUPLICATE_ID"))
    );
    let at = r.at.unwrap();
    assert_eq!(
        (at.entity, at.name.as_str()),
        (VariableEntity::Variable, "background")
    );
    assert!(sent(&fx).is_empty());
    assert_eq!(*l.app.vars(), before);
}

#[test]
fn a_collection_the_person_made_by_hand_with_the_same_name_is_not_a_duplicate() {
    // Duplicates are found by the ids the import gives (`col-root`, `var-root-background`): a collection of the same
    // name with another id is allowed by the model, so two of them exist. A known limit, written in the dialog's help.
    let mut l = Loopback::live();
    l.send(Action::CreateCollection {
        name: "root".into(),
    });
    import(&mut l, ":root { --background: #fff; }");
    assert_eq!(result(&l).status, ImportStatus::Imported);
    let named: Vec<_> = l
        .app
        .vars()
        .collections
        .iter()
        .filter(|c| c.name == "root")
        .collect();
    assert_eq!(named.len(), 2);
}

#[test]
fn empty_text_is_refused_and_says_so() {
    let mut l = Loopback::live();
    for text in ["", "   \n\t  "] {
        let fx = import(&mut l, text);
        let r = result(&l);
        assert_eq!(
            (r.status, r.code.as_deref()),
            (ImportStatus::Refused, Some("empty_text"))
        );
        assert!(sent(&fx).is_empty());
    }
}

#[test]
fn text_over_256_kib_is_refused_before_it_is_read() {
    let mut l = Loopback::live();
    let text = format!(":root {{ --a: #fff; }}{}", " ".repeat(256 * 1024));
    let fx = import(&mut l, &text);
    let r = result(&l);
    assert_eq!(r.code.as_deref(), Some("E_IMPORT_TOO_LARGE"));
    assert_eq!((r.bytes, r.limit), (text.len(), 256 * 1024));
    assert!(sent(&fx).is_empty() && l.app.vars().variables.is_empty());
    // The limit is in bytes: 256 KiB exactly is read.
    let ok = format!(":root {{ --a: #fff; }}{}", " ".repeat(256 * 1024 - 20));
    assert_eq!(ok.len(), 256 * 1024);
    import(&mut l, &ok);
    assert_eq!(result(&l).status, ImportStatus::Imported);
}

#[test]
fn text_with_no_variable_says_so_with_the_report_and_applies_nothing() {
    let mut l = Loopback::live();
    let fx = import(&mut l, ".card { --local: #fff; } body { color: red; }");
    let r = result(&l);
    assert_eq!((r.status, r.code), (ImportStatus::Nothing, None));
    assert_eq!(r.report.as_ref().unwrap().variables, 0);
    assert!(
        !r.report.as_ref().unwrap().entries.is_empty(),
        "the report says what was skipped"
    );
    assert!(sent(&fx).is_empty());
    assert!(!l.app.state().history.can_undo);
}

#[test]
fn the_names_and_messages_of_a_report_are_made_safe_where_they_enter_the_state() {
    // The importer echoes text of the style sheet in a report line; the dialog makes each name and message safe once more
    // with the display profile (rule U9): no pile of stacked marks, no no-break space, as in the export dialog.
    let (pile, five) = ("\u{0301}".repeat(100), "\u{0301}".repeat(5));
    let mut l = Loopback::live();
    import(
        &mut l,
        &format!(".card{pile}\u{a0}\u{a0}x {{ --local: #fff; }} body {{ color: red; }}"),
    );
    let entries = result(&l).report.unwrap().entries;
    assert!(
        entries
            .iter()
            .any(|e| e.name.contains('\u{0301}') || e.message.contains('\u{0301}')),
        "the selector is echoed: {entries:?}"
    );
    for e in entries {
        for text in [&e.name, &e.message] {
            assert!(!text.contains(&five), "{text:?}");
            assert!(!text.contains('\u{a0}'), "{text:?}");
        }
    }
}

#[test]
fn hostile_text_is_read_without_panic_and_nothing_it_holds_runs() {
    let mut l = Loopback::live();
    let deep = format!("{}{}", "a{".repeat(100_000), "}".repeat(100_000));
    for text in [
        deep.as_str(),
        "@import url(https://example.com/x.css); :root { --x: url(javascript:alert(1)); }",
        ":root { --a: \u{202e}#fff; --b: calc(1px + 2px); } \u{0}\u{1}",
        "}}}}{{{{ :root{ --\u{0}: 1 }",
    ] {
        import(&mut l, text);
        let r = result(&l);
        assert_ne!(r.status, ImportStatus::Imported, "{r:?}");
        assert!(l.app.vars().variables.is_empty());
    }
}

#[test]
fn a_list_that_does_not_fit_the_queue_is_refused_whole() {
    let mut l = Loopback::live();
    l.app.set_max_pending(3);
    let fx = import(&mut l, THEME);
    let r = result(&l);
    assert_eq!(
        (r.status, r.code.as_deref()),
        (ImportStatus::Refused, Some("outbox_full"))
    );
    assert!(sent(&fx).is_empty() && l.app.vars().collections.is_empty());
}

#[test]
fn the_result_stays_until_it_is_closed_or_another_import_replaces_it() {
    let mut l = Loopback::live();
    import(&mut l, THEME);
    l.send(Action::Select { ids: ids(&["r1"]) });
    assert_eq!(result(&l).status, ImportStatus::Imported);
    l.send(Action::CloseVariablesImport {});
    assert!(l.app.state().variables_io.import.is_none());
    import(&mut l, "");
    assert_eq!(result(&l).code.as_deref(), Some("empty_text"));
}

/// `:root { --v0: 0px; ... }` with `n` variables.
fn root(n: usize) -> String {
    let mut t = String::from(":root {");
    for i in 0..n {
        t.push_str(&format!(" --v{i}: {i}px;"));
    }
    t.push('}');
    t
}

#[test]
fn the_largest_import_the_dialog_takes_is_applied_and_confirmed_and_undone_in_time() {
    // 50 variables and their collection: 51 ops that the loopback confirms one by one, as the server's echoes do. The
    // replica's cost of confirming grows faster than the count (about n squared), which is why the cap is small.
    let mut l = Loopback::live();
    #[cfg(not(target_arch = "wasm32"))]
    let t0 = std::time::Instant::now();
    let fx = import(&mut l, &root(50));
    assert_eq!(
        result(&l).status,
        ImportStatus::Imported,
        "{:?}",
        result(&l)
    );
    assert_eq!(sent(&fx).len(), 51);
    assert_eq!(l.app.vars().variables.len(), 50);
    assert!(l.app.state().pending.is_empty(), "every op was confirmed");
    l.send(Action::Undo {});
    assert!(l.app.vars().variables.is_empty(), "one undo");
    assert!(
        l.app.state().pending.is_empty(),
        "and the undo was confirmed"
    );
    #[cfg(not(target_arch = "wasm32"))]
    assert!(
        t0.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        t0.elapsed()
    );
}

#[test]
fn a_text_of_51_variables_is_refused_with_the_pointer_to_the_agent_tool_and_nothing_is_sent() {
    let mut l = Loopback::live();
    let fx = import(&mut l, &root(51));
    let r = result(&l);
    assert_eq!(
        (r.status, r.code.as_deref()),
        (ImportStatus::Refused, Some("E_IMPORT_TOO_MANY"))
    );
    assert_eq!(
        (r.report.as_ref().unwrap().variables, r.max_variables),
        (51, 50)
    );
    assert!(
        sent(&fx).is_empty()
            && l.app.vars().variables.is_empty()
            && l.app.vars().collections.is_empty()
    );
    assert!(!l.app.state().history.can_undo, "no undo step");
    assert!(l.app.state().pending.is_empty());
}

#[test]
fn the_result_goes_when_the_file_no_longer_holds_the_import() {
    // An undo takes the variables away: an open dialog must not go on saying "Imported".
    let mut l = Loopback::live();
    import(&mut l, THEME);
    assert_eq!(result(&l).status, ImportStatus::Imported);
    l.send(Action::Undo {});
    assert!(l.app.state().variables_io.import.is_none());
    l.send(Action::Redo {});
    assert!(
        l.app.state().variables_io.import.is_none(),
        "a redo does not bring an old result back"
    );
    assert_eq!(l.app.vars().variables.len(), 6);
}

#[test]
fn the_result_goes_when_someone_else_removes_a_variable_the_import_added() {
    let mut l = Loopback::live();
    import(&mut l, ":root { --background: #fff; --other: #000; }");
    assert_eq!(result(&l).status, ImportStatus::Imported);
    l.remote(
        "agent:other",
        "k1",
        1,
        Command::RemoveVariable {
            id: "var-root-other".into(),
        },
    );
    assert_eq!(l.app.vars().variables.len(), 1);
    assert!(l.app.state().variables_io.import.is_none());
}

#[test]
fn a_result_that_is_a_refusal_stays_when_the_file_changes() {
    let mut l = Loopback::live();
    import(&mut l, "");
    l.send(Action::CreateCollection { name: "x".into() });
    assert_eq!(result(&l).code.as_deref(), Some("empty_text"));
}

#[test]
fn only_variable_commands_are_made() {
    let mut l = Loopback::live();
    let fx = import(&mut l, THEME);
    // The reader makes collections (with their modes) and variables, nothing else: no `AddMode`, no `SetVariableValue`.
    let sent = sent(&fx);
    assert!(
        sent.iter()
            .any(|c| matches!(c, Command::AddCollection { .. }))
    );
    assert!(
        sent.iter()
            .any(|c| matches!(c, Command::AddVariable { .. }))
    );
    assert!(
        sent.iter().all(|c| matches!(
            c,
            Command::AddCollection { .. } | Command::AddVariable { .. }
        )),
        "{sent:?}"
    );
}
