//! A tool another crate contributes: listed with the built-in ones, called with the request's principal.

use meldframe_host::Principal;
use rmcp::model::{CallToolResult, Tool};
use serde_json::Value;
use std::{future::Future, pin::Pin};

/// A tool another crate contributes (capture): listed with the others, called with the request's principal.
pub trait Extra: std::fmt::Debug + Send + Sync {
    fn tool(&self) -> Tool;
    fn call(&self, who: Principal, args: serde_json::Map<String, Value>) -> ExtraCall<'_>;
}

pub type ExtraCall<'a> = Pin<Box<dyn Future<Output = CallToolResult> + Send + 'a>>;
