# Error codes of `meldframe-check`

Every `ApplyError` has a stable machine code, returned by `ApplyError::code()`. A code is a
SCREAMING_SNAKE string that starts with `E_`. Once released, a code never changes meaning and is never
reused; a variant may gain details, but its code stays. The English message may be reworded; match on the
code, not on the message.

A refused command leaves the file unchanged, so an agent can always correct the command and send it again.

A test in `crates/core/meldframe-check/tests/error_codes.rs` reads this table and fails if a variant has no
row, a row has no variant, or two variants share a code. Add the row in the same change that adds the
variant.

| Code | Meaning and typical cause | What an agent should do next |
| --- | --- | --- |
| `E_NODE_NOT_FOUND` | A node id in the command does not exist. The id has a typo, or the node was removed by another actor. | Read the file tree again and use an id that exists. |
| `E_DUPLICATE_ID` | An added node uses an id that is already taken. | Choose a fresh id, or edit the existing node instead of adding it. |
| `E_PARENT_NOT_FOUND` | The new parent does not exist. | Read the file tree again and use an existing frame as parent. |
| `E_PARENT_NOT_FRAME` | The new parent exists but is not a frame; only frames hold children. | Add a frame first, or choose a frame as parent. |
| `E_CYCLE` | A reparent would make a node its own ancestor. | Pick a parent that is not the node or one of its descendants. |
| `E_TOO_MANY_NODES` | The file would hold more nodes than the limit. | Remove nodes, or split the work over several files. Read the limit with `whoami`. |
| `E_TOO_DEEP` | The tree would nest deeper than the limit. | Flatten the nesting; put the node higher in the tree. |
| `E_INVALID_ID` | An id is empty, too long, or uses characters other than letters, digits and `_-:.`. | Rewrite the id within those rules. |
| `E_NOT_FINITE` | A number is NaN or infinite. | Send a finite number. |
| `E_OUT_OF_RANGE` | A number is outside the range its field allows. | Send a value inside the range named in the message. |
| `E_TOO_LONG` | A string is longer than its limit. | Shorten the string to the length named in the message. |
| `E_INVALID_COLOR` | A colour is not hex (`#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`). | Send the colour as hex. |
| `E_INVALID_POS` | A `pos` from a client has bad characters or a bad shape. | Do not send `pos`; use `before` or `after` and let the server compute the position. |
| `E_POS_TOO_LONG` | A `pos` is longer than the limit. A `pos` you sent is simply too long; a computed one means the gap between siblings is too crowded. | If you sent a `pos`, stop sending it and use `before` or `after`. If the server computed it, ask for the siblings to be rebalanced, then send the command again. |
| `E_BAD_REORDER` | A reorder is incoherent: more than one of `before`, `after` and `pos`, or the anchor does not exist or is not a sibling. | Send exactly one of them, and an anchor that is a sibling of the node. |
| `E_TOO_MANY_ENTRIES` | A list field holds more entries than allowed; today `shadows` has more than the limit. | Send fewer entries; merge or drop shadows you do not need. |
| `E_INVALID_FONT_WEIGHT` | `fontWeight` is not 100 to 900 in steps of 100. | Send one of 100, 200, 300, 400, 500, 600, 700, 800 or 900. |
| `E_INVALID_ASSET_ID` | `image.assetId` is not a lowercase hex SHA-256 (64 characters). | Upload the image first and use the asset id the upload returned. |
| `E_IMAGE_ON_TEXT` | An image fill was set on a text node; a text node cannot hold one. | Put the image on a frame or shape behind the text, or remove the fill. |
