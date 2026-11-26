//! The Import variables dialog (D8), the part that reads no `App`: CSS text to the commands that add its variables to
//! the person's file, and the result the dialog shows. The text is read by the `variables-import` feature (nothing is
//! run or fetched, the size and the count of what it makes are bounded); the commands are then checked, one after the
//! other, on what the ones before them make of the file, exactly as `Action::EditVariables` checks them, so a list
//! that would fail half way is refused before anything is applied.
//!
//! An import is at most [`MAX_VARIABLES`] variables: the browser sends one request for each op. An import only adds. Ids come from names (`col-root`, `var-root-brand`), so text that names a collection or a
//! variable the file already holds fails at its first such command with `E_DUPLICATE_ID`, and the result names it.

use meldframe_app_model::{
    ImportAt, ImportEntry, ImportEntryKind, ImportReport, ImportStatus, VariableEntity,
    VariablesImportResult,
};
use meldframe_app_rules::modes::{EditRefusal, check_edit_at};
use meldframe_protocol::{Command, Node, Variables};
use variables_import::{
    EntryKind, Import, ImportError, ImportOptions, MAX_INPUT_BYTES, import_css,
};

/// The most variables the dialog imports at a time.
///
/// The browser sends the commands of an import to the server as one POST for each op, and the server gives a client a burst
/// of 60 requests and 30 a second after it (`burst: 60`, `per_second: 30` in `meldframe-http`); an op it refuses is tried
/// again 3 times and then rolled back with the ops that depend on it. An import of more than about 60 commands can
/// therefore be rolled back after the dialog said it was done. The cap keeps an import inside the burst. A longer
/// style sheet goes through the agent tool `import_tokens`, which cuts it into atomic parts.
pub const MAX_VARIABLES: usize = 50;

/// The most commands of an import: the burst of 60, less 5 for the person's own ops sent in the same moments.
pub const MAX_COMMANDS: usize = 55;

/// The collections a text can make, besides its variables: `root` and `theme` (ADR-0020). An import is at most this
/// many `AddCollection` commands and one `AddVariable` for each variable, and no command of another kind.
const COLLECTIONS: usize = 2;
const _: () = assert!(COLLECTIONS + MAX_VARIABLES <= MAX_COMMANDS);

/// The refusal code of a text with more variables than [`MAX_VARIABLES`] (`docs/error-codes.md`).
pub const TOO_MANY: &str = "E_IMPORT_TOO_MANY";

/// What an import comes to before anything is applied: the result to show, and the commands to apply when it is
/// [`ImportStatus::Imported`] (none otherwise).
#[derive(Debug, Clone)]
pub struct Plan {
    pub result: VariablesImportResult,
    pub commands: Vec<Command>,
}

fn result(status: ImportStatus, bytes: usize) -> VariablesImportResult {
    VariablesImportResult {
        status,
        code: None,
        at: None,
        bytes,
        limit: MAX_INPUT_BYTES,
        max_variables: MAX_VARIABLES,
        report: None,
    }
}

fn refused(bytes: usize, code: &str) -> Plan {
    let mut r = result(ImportStatus::Refused, bytes);
    r.code = Some(code.to_string());
    Plan {
        result: r,
        commands: Vec::new(),
    }
}

/// Read `text` and plan its import into the file of `nodes` and `vars`.
pub fn plan(text: &str, nodes: &[Node], vars: &Variables) -> Plan {
    let bytes = text.len();
    if text.trim().is_empty() {
        return refused(bytes, "empty_text");
    }
    let import = match import_css(text, &ImportOptions::default()) {
        Ok(i) => i,
        Err(e @ ImportError::TooLarge { .. }) => return refused(bytes, e.code()),
    };
    let report = report_of(&import);
    if import.commands.is_empty() {
        let mut r = result(ImportStatus::Nothing, bytes);
        r.report = Some(report);
        return Plan {
            result: r,
            commands: Vec::new(),
        };
    }
    let mut r = result(ImportStatus::Imported, bytes);
    r.report = Some(report);
    // Before the check of the file: the check is the slow part, and a text over the cap is refused whatever the file holds.
    // The count is that of the reader, which stops at the limit of a file (5,000): for a longer text it is 5,000, a lower
    // bound, so the refusal and the front end say "more than the cap" and never quote the count as the text's own.
    if import.report.variables > MAX_VARIABLES {
        r.status = ImportStatus::Refused;
        r.code = Some(TOO_MANY.to_string());
        return Plan {
            result: r,
            commands: Vec::new(),
        };
    }
    // The commands are at most COLLECTIONS + MAX_VARIABLES, inside MAX_COMMANDS (the const assert above), so no test of
    // their number is needed after the test of the variables.
    debug_assert!(import.commands.len() <= MAX_COMMANDS);
    match check_edit_at(nodes, vars, &import.commands) {
        Ok(()) => Plan {
            result: r,
            commands: import.commands,
        },
        Err((at, refusal)) => {
            // The commands are the import's own, so a refusal is the file's: its code, and the name the text gave.
            r.status = ImportStatus::Refused;
            r.code = Some(match refusal {
                EditRefusal::Invalid(e) => e.code().to_string(),
                EditRefusal::NotVariable => "not_a_variable_command".to_string(),
            });
            r.at = import.commands.get(at).and_then(subject);
            Plan {
                result: r,
                commands: Vec::new(),
            }
        }
    }
}

/// What a command adds, by name.
fn subject(c: &Command) -> Option<ImportAt> {
    let (entity, name) = match c {
        Command::AddCollection { collection } => (VariableEntity::Collection, &collection.name),
        Command::AddMode { mode, .. } => (VariableEntity::Mode, &mode.name),
        Command::AddVariable { variable } => (VariableEntity::Variable, &variable.name),
        _ => return None,
    };
    Some(ImportAt {
        entity,
        name: name.clone(),
    })
}

fn report_of(i: &Import) -> ImportReport {
    let r = &i.report;
    ImportReport {
        collections: r.collections,
        modes: r.modes,
        variables: r.variables,
        colors: r.colors,
        numbers: r.numbers,
        aliases: r.aliases,
        skipped: r.skipped,
        entries: r
            .entries
            .iter()
            .map(|e| ImportEntry {
                kind: match e.kind {
                    EntryKind::Converted => ImportEntryKind::Converted,
                    EntryKind::Skipped => ImportEntryKind::Skipped,
                    EntryKind::Note => ImportEntryKind::Note,
                },
                code: e.code.to_string(),
                name: e.name.clone(),
                message: e.message.clone(),
            })
            .collect(),
        omitted: r.omitted,
    }
}
