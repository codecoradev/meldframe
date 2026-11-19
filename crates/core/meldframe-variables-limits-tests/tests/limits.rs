//! The ceilings of the variable model (32 collections, 16 modes, 5,000 variables, name lengths) hold, and an
//! alias check stays linear in the size of the file: a layered diamond has 16^12 paths and must not be walked
//! once per path, neither when one alias is written nor when the file is loaded.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use meldframe_check::{Limits, check_command_with, check_file_with};
use meldframe_protocol::{Bind, Collection, Command, Mode, Value, VarType, Variable, Variables};
use meldframe_variables::{alias_depth, alias_height};
use serde_json::json;
use std::collections::BTreeMap;

const LAYERS: usize = 12;
const WIDTH: usize = 16;

fn modes(n: usize) -> Vec<Mode> {
    (0..n)
        .map(|i| Mode {
            id: format!("m{i}"),
            name: format!("m{i}"),
        })
        .collect()
}

fn var(id: &str, col: &str, name: &str, values: BTreeMap<String, Bind<Value>>) -> Variable {
    Variable {
        id: id.into(),
        collection: col.into(),
        name: name.into(),
        kind: VarType::Color,
        values,
    }
}

/// `LAYERS` layers of `WIDTH` colour variables in one collection of `WIDTH` modes. In mode `j` a variable of
/// layer `i` aliases variable `j` of layer `i + 1`; the last layer holds literals.
fn diamond() -> Variables {
    diamond_of(LAYERS)
}

/// The same diamond with `layers` layers: a chain of `layers - 1` alias steps from the top to the literals.
fn diamond_of(layers: usize) -> Variables {
    let mut v = Variables {
        collections: vec![Collection {
            id: "c".into(),
            name: "c".into(),
            modes: modes(WIDTH),
        }],
        variables: Vec::new(),
    };
    for layer in 0..layers {
        for k in 0..WIDTH {
            let values = (0..WIDTH)
                .map(|j| {
                    let b = if layer + 1 == layers {
                        Bind::Lit(Value::Color("#102030".into()))
                    } else {
                        Bind::var(format!("v{}_{j}", layer + 1))
                    };
                    (format!("m{j}"), b)
                })
                .collect();
            v.variables.push(var(
                &format!("v{layer}_{k}"),
                "c",
                &format!("l{layer}/k{k}"),
                values,
            ));
        }
    }
    v
}

/// Run `f` and fail if it takes longer than `secs`. A linear check takes milliseconds; a walk of every path
/// does not finish, so the bound is generous and the test is not flaky.
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(secs))
            .expect("the alias check did not finish: it walks every path")
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = secs;
        f()
    }
}

#[test]
fn depth_and_height_of_a_layered_diamond_are_worked_out_without_walking_paths() {
    let (d, h) = within(10, || {
        let v = diamond();
        (
            alias_depth(&v, "v0_0"),
            alias_height(&v, &format!("v{}_0", LAYERS - 1)),
        )
    });
    assert_eq!((d, h), (LAYERS - 1, LAYERS - 1));
}

#[test]
fn a_file_with_a_layered_diamond_loads_fast() {
    let r = within(10, || check_file_with(&[], &diamond(), &Limits::default()));
    assert_eq!(r, Ok(()));
}

#[test]
fn writing_one_alias_into_a_layered_diamond_is_checked_fast() {
    let r = within(10, || {
        let c: Command = serde_json::from_value(json!({
            "type": "setVariableValue", "id": "v0_0", "mode": "m3", "value": {"var": "v1_5"}
        }))
        .unwrap();
        check_command_with(&[], &diamond(), &c, &Limits::default())
    });
    assert_eq!(r, Ok(()));
}

#[test]
fn a_chain_that_closes_the_diamond_back_to_its_top_is_a_cycle() {
    let mut v = diamond();
    v.variables[(LAYERS - 1) * WIDTH]
        .values
        .insert("m0".into(), Bind::var("v0_0"));
    let r = within(10, move || check_file_with(&[], &v, &Limits::default()));
    assert_eq!(r.expect_err("a cycle").code(), "E_ALIAS_CYCLE");
}

#[test]
fn a_diamond_of_more_than_sixteen_steps_is_refused_at_load_and_one_alias_more_is_refused_as_a_command()
 {
    // 17 layers are 16 steps and load; 18 are 17 steps and do not, with 16^17 paths walked never.
    let r = within(10, || {
        check_file_with(&[], &diamond_of(17), &Limits::default())
    });
    assert_eq!(r, Ok(()), "16 steps load");
    let e = within(10, || {
        check_file_with(&[], &diamond_of(18), &Limits::default())
    })
    .expect_err("17 steps");
    assert_eq!(e.code(), "E_ALIAS_TOO_DEEP");
    // As a command: a new variable above the top of the 16-step diamond makes 17.
    let values: serde_json::Map<String, serde_json::Value> = (0..WIDTH)
        .map(|j| (format!("m{j}"), json!({"var": "v0_0"})))
        .collect();
    let add = json!({"type": "addVariable", "variable": {"id": "top", "collection": "c", "name": "top", "type": "color", "values": values}});
    let code = within(10, move || {
        let c: Command = serde_json::from_value(add).unwrap();
        check_command_with(&[], &diamond_of(17), &c, &Limits::default())
            .expect_err("17 steps")
            .code()
    });
    assert_eq!(code, "E_ALIAS_TOO_DEEP");
}

