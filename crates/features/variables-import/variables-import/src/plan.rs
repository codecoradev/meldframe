//! From the custom properties a scan kept to the commands of an import: names, ids, modes, the types of aliases, and the
//! order in which the commands can be applied. Every custom property ends imported or skipped with a reason; none is
//! dropped without a line in the report.

use crate::log::{Log, conversions, kill, notes, section_name};
use crate::names::{variable_id, variable_name};
use crate::scan::{Decl, Scan, Section};
use crate::text::short;
use crate::value::{self, Val};
use crate::{EntryKind, Import, Report};
use meldframe_check::Limits;
use meldframe_protocol::{Bind, Collection, Command, Mode, Value, VarType, Variable};
use meldframe_variables::MAX_ALIAS_DEPTH;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub(crate) struct Slot {
    pub val: Val,
    pub text: String,
}

pub(crate) struct Prop {
    pub raw: String,
    /// `raw` cut for a report, once: a report line names a property many times and must not cut a long name each time.
    pub label: String,
    pub section: Section,
    pub light: Option<Slot>,
    pub dark: Option<Slot>,
    pub dark_source: u32,
    pub name: String,
    pub id: String,
    pub dead: bool,
}

pub(crate) fn slots(p: &Prop) -> impl Iterator<Item = &Slot> {
    p.light.iter().chain(p.dark.iter())
}

fn add_decls(scan: &Scan, log: &mut Log) -> Vec<Prop> {
    let mut props: Vec<Prop> = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for d in &scan.decls {
        let i = match index.get(d.name.as_str()) {
            Some(&i) if props[i].section != d.section => {
                log.skipped += 1;
                log.push(
                    EntryKind::Skipped,
                    "duplicate_name",
                    &d.name,
                    format!(
                        "also declared in {}; the first declaration is kept",
                        section_name(props[i].section)
                    ),
                );
                continue;
            }
            Some(&i) => i,
            None => {
                props.push(Prop {
                    raw: d.name.clone(),
                    label: short(&d.name),
                    section: d.section,
                    light: None,
                    dark: None,
                    dark_source: 0,
                    name: String::new(),
                    id: String::new(),
                    dead: false,
                });
                index.insert(&d.name, props.len() - 1);
                props.len() - 1
            }
        };
        set_slots(&mut props[i], d, log);
    }
    props
}

fn set_slots(p: &mut Prop, d: &Decl, log: &mut Log) {
    let slot = || Slot {
        val: value::classify(d.name.trim_start_matches('-'), &d.value),
        text: d.value.clone(),
    };
    for (on, target) in [(d.light, &mut p.light), (d.dark, &mut p.dark)] {
        if on {
            if target.is_some() {
                log.push(
                    EntryKind::Note,
                    "overridden",
                    &d.name,
                    "declared again in the same mode; the last declaration is used",
                );
            }
            *target = Some(slot());
        }
    }
    if d.dark {
        p.dark_source = d.source;
    }
}

/// Everything that decides a custom property on its own, before aliases are looked at.
fn check_alone(props: &mut [Prop], limits: &Limits, log: &mut Log) {
    let mut taken: HashMap<(Section, String), String> = HashMap::new(); // the label of the owner
    let mut ids: HashSet<String> = HashSet::new();
    let mut alive = 0usize;
    for p in props.iter_mut() {
        let skip = slots(p).find_map(|s| match &s.val {
            Val::Skip(code, msg) => Some((*code, msg.clone())),
            Val::Number { value, .. } if value.abs() > limits.max_size => Some((
                "out_of_range",
                format!("{value} is beyond the limit of {}", limits.max_size),
            )),
            _ => None,
        });
        if let Some((code, msg)) = skip {
            kill(p, log, code, msg);
            continue;
        }
        if p.light.is_none() {
            kill(
                p,
                log,
                "dark_without_default",
                "only a dark rule sets it, and a variable needs a value in the default mode too",
            );
            continue;
        }
        let Some(name) = variable_name(&p.raw, p.section) else {
            kill(
                p,
                log,
                "invalid_name",
                "no letter or digit is left in the name",
            );
            continue;
        };
        if name.chars().count() > limits.max_name {
            kill(
                p,
                log,
                "name_too_long",
                format!("the name is over {} characters", limits.max_name),
            );
            continue;
        }
        if let Some(other) = taken.get(&(p.section, name.clone())) {
            let m = format!("the name {name} is already taken by {other}");
            kill(p, log, "name_collision", m);
            continue;
        }
        if alive >= limits.max_variables {
            let m = format!("a file holds at most {} variables", limits.max_variables);
            kill(p, log, "limit_variables", m);
            continue;
        }
        alive += 1;
        taken.insert((p.section, name.clone()), p.label.clone());
        let coll = section_name(p.section);
        p.id = variable_id(coll, &name, &mut ids, limits.max_id);
        if name.replace('/', "-") != p.raw.trim_start_matches('-') {
            log.push(
                EntryKind::Converted,
                "renamed",
                &p.raw,
                format!("the variable is named {name}"),
            );
        }
        p.name = name;
    }
}

