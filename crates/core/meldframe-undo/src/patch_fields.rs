//! The list of patchable node fields, written once. `NodePatch` and `Node` use the same names, so a
//! helper that walks every field is a one-line macro over this list.

/// Calls `$m!(field, field, ...)` with every plain field of `NodePatch`. `image` is not in the list: it
/// has three states and each helper handles it by hand.
macro_rules! patch_fields {
    ($m:ident) => {
        $m!(
            name,
            x,
            y,
            w,
            h,
            sw,
            sh,
            fill,
            fill2,
            radius,
            layout,
            gap,
            padding,
            align,
            justify,
            clip,
            text,
            font_size,
            shader,
            font_weight,
            opacity,
            text_align,
            border,
            shadows
        )
    };
}

pub(crate) use patch_fields;

/// Compile-time guard: destructures `NodePatch` with every field named and no `..`, so a field added to
/// `NodePatch` and missing from `patch_fields!` is a compile error, not a silently dropped field.
const _: () = {
    macro_rules! destructure_all {
        ($($f:ident),*) => {
            #[allow(dead_code)]
            fn every_patch_field_is_listed(p: meldframe_protocol::NodePatch) {
                let meldframe_protocol::NodePatch { $($f: _,)* image: _ } = p;
            }
        };
    }
    patch_fields!(destructure_all);
};
