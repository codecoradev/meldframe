//! From the checked arguments to a plan: the work on the text, which is bounded and runs on the blocking pool. It
//! sets the ids, enforces the limits that are the tool's (100 Commands, `strict`) and says nothing about the file.

use crate::answer;
use crate::args::Args;
use crate::refuse;
use crate::schema::DIGEST_CHARS;
use html_import::html::EntryKind;
use html_import::map::{BATCH_LIMIT, Options, Plan, plan};
use serde_json::{Value, json};

/// The width of the root frame when neither the page nor the call gives one: the Desktop preset.
const DEFAULT_WIDTH: f64 = 1440.0;

/// A plan with what the call adds to it.
#[derive(Debug)]
pub struct Planned {
    pub plan: Plan,
    /// The id of the root frame.
    pub root: String,
    /// The digest of the call: 12 hex digits of a hash of every argument that shapes the Commands.
    pub digest: String,
    /// The key of the batch: the op of Command `i` is `{prefix}:{i}`.
    pub prefix: String,
}

/// FNV-1a over the parts, each with its length first, so no two lists of parts make the same bytes.
fn fnv(parts: &[&[u8]]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    let mut eat = |b: u8| h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    for p in parts {
        (p.len() as u64)
            .to_le_bytes()
            .into_iter()
            .for_each(&mut eat);
        p.iter().copied().for_each(&mut eat);
    }
    h
}

/// A hash of what shapes the Commands: the text, the name, the width and the parent. The same key with other values
/// is another call, and so is the same text under another parent.
fn digest(a: &Args) -> String {
    let width = a
        .width
        .map_or(Vec::new(), |w| w.to_bits().to_le_bytes().to_vec());
    let h = fnv(&[
        a.html.as_bytes(),
        a.css.as_bytes(),
        a.name.as_deref().map_or(&[0][..], str::as_bytes),
        &width,
        a.parent_id.as_deref().map_or(&[0][..], str::as_bytes),
    ]);
    format!("{:0width$x}", h & 0xffff_ffff_ffff, width = DIGEST_CHARS)
}

/// The ids of the nodes: `imp-<hash of file, actor, key and digest>-<i>`, at most 64 characters, letters, digits and
/// `-`. The same call gives the same ids; the same text under another key gives other ids, and so does another actor
/// in the same file, so one page can be imported twice and two actors can send the same key.
fn id_prefix(a: &Args, actor: &str, digest: &str) -> String {
    let h = fnv(&[
        a.file.as_bytes(),
        actor.as_bytes(),
        a.key.as_bytes(),
        digest.as_bytes(),
    ]);
    format!("imp-{h:016x}")
}

/// Read the text and plan the Commands for `actor`. Errors are bodies of a tool error: the parse limits, the batch
/// limit and `strict`.
pub fn make(a: &Args, actor: &str) -> Result<Planned, Value> {
    let digest = digest(a);
    let id_prefix = id_prefix(a, actor, &digest);
    let options = Options {
        width: a.width.unwrap_or(DEFAULT_WIDTH),
        name: a.name.clone(),
        id_prefix: id_prefix.clone(),
    };
    let planned = plan(&a.html, &a.css, &options).map_err(|e| refuse(e.code(), e.to_string()))?;
    if planned.exceeds_batch() {
        return Err(refuse(
            "E_IMPORT_HTML_TOO_MANY",
            format!(
                "the HTML makes {} nodes and a call writes at most {BATCH_LIMIT} as one batch, all or none: send one section at a time, each into a frame of the file with parentId",
                planned.commands.len()
            ),
        ));
    }
    if a.strict {
        let r = &planned.report;
        let refused: Vec<Value> = (r.entries.iter())
            .filter(|e| e.kind == EntryKind::Refused)
            .map(answer::entry)
            .collect();
        // An entry past the bounds of the report may have been a refusal: with `strict` that is not worth the risk.
        if !refused.is_empty() || r.omitted > 0 {
            let mut e = refuse(
                "E_IMPORT_HTML_UNSUPPORTED",
                format!(
                    "strict is on and {} refused entries are listed (and {} more are not): nothing was written. Leave strict out to import what is read and see the rest under unsupported",
                    refused.len(),
                    r.omitted
                ),
            );
            e["error"]["refused"] = json!(refused);
            e["error"]["omitted"] = json!(r.omitted);
            return Err(e);
        }
    }
    let root = format!("{id_prefix}-0");
    let prefix = format!("{}:{digest}", a.key);
    Ok(Planned {
        plan: planned,
        root,
        digest,
        prefix,
    })
}