/// The ids of the type and the depth of each variable, in the order the commands can be applied.
struct Resolved {
    ty: Vec<Option<VarType>>,
    order: Vec<usize>,
}

fn target_of(s: &Slot) -> Option<&str> {
    match &s.val {
        Val::Alias { target, .. } => Some(target),
        _ => None,
    }
}

/// Look every alias up, then walk the variables so that each comes after what it points at; find the types, refuse
/// a chain that is too long or a cycle, and a variable that points at something that was not imported.
fn resolve(props: &mut [Prop], log: &mut Log) -> Resolved {
    let n = props.len();
    let index: HashMap<String, usize> = props
        .iter()
        .enumerate()
        .map(|(i, p)| (p.raw.clone(), i))
        .collect();
    // Every alias edge, to targets that exist; a variable that points at nothing is killed, and so is everything that
    // points at a variable that was killed, whatever the order of the text.
    let mut deps: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut rev: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut dead: Vec<usize> = Vec::new();
    for i in 0..n {
        if props[i].dead {
            dead.push(i);
            continue;
        }
        let mut missing: Option<String> = None;
        for t in slots(&props[i]).filter_map(target_of) {
            match index.get(t) {
                Some(&j) => {
                    if !deps[i].contains(&j) {
                        deps[i].push(j);
                        rev[j].push(i);
                    }
                }
                None => missing = Some(short(t)),
            }
        }
        if let Some(t) = missing {
            let m = format!("it points at {t}, which was not imported");
            kill(&mut props[i], log, "alias_target_missing", m);
            dead.push(i);
        }
    }
    while let Some(d) = dead.pop() {
        for &i in &rev[d] {
            if !props[i].dead {
                let m = format!(
                    "it points at {}, which was not imported",
                    short(&props[d].raw)
                );
                kill(&mut props[i], log, "alias_target_missing", m);
                dead.push(i);
            }
        }
    }
    let mut left = vec![0usize; n];
    for i in 0..n {
        if !props[i].dead {
            left[i] = deps[i].len();
        }
    }
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| !props[i].dead && left[i] == 0).collect();
    let mut ty: Vec<Option<VarType>> = vec![None; n];
    let mut depth = vec![0usize; n];
    let mut done = vec![false; n];
    let mut order = Vec::new();
    while let Some(i) = queue.pop_front() {
        done[i] = true;
        if !props[i].dead {
            settle(props, i, &index, &mut ty, &mut depth, log);
            if !props[i].dead {
                order.push(i);
            }
        }
        for &d in &rev[i] {
            if props[d].dead {
                continue;
            }
            left[d] -= 1;
            if left[d] == 0 {
                queue.push_back(d);
            }
        }
    }
    for i in 0..n {
        if !props[i].dead && !done[i] {
            let m = "it is part of an alias cycle, or points at one".to_string();
            kill(&mut props[i], log, "alias_cycle", m);
        }
    }
    Resolved { ty, order }
}

