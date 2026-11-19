//! The `get_variables` tool: the collections of a file and a page of its variables, in bounded output.

use super::Handler;
use super::tools::{Body, args, check_file, error, invalid};
use meldframe_host::Principal;
use meldframe_mcp_defs::view::{self, VariablesError};
use serde::Deserialize;
use serde_json::{Map, Value, json};

impl Handler {
    pub(super) async fn get_variables(&self, who: &Principal, a: Map<String, Value>) -> Body {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct A {
            file: String,
            collection: Option<String>,
            prefix: Option<String>,
            limit: Option<usize>,
            after: Option<String>,
        }
        let a: A = args(a)?;
        let file = a.file;
        check_file(&file)?;
        let name = file.clone();
        let (seq, vars) = self.read(who, move |r, w| r.variables(w, &name)).await?;
        let q = (
            a.collection.as_deref(),
            a.prefix.as_deref(),
            a.after.as_deref(),
        );
        let page = view::variables(&vars, q, a.limit).map_err(|e| match e {
            VariablesError::NoCollection => {
                error("E_COLLECTION_NOT_FOUND", "no such collection", false)
            }
            VariablesError::Invalid(why) => invalid(why),
        })?;
        let mut out = page;
        out["ok"] = json!(true);
        out["file"] = json!(file);
        out["seq"] = json!(seq);
        Ok(out)
    }
}
