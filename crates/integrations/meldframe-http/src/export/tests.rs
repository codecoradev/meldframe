//! The text an agent reads about the frame id says the cap the check uses.

use super::*;

#[test]
fn the_schema_and_the_refusal_state_the_cap_the_check_applies() {
    let schema = input_schema();
    let frame = &schema["properties"]["frame"];
    assert_eq!(frame["maxLength"], MAX_FRAME_ID);
    let said = format!("1 to {MAX_FRAME_ID} letters");
    let description = frame["description"].as_str().unwrap();
    assert!(description.contains(&said), "{description}");
    assert!(frame_refusal().contains(&said), "{}", frame_refusal());
}
