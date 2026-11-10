//! Translations loaded from `lang/*.yaml` (German only).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

pub const BUILTIN: &[(&str, &str)] = &[
    ("de", include_str!("../lang/de.yaml")),
];

type Table = HashMap<String, String>;

struct State {
    code: String,
    table: Table,
}

static STATE: RwLock<Option<Arc<State>>> = RwLock::new(None);
static OVERRIDE_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

fn flatten(prefix: &str, value: &serde_yaml::Value, out: &mut Table) {
    let join = |k: &str| if prefix.is_empty() { k.to_string() } else { format!("{}.{}", prefix, k) };
    match value {
        serde_yaml::Value::Mapping(map) => {
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s.clone(),
                    other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
                };
                flatten(&join(&key), v, out);
            }
        }
        serde_yaml::Value::Sequence(seq) => {
            for (i, v) in seq.iter().enumerate() {
                flatten(&join(&i.to_string()), v, out);
            }
        }
        serde_yaml::Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        serde_yaml::Value::Number(n) => {
            out.insert(prefix.to_string(), n.to_string());
        }
        serde_yaml::Value::Bool(b) => {
            out.insert(prefix.to_string(), b.to_string());
        }
        _ => {}
    }
}

pub fn parse(src: &str) -> Result<Table, serde_yaml::Error> {
    let value: serde_yaml::Value = serde_yaml::from_str(src)?;
    let mut table = Table::new();
    flatten("", &value, &mut table);
    Ok(table)
}

fn load_table(code: &str) -> Table {
    let mut table = BUILTIN
        .iter()
        .find(|(c, _)| *c == code)
        .or_else(|| BUILTIN.first())
        .and_then(|(_, src)| parse(src).ok())
        .unwrap_or_default();

    let dir = OVERRIDE_DIR.read().ok().and_then(|d| d.clone());
    if let Some(path) = dir.map(|d| d.join(format!("{}.yaml", code))) {
        if let Ok(src) = std::fs::read_to_string(&path) {
            match parse(&src) {
                Ok(extra) => {
                    tracing::info!("Loaded language overrides from {:?}", path);
                    table.extend(extra);
                }
                Err(e) => tracing::warn!("Ignoring invalid language file {:?}: {}", path, e),
            }
        }
    }
    table
}

pub fn set_override_dir(dir: Option<&Path>) {
    if let Ok(mut d) = OVERRIDE_DIR.write() {
        *d = dir.map(Path::to_path_buf);
    }
}

pub fn set_language(_code: &str) {
    let state = State { table: load_table("de"), code: "de".to_string() };
    if let Ok(mut s) = STATE.write() {
        *s = Some(Arc::new(state));
    }
}

fn current() -> Option<Arc<State>> {
    STATE.read().ok().and_then(|s| s.clone())
}

pub fn language() -> String {
    "de".to_string()
}

pub fn t(key: &str) -> String {
    if current().is_none() {
        set_language("de");
    }
    let state = current();
    state
        .as_ref()
        .and_then(|s| s.table.get(key))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

pub fn try_t(key: &str) -> Option<String> {
    if current().is_none() {
        set_language("de");
    }
    current().and_then(|s| s.table.get(key).cloned())
}

pub fn tf(key: &str, vars: &[(&str, &str)]) -> String {
    let mut s = t(key);
    for (k, v) in vars {
        s = s.replace(&format!("{{{}}}", k), v);
    }
    s
}

pub fn nth(key: &str, idx: usize) -> String {
    t(&format!("{}.{}", key, idx))
}

pub fn num(val: f64, decimals: usize) -> String {
    let dec = t("meta.decimal");
    let dec = if dec == "meta.decimal" { "," } else { &dec };
    if decimals == 0 {
        format!("{:.0}", val)
    } else {
        format!("{:.1$}", val, decimals).replace('.', dec)
    }
}

pub fn font_for(_code: &str) -> Option<String> {
    None
}
