//! The in-memory store of the critique lists.

use crate::{
    Item, MAX_BATCHES, MAX_ITEMS, ReviewError, ReviewStore, Status, check_batch, check_texts,
};
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// Lists in memory, for tests and a server without a database. A restart loses them.
#[derive(Debug, Default)]
pub struct MemoryReviews {
    lists: Mutex<HashMap<(String, String), Vec<Item>>>,
}

impl MemoryReviews {
    /// Run `f` on the item `n` of the file's list, or refuse when there is none.
    fn with_item<T>(
        &self,
        workspace: &str,
        file: &str,
        n: u32,
        f: impl FnOnce(&mut Item) -> Result<T, ReviewError>,
    ) -> Result<T, ReviewError> {
        let mut lists = self.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let item = lists
            .get_mut(&(workspace.to_owned(), file.to_owned()))
            .and_then(|l| l.iter_mut().find(|i| i.n == n))
            .ok_or(ReviewError::ItemNotFound(n))?;
        f(item)
    }
}

impl ReviewStore for MemoryReviews {
    fn add(
        &self,
        workspace: &str,
        file: &str,
        actor: &str,
        texts: &[String],
    ) -> Result<Vec<Item>, ReviewError> {
        let texts = check_texts(texts)?;
        let mut lists = self.lists.lock().unwrap_or_else(PoisonError::into_inner);
        let list = lists
            .entry((workspace.to_owned(), file.to_owned()))
            .or_default();
        if list.len() + texts.len() > MAX_ITEMS {
            return Err(ReviewError::Full { limit: MAX_ITEMS });
        }
        // Items are never removed, so the next number is the count plus one.
        let first = list.len() as u32 + 1;
        let added: Vec<Item> = texts
            .into_iter()
            .zip(first..)
            .map(|(text, n)| Item {
                n,
                text,
                by: actor.to_owned(),
                status: Status::Open,
                batches: Vec::new(),
            })
            .collect();
        list.extend(added.iter().cloned());
        Ok(added)
    }

    fn list(&self, workspace: &str, file: &str) -> Result<Vec<Item>, ReviewError> {
        let lists = self.lists.lock().unwrap_or_else(PoisonError::into_inner);
        Ok(lists
            .get(&(workspace.to_owned(), file.to_owned()))
            .cloned()
            .unwrap_or_default())
    }

    fn link(&self, workspace: &str, file: &str, n: u32, batch: &str) -> Result<Item, ReviewError> {
        check_batch(batch)?;
        self.with_item(workspace, file, n, |item| {
            if !item.batches.iter().any(|b| b == batch) {
                if item.batches.len() >= MAX_BATCHES {
                    return Err(ReviewError::Full { limit: MAX_BATCHES });
                }
                item.batches.push(batch.to_owned());
            }
            Ok(item.clone())
        })
    }

    fn mark(
        &self,
        workspace: &str,
        file: &str,
        n: u32,
        status: Status,
    ) -> Result<Item, ReviewError> {
        self.with_item(workspace, file, n, |item| {
            item.status = status;
            Ok(item.clone())
        })
    }
}
