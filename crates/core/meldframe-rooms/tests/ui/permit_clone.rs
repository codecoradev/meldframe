// E0277: a permit is not `Clone`, so one permit cannot be copied to admit a second op.
use meldframe_host::Permit;

fn needs_clone<T: Clone>() {}

fn main() {
    needs_clone::<Permit>();
}