/// The type and the depth of variable `i`, whose targets are settled; or the reason it is refused.
fn settle(
    props: &mut [Prop],
    i: usize,
    index: &HashMap<String, usize>,
    ty: &mut [Option<VarType>],
    depth: &mut [usize],
    log: &mut Log,
) {
    let mut found: Option<VarType> = None;
    let mut d = 0usize;
    let mut bad: Option<(&'static str, String)> = None;
    for s in slots(&props[i]) {
        let t = match &s.val {
            Val::Color { .. } => Some(VarType::Color),
            Val::Number { .. } => Some(VarType::Number),
            Val::Alias { target, .. } => {
                let j = index[target.as_str()];
                if props[j].dead {
                    bad = Some((
                        "alias_target_missing",
                        format!("it points at {target}, which was not imported"),
                    ));
                    None
                } else {
                    d = d.max(depth[j] + 1);
                    ty[j]
                }
            }
            Val::Skip(..) => None,
        };
        match (found, t) {
            (Some(a), Some(b)) if a != b => {
                bad = Some((
                    "type_mismatch",
                    "its values are not all colours or all numbers".into(),
                ));
            }
            (None, Some(b)) => found = Some(b),
            _ => {}
        }
    }
    if bad.is_none() && d > MAX_ALIAS_DEPTH {
        bad = Some((
            "alias_too_deep",
            format!("an alias chain is at most {MAX_ALIAS_DEPTH} steps long"),
        ));
    }
    if let Some((code, msg)) = bad {
        kill(&mut props[i], log, code, msg);
        return;
    }
    ty[i] = found;
    depth[i] = d;
}

fn bind(v: &Val, props: &[Prop], index: &HashMap<&str, usize>) -> Bind<Value> {
    match v {
        Val::Color { hex, .. } => Bind::Lit(Value::Color(hex.clone())),
        Val::Number { value, .. } => Bind::Lit(Value::Number(*value)),
        Val::Alias { target, .. } => Bind::var(props[index[target.as_str()]].id.clone()),
        Val::Skip(..) => unreachable!("a skipped value does not reach the commands"),
    }
}

pub(crate) fn build(scan: &Scan, limits: &Limits) -> Import {
    let mut log = Log::default();
    for l in &scan.left {
        log.push(EntryKind::Skipped, l.code, &l.what, &l.message);
    }
    let mut props = add_decls(scan, &mut log);
    check_alone(&mut props, limits, &mut log);
    let r = resolve(&mut props, &mut log);
    conversions(&props, &mut log);

    let alive = |s: Section| props.iter().any(|p| !p.dead && p.section == s);
    let dark = props
        .iter()
        .any(|p| !p.dead && p.section == Section::Root && p.dark.is_some());
    let mut sections: Vec<Section> = Vec::new();
    for p in props.iter().filter(|p| !p.dead) {
        if !sections.contains(&p.section) {
            sections.push(p.section);
        }
    }
    debug_assert!(sections.iter().all(|s| alive(*s)));
    let mode = |id: &str| Mode {
        id: id.into(),
        name: id.into(),
    };
    let mut commands = Vec::new();
    let mut report = Report::default();
    for s in &sections {
        let modes = match s {
            Section::Theme => vec![mode("default")],
            Section::Root if dark => vec![mode("light"), mode("dark")],
            Section::Root => vec![mode("light")],
        };
        report.modes += modes.len();
        report.collections += 1;
        commands.push(Command::AddCollection {
            collection: Collection {
                id: format!("col-{}", section_name(*s)),
                name: section_name(*s).into(),
                modes,
            },
        });
    }
    let index: HashMap<&str, usize> = props
        .iter()
        .enumerate()
        .map(|(i, p)| (p.raw.as_str(), i))
        .collect();
    let mut inherited = 0usize;
    for &i in &r.order {
        let p = &props[i];
        let (Some(light), kind) = (&p.light, r.ty[i]) else {
            continue;
        };
        let Some(kind) = kind else { continue };
        let mut values = BTreeMap::new();
        match p.section {
            Section::Theme => {
                values.insert("default".to_string(), bind(&light.val, &props, &index));
            }
            Section::Root => {
                values.insert("light".to_string(), bind(&light.val, &props, &index));
                if dark {
                    let from = p.dark.as_ref().unwrap_or_else(|| {
                        inherited += 1;
                        light
                    });
                    values.insert("dark".to_string(), bind(&from.val, &props, &index));
                }
            }
        }
        if slots(p).any(|s| matches!(s.val, Val::Alias { .. })) {
            report.aliases += 1;
        }
        match kind {
            VarType::Color => report.colors += 1,
            VarType::Number => report.numbers += 1,
        }
        report.variables += 1;
        commands.push(Command::AddVariable {
            variable: Variable {
                id: p.id.clone(),
                collection: format!("col-{}", section_name(p.section)),
                name: p.name.clone(),
                kind,
                values,
            },
        });
    }
    log.omitted += scan.left_omitted;
    notes(scan, &props, inherited, &mut log);
    report.skipped = log.skipped;
    report.entries = log.entries;
    report.omitted = log.omitted;
    Import { commands, report }
}