#[test]
fn a_chain_of_seventeen_is_refused_and_sixteen_is_not() {
    // Variables a0..a16 in one collection with one mode; a(i) aliases a(i+1).
    let mut v = Variables {
        collections: vec![Collection {
            id: "c".into(),
            name: "c".into(),
            modes: modes(1),
        }],
        variables: Vec::new(),
    };
    for i in 0..=16 {
        let b = if i == 16 {
            Bind::Lit(Value::Color("#000".into()))
        } else {
            Bind::var(format!("a{}", i + 1))
        };
        v.variables.push(var(
            &format!("a{i}"),
            "c",
            &format!("a{i}"),
            BTreeMap::from([("m0".to_string(), b)]),
        ));
    }
    assert_eq!(
        check_file_with(&[], &v, &Limits::default()),
        Ok(()),
        "16 steps"
    );
    v.variables.push(var(
        "top",
        "c",
        "top",
        BTreeMap::from([("m0".to_string(), Bind::var("a0"))]),
    ));
    let e = check_file_with(&[], &v, &Limits::default()).expect_err("17 steps");
    assert!(format!("{e:?}").contains("AliasTooDeep"), "{e:?}");
}

fn refused(vars: &Variables, c: serde_json::Value) -> &'static str {
    let c: Command = serde_json::from_value(c).unwrap();
    check_command_with(&[], vars, &c, &Limits::default())
        .expect_err("the command must be refused")
        .code()
}

fn collections(n: usize) -> Variables {
    Variables {
        collections: (0..n)
            .map(|i| Collection {
                id: format!("c{i}"),
                name: format!("c{i}"),
                modes: modes(1),
            })
            .collect(),
        variables: Vec::new(),
    }
}

#[test]
fn the_thirty_third_collection_is_refused() {
    let add = |id: &str| json!({"type": "addCollection", "collection": {"id": id, "name": id, "modes": [{"id": "m", "name": "m"}]}});
    assert_eq!(
        check_command_with(
            &[],
            &collections(31),
            &serde_json::from_value(add("x")).unwrap(),
            &Limits::default()
        ),
        Ok(())
    );
    assert_eq!(refused(&collections(32), add("x")), "E_TOO_MANY_ENTRIES");
}

#[test]
fn the_seventeenth_mode_is_refused() {
    let add =
        json!({"type": "addMode", "collection": "c", "mode": {"id": "extra", "name": "extra"}});
    let one = |n| Variables {
        collections: vec![Collection {
            id: "c".into(),
            name: "c".into(),
            modes: modes(n),
        }],
        variables: Vec::new(),
    };
    let c: Command = serde_json::from_value(add.clone()).unwrap();
    assert_eq!(
        check_command_with(&[], &one(15), &c, &Limits::default()),
        Ok(())
    );
    assert_eq!(refused(&one(16), add), "E_TOO_MANY_ENTRIES");
}

fn many_variables(n: usize) -> Variables {
    let mut v = collections(1);
    for i in 0..n {
        v.variables.push(var(
            &format!("v{i}"),
            "c0",
            &format!("n{i}"),
            BTreeMap::from([("m0".to_string(), Bind::Lit(Value::Color("#000".into())))]),
        ));
    }
    v
}

#[test]
fn the_five_thousand_and_first_variable_is_refused() {
    let add = json!({"type": "addVariable", "variable": {"id": "x", "collection": "c0", "name": "x", "type": "color", "values": {"m0": "#000"}}});
    let c: Command = serde_json::from_value(add.clone()).unwrap();
    assert_eq!(
        check_command_with(&[], &many_variables(4_999), &c, &Limits::default()),
        Ok(())
    );
    assert_eq!(refused(&many_variables(5_000), add), "E_TOO_MANY_ENTRIES");
    // A stored file over the ceiling is refused at load too.
    assert!(check_file_with(&[], &many_variables(5_001), &Limits::default()).is_err());
    assert_eq!(
        check_file_with(&[], &many_variables(5_000), &Limits::default()),
        Ok(())
    );
}

#[test]
fn a_name_longer_than_the_ceiling_is_refused() {
    let v = collections(1);
    let name = |n: usize| "a".repeat(n);
    let add = |n: usize| json!({"type": "addVariable", "variable": {"id": "x", "collection": "c0", "name": name(n), "type": "color", "values": {"m0": "#000"}}});
    let c: Command = serde_json::from_value(add(256)).unwrap();
    assert_eq!(check_command_with(&[], &v, &c, &Limits::default()), Ok(()));
    assert_eq!(refused(&v, add(257)), "E_TOO_LONG");
    let col = json!({"type": "addCollection", "collection": {"id": "n", "name": name(257), "modes": [{"id": "m", "name": "m"}]}});
    assert_eq!(refused(&v, col), "E_TOO_LONG");
}
