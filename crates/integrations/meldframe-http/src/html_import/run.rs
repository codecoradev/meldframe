//! One call from its checked arguments to its answer: the import, the placement of the root and the dry run.

use super::answer;
use super::check::Call;
use super::tool::{Body, HtmlImport};
use super::write::Write;
use crate::Reply;
use crate::mcp::{FileView, fnv1a, tool_error as error, tool_failed as failed};
use html_import::map::{ImportError, Imported, MAX_COMMANDS, Options, import};
use meldframe_host::Principal;
use meldframe_protocol::{Command, Doc, Node, Variables};
use serde_json::{Map, Value, json};

/// The room between the root and the frames beside it, as `make_variant` leaves.
const GAP: f64 = meldframe_variant::GAP;

/// FNV-1a over the parts, each followed by a zero byte.
fn fnv(parts: &[&[u8]]) -> u64 {
    fnv1a(
        parts
            .iter()
            .flat_map(|p| p.iter().copied().chain(std::iter::once(0))),
    )
}

impl HtmlImport {
    /// The file as the tool needs it: nodes and variables, from one read that is not a call of its own.
    async fn view(&self, who: &Principal, file: &str) -> Result<(FileView, Variables), Value> {
        let (reader, w, f) = (self.reader.clone(), who.clone(), file.to_owned());
        match self
            .shared
            .blocking(who, move || reader.view_with_variables(&w, &f))
            .await
        {
            Err(reply) => Err(failed(&reply)),
            Ok(Err(failure)) => Err(failed(&Reply::failure(&failure))),
            Ok(Ok(v)) => Ok(v),
        }
    }

    pub(super) async fn run(&self, who: &Principal, a: Map<String, Value>) -> Body {
        let Call { args, key, note } = self.check(a)?;
        let dry = args.dry_run.unwrap_or(false);
        let css = args.css.clone().unwrap_or_default();
        let width = args.width.unwrap_or(1440.0);
        let digest = format!(
            "{:012x}",
            fnv(&[
                args.html.as_bytes(),
                css.as_bytes(),
                args.name.as_deref().unwrap_or("").as_bytes(),
                &width.to_bits().to_le_bytes(),
                args.parent_id.as_deref().unwrap_or("").as_bytes(),
            ]) & 0xffff_ffff_ffff
        );
        // A dry run without a key makes its ids from the key `dry-run`: they are not the ids of a call that writes. The
        // actor is in the hash, so two actors that send the same key and the same HTML do not make the same ids.
        let key = key.unwrap_or_else(|| "dry-run".into());
        let prefix = format!(
            "imp-{:012x}",
            fnv(&[
                who.actor.as_bytes(),
                args.file.as_bytes(),
                key.as_bytes(),
                digest.as_bytes()
            ]) & 0xffff_ffff_ffff
        );
        let opts = Options {
            id_prefix: prefix,
            name: args.name.clone(),
            width,
            origin: (0.0, 0.0),
            parent: args.parent_id.clone(),
        };
        let html = args.html.clone();
        let mut imported = match self
            .shared
            .blocking(who, move || import(&html, &css, &opts))
            .await
        {
            Err(reply) => return Err(failed(&reply)),
            Ok(Err(e)) => return Err(refusal(&e)),
            Ok(Ok(i)) => i,
        };
        if args.strict.unwrap_or(false) && imported.unsupported_count() > 0 {
            let mut e = error(
                "E_IMPORT_HTML_UNSUPPORTED",
                format!(
                    "strict is on and {} properties were not carried (the first 100 are listed under unsupported): send strict false to import what can be read, or change the HTML",
                    imported.unsupported_count()
                ),
                false,
            );
            e["error"]["unsupported"] = Value::Array(answer::unsupported(&imported));
            return Err(e);
        }
        // The file is read after the text is known to be importable. Where the root goes: beside the other top-level
        // frames, or where the root of an earlier call stands when the file already holds it, so that a retry sends the
        // commands the first call wrote.
        let view = if args.parent_id.is_none() || dry {
            Some(self.view(who, &args.file).await?)
        } else {
            None
        };
        let replay = view
            .as_ref()
            .is_some_and(|(v, _)| v.nodes.iter().any(|n| n.id == imported.root));
        if let (Some((v, _)), None) = (&view, &args.parent_id)
            && let Some(Command::AddNode { node, .. }) = imported.commands.first_mut()
        {
            (node.x, node.y) = place(&v.nodes, &imported.root);
        }
        let mut out = answer::result(&imported);
        if dry {
            // A dry run after the call that wrote is that call again: its root is in the file, so nothing is applied.
            if let Some((v, vars)) = view.filter(|_| !replay) {
                preflight(&imported, v.nodes, vars)?;
            }
            out["dryRun"] = json!(true);
            out["written"] = json!(0);
            out["replayed"] = json!(replay);
            return Ok(out);
        }
        let call = Write {
            file: &args.file,
            key: &key,
            digest: &digest,
            note,
            imported: &imported,
        };
        self.write(who, call, out).await
    }
}

/// To the right of every top-level frame, [`GAP`] apart, at the top; where the root of an earlier call stands when
/// the file already holds it.
fn place(nodes: &[Node], root: &str) -> (f64, f64) {
    if let Some(n) = nodes.iter().find(|n| n.id == root) {
        return (n.x, n.y);
    }
    let right = nodes
        .iter()
        .filter(|n| n.parent_id.is_none())
        .map(|n| n.x + n.w)
        .reduce(f64::max);
    (right.map_or(0.0, |r| r + GAP), 0.0)
}

/// The refusals of the importer as tool errors.
fn refusal(e: &ImportError) -> Value {
    error(e.code(), e.to_string(), false)
}

/// What a write would be refused for: the commands applied in order to a copy of the file under the limits of the
/// server, as `make_variant` checks its copy. The error names the command and the element it came from.
fn preflight(i: &Imported, nodes: Vec<Node>, vars: Variables) -> Result<(), Value> {
    let mut doc = Doc { nodes, vars };
    let limits = meldframe_core::Limits::default();
    for (n, c) in i.commands.iter().enumerate() {
        if let Err(e) = meldframe_core::apply_doc(&mut doc, c, &limits) {
            let mut v = error(e.code(), e.to_string(), false);
            v["error"]["index"] = json!(n);
            v["error"]["source"] = json!(i.nodes[n].source);
            v["error"]["nodeId"] = json!(i.nodes[n].id);
            return Err(v);
        }
    }
    debug_assert!(i.commands.len() <= MAX_COMMANDS);
    Ok(())
}
