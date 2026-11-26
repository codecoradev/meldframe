/// The key of one file's rows in an in-memory store: the workspace and the file as two parts, never joined into one
/// string, so a workspace cannot reach another workspace's file by the way its name runs into the file's. The stores of
/// batch headers and of checkpoints (`activity-store`, `checkpoint-store`) both key their maps by it.
pub fn store_key(workspace: &str, file: &str) -> (String, String) {
    (workspace.to_owned(), file.to_owned())
}
