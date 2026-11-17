//! The list of patchable node fields, written once. `NodePatch` and `Node` use the same names, so a
//! helper that walks every field is a one-line macro over this list.

/// Calls `$m!(field, field, ...)` with every plain field of `NodePatch`. `image` and the four paddings per side
/// (`pad_t`, `pad_r`, `pad_b`, `pad_l`) are not in the list: they have three states and each helper handles
/// them by hand.
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
            mar_t,
            mar_r,
            mar_b,
            mar_l,
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
                let meldframe_protocol::NodePatch { $($f: _,)* image: _, pad_t: _, pad_r: _, pad_b: _, pad_l: _ } = p;
            }
        };
    }
    patch_fields!(destructure_all);
};
