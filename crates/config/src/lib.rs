//! Composition-owned typed operational policy. No game, graphics, interpreter or global singleton.
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub mod settings;
pub type Result<T> = std::result::Result<T, String>;
pub const MAX_SETTINGS: usize = 64;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Value {
    Bool(bool),
    Integer(i64),
    Float(f64),
    Text(String),
    DurationMs(u64),
    Bytes(u64),
}
impl Value {
    pub fn plain(&self) -> Json {
        match self {
            Self::Bool(v) => json!(v),
            Self::Integer(v) => json!(v),
            Self::Float(v) => json!(v),
            Self::Text(v) => json!(v),
            Self::DurationMs(v) | Self::Bytes(v) => json!(v),
        }
    }
    pub fn integer(&self) -> i64 {
        match self {
            Self::Integer(v) => *v,
            Self::DurationMs(v) | Self::Bytes(v) => *v as i64,
            _ => panic!("typed config consumer mismatch"),
        }
    }
    pub fn boolean(&self) -> bool {
        if let Self::Bool(v) = self {
            *v
        } else {
            panic!("typed config consumer mismatch")
        }
    }
    pub fn float(&self) -> f64 {
        if let Self::Float(v) = self {
            *v
        } else {
            panic!("typed config consumer mismatch")
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Kind {
    Bool,
    Integer { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    Text { max: usize },
    Enum { choices: Vec<String> },
    DurationMs { min: u64, max: u64 },
    Bytes { min: u64, max: u64 },
}
impl Kind {
    pub fn parse(&self, text: &str) -> Result<Value> {
        let e = || format!("invalid {self:?} value {text:?}");
        let v = match self {
            Self::Bool => Value::Bool(match text {
                "true" | "on" | "1" => true,
                "false" | "off" | "0" => false,
                _ => return Err(e()),
            }),
            Self::Integer { .. } => Value::Integer(text.parse().map_err(|_| e())?),
            Self::Float { .. } => Value::Float(text.parse().map_err(|_| e())?),
            Self::Text { .. } | Self::Enum { .. } => Value::Text(text.to_owned()),
            Self::DurationMs { .. } => Value::DurationMs(text.parse().map_err(|_| e())?),
            Self::Bytes { .. } => Value::Bytes(text.parse().map_err(|_| e())?),
        };
        self.validate(&v)?;
        Ok(v)
    }
    pub fn validate(&self, v: &Value) -> Result<()> {
        let valid = match (self, v) {
            (Self::Bool, Value::Bool(_)) => true,
            (Self::Integer { min, max }, Value::Integer(v)) => (*min..=*max).contains(v),
            (Self::Float { min, max }, Value::Float(v)) => {
                v.is_finite() && (*min..=*max).contains(v)
            }
            (Self::Text { max }, Value::Text(v)) => {
                v.len() <= *max && !v.chars().any(char::is_control)
            }
            (Self::Enum { choices }, Value::Text(v)) => choices.contains(v),
            (Self::DurationMs { min, max }, Value::DurationMs(v))
            | (Self::Bytes { min, max }, Value::Bytes(v)) => (*min..=*max).contains(v),
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(format!("type/range error: {v:?} requires {self:?}"))
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Policy {
    Immediate,
    NextTick,
    NextFrame,
    Reconfigure,
    RestartRequired,
    ImmutableAfterOpen,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Source {
    Default,
    UserFile,
    Environment,
    Cli,
    Runtime,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub key: String,
    pub kind: Kind,
    pub default: Value,
    pub owner: String,
    pub description: String,
    pub unit: String,
    pub policy: Policy,
    pub persist: bool,
    pub reason: String,
    pub availability: String,
}
#[derive(Debug, Clone)]
struct Entry {
    spec: Spec,
    layers: BTreeMap<Source, Value>,
    effective: Value,
    effective_source: Source,
}
impl Entry {
    fn winner(&self) -> (Source, Value) {
        self.layers
            .last_key_value()
            .map(|(s, v)| (*s, v.clone()))
            .unwrap_or((Source::Default, self.spec.default.clone()))
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Change {
    pub key: String,
    pub previous: Value,
    pub requested: Value,
    pub effective: Value,
    pub source: Source,
    pub policy: Policy,
    pub result: String,
    pub boundary: u64,
    pub error: Option<String>,
}
#[derive(Debug, Clone)]
struct Pending {
    entries: BTreeMap<String, Entry>,
    policy: Policy,
    keys: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Registry {
    entries: BTreeMap<String, Entry>,
    pending: Option<Pending>,
    pub history: VecDeque<Change>,
    pub path: Option<PathBuf>,
    opened: bool,
    snapshot: Json,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
            pending: None,
            history: VecDeque::new(),
            path: None,
            opened: false,
            snapshot: json!({}),
        }
    }
}
impl Registry {
    pub fn register(&mut self, spec: Spec) -> Result<()> {
        if self.opened || self.entries.len() >= MAX_SETTINGS || self.entries.contains_key(&spec.key)
        {
            return Err("registration closed/capacity/duplicate semantic key".into());
        }
        if !semantic(&spec.key)
            || !semantic(&spec.owner)
            || spec.description.len() > 256
            || spec.reason.len() > 256
            || spec.unit.len() > 32
            || spec.availability.len() > 128
        {
            return Err("invalid bounded setting metadata".into());
        }
        match &spec.kind {
            Kind::Enum { choices }
                if choices.is_empty()
                    || choices.len() > 16
                    || choices.iter().any(|v| v.len() > 128) =>
            {
                return Err("enum metadata exceeds bounds".into());
            }
            Kind::Text { max } if *max > 256 => return Err("text setting exceeds 256 bytes".into()),
            _ => {}
        }
        spec.kind.validate(&spec.default)?;
        let key = spec.key.clone();
        let effective = spec.default.clone();
        self.entries.insert(
            key,
            Entry {
                spec,
                layers: BTreeMap::new(),
                effective,
                effective_source: Source::Default,
            },
        );
        self.refresh();
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
    pub fn spec(&self, key: &str) -> Result<&Spec> {
        self.entries
            .get(key)
            .map(|e| &e.spec)
            .ok_or_else(|| format!("unknown setting {key}"))
    }
    pub fn effective(&self, key: &str) -> &Value {
        &self
            .entries
            .get(key)
            .expect("registered native setting")
            .effective
    }
    pub fn snapshot(&self) -> Json {
        self.snapshot.clone()
    }
    pub fn has_pending(&self, policy: Policy) -> bool {
        self.pending.as_ref().is_some_and(|p| p.policy == policy)
    }
    pub fn seed(&mut self, source: Source, values: &[(String, Value)]) -> Result<()> {
        if self.opened || source == Source::Runtime || source == Source::Default {
            return Err("source loading is startup-only".into());
        }
        let mut entries = self.entries.clone();
        for (k, v) in values {
            let e = entries
                .get_mut(k)
                .ok_or_else(|| format!("unknown setting {k} from {source:?}"))?;
            e.spec
                .kind
                .validate(v)
                .map_err(|err| format!("{k} from {source:?}: {err}"))?;
            e.layers.insert(source, v.clone());
        }
        validate_pair(&entries)?;
        self.entries = entries;
        self.refresh();
        Ok(())
    }
    pub fn open(&mut self) {
        for e in self.entries.values_mut() {
            let (s, v) = e.winner();
            e.effective = v;
            e.effective_source = s;
        }
        self.opened = true;
        self.refresh();
    }
    /// One outstanding atomic batch; mixed application boundaries are rejected, never partially applied.
    pub fn request(&mut self, changes: &[(String, Option<Value>)]) -> Result<Policy> {
        let result = self.request_inner(changes);
        if let Err(error) = &result {
            for (k, v) in changes.iter().take(16) {
                if let Some(e) = self.entries.get(k) {
                    let requested = match v {
                        Some(Value::Text(s)) => Value::Text(s.chars().take(256).collect()),
                        Some(v) => v.clone(),
                        None => e.spec.default.clone(),
                    };
                    self.history.push_back(Change {
                        key: k.clone(),
                        previous: e.effective.clone(),
                        requested,
                        effective: e.effective.clone(),
                        source: Source::Runtime,
                        policy: e.spec.policy,
                        result: "rejected".into(),
                        boundary: 0,
                        error: Some(error.chars().take(256).collect()),
                    });
                }
            }
            while self.history.len() > 128 {
                self.history.pop_front();
            }
            self.refresh();
        }
        result
    }
    fn request_inner(&mut self, changes: &[(String, Option<Value>)]) -> Result<Policy> {
        if !self.opened {
            return Err("configuration is not open".into());
        }
        if self.pending.is_some() {
            return Err("configuration transaction pending; wait for its boundary".into());
        }
        if changes.is_empty() || changes.len() > 16 {
            return Err("batch needs 1..16 settings".into());
        }
        let mut entries = self.entries.clone();
        let mut policy = None;
        let mut keys = Vec::new();
        for (k, v) in changes {
            if keys.contains(k) {
                return Err("duplicate key in transaction".into());
            }
            let e = entries
                .get_mut(k)
                .ok_or_else(|| format!("unknown setting {k}"))?;
            if e.spec.availability.starts_with("unavailable") {
                return Err(format!("{k}: {}", e.spec.availability));
            }
            if e.spec.policy == Policy::ImmutableAfterOpen {
                return Err(format!("{k} immutable after open: {}", e.spec.reason));
            }
            if let Some(p) = policy
                && p != e.spec.policy
            {
                return Err("atomic batch must share one change policy".into());
            }
            policy = Some(e.spec.policy);
            if let Some(v) = v {
                e.spec
                    .kind
                    .validate(v)
                    .map_err(|err| format!("{k}: {err}"))?;
                e.layers.insert(Source::Runtime, v.clone());
            } else {
                e.layers.remove(&Source::Runtime);
            }
            keys.push(k.clone());
        }
        validate_pair(&entries)?;
        let policy = policy.unwrap();
        if policy == Policy::RestartRequired {
            self.entries = entries;
            self.record(&keys, "restart required", 0, None, None);
            self.refresh();
        } else {
            self.pending = Some(Pending {
                entries,
                policy,
                keys,
            });
            self.refresh();
        }
        Ok(policy)
    }
    /// Adapter must prepare atomically and leave its old state unchanged on error. Effective publication
    /// follows its acknowledgement; no I/O/worker rebuilding is smuggled into native hot consumers.
    pub fn apply(
        &mut self,
        policy: Policy,
        boundary: u64,
        prepare: impl FnOnce(&Self) -> Result<()>,
    ) -> Result<bool> {
        if !self.has_pending(policy) {
            return Ok(false);
        }
        let p = self.pending.take().unwrap();
        let mut candidate = self.clone();
        candidate.entries = p.entries;
        for key in &p.keys {
            let e = candidate.entries.get_mut(key).unwrap();
            let (s, v) = e.winner();
            e.effective = v;
            e.effective_source = s;
        }
        candidate.refresh();
        match prepare(&candidate) {
            Ok(()) => {
                let previous = std::mem::replace(&mut self.entries, candidate.entries);
                self.record(&p.keys, "applied", boundary, None, Some(&previous));
                self.refresh();
                Ok(true)
            }
            Err(e) => {
                self.record_failed(&p.keys, &candidate, boundary, &e);
                self.refresh();
                Err(e)
            }
        }
    }
    fn record(
        &mut self,
        keys: &[String],
        result: &str,
        boundary: u64,
        error: Option<String>,
        previous: Option<&BTreeMap<String, Entry>>,
    ) {
        for k in keys {
            let e = &self.entries[k];
            let (s, v) = e.winner();
            self.history.push_back(Change {
                key: k.clone(),
                previous: previous
                    .map_or_else(|| e.effective.clone(), |old| old[k].effective.clone()),
                requested: v,
                effective: e.effective.clone(),
                source: s,
                policy: e.spec.policy,
                result: result.into(),
                boundary,
                error: error.clone(),
            });
        }
        while self.history.len() > 128 {
            self.history.pop_front();
        }
    }
    fn record_failed(&mut self, keys: &[String], candidate: &Self, boundary: u64, error: &str) {
        for k in keys {
            let e = &self.entries[k];
            let (s, v) = candidate.entries[k].winner();
            self.history.push_back(Change {
                key: k.clone(),
                previous: e.effective.clone(),
                requested: v,
                effective: e.effective.clone(),
                source: s,
                policy: e.spec.policy,
                result: "failed".into(),
                boundary,
                error: Some(error.chars().take(256).collect()),
            });
        }
        while self.history.len() > 128 {
            self.history.pop_front();
        }
    }
    fn refresh(&mut self) {
        let mut entries = serde_json::Map::new();
        for (k, e) in &self.entries {
            let requested_entry = self
                .pending
                .as_ref()
                .and_then(|p| p.entries.get(k))
                .unwrap_or(e);
            let (s, v) = requested_entry.winner();
            entries.insert(k.clone(),json!({"key":k,"type":e.spec.kind,"default":e.spec.default.plain(),"requested":v.plain(),"effective":e.effective.plain(),"source":e.effective_source,"requested_source":s,"owner":e.spec.owner,"description":e.spec.description,"unit":e.spec.unit,"policy":e.spec.policy,"persist":e.spec.persist,"reason":e.spec.reason,"availability":e.spec.availability,"mutable":e.spec.policy!=Policy::ImmutableAfterOpen,"layers":requested_entry.layers,"pending":v!=e.effective||self.pending.as_ref().is_some_and(|p|p.keys.contains(k))}));
        }
        self.snapshot = json!({"settings":entries,"count":self.len(),"history":self.history,"path":self.path.as_ref().map(|p|p.display().to_string())});
    }
    pub fn load_file(&mut self, path: PathBuf) -> Result<()> {
        self.path = Some(path.clone());
        if !path.exists() {
            self.refresh();
            return Ok(());
        }
        let file = read_file(&path)?;
        self.seed(
            Source::UserFile,
            &file.values.into_iter().collect::<Vec<_>>(),
        )?;
        Ok(())
    }
    /// Explicit save of current successful values (or restart request). Runtime requests never auto-save.
    pub fn persist(&mut self, key: &str, remove: bool) -> Result<()> {
        self.persist_many(&[key.to_owned()], remove)
    }
    pub fn persist_many(&mut self, keys: &[String], remove: bool) -> Result<()> {
        if self.pending.is_some() {
            return Err("wait for application before persisting".into());
        }
        if keys.is_empty() || keys.len() > 16 {
            return Err("persist needs 1..16 keys".into());
        }
        let path = self.path.as_ref().ok_or("no user config path configured")?;
        let mut file = if path.exists() {
            read_file(path)?
        } else {
            UserFile::default()
        };
        for key in keys {
            let e = self
                .entries
                .get(key)
                .ok_or_else(|| format!("unknown setting {key}"))?;
            if !e.spec.persist {
                return Err(format!("{key} cannot persist: {}", e.spec.reason));
            }
            if remove {
                file.values.remove(key);
            } else {
                file.values.insert(key.clone(), e.winner().1);
            }
        }
        let mut next_launch = self.clone();
        next_launch.opened = false;
        next_launch.pending = None;
        for e in next_launch.entries.values_mut() {
            e.layers.clear();
        }
        next_launch.seed(
            Source::UserFile,
            &file
                .values
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect::<Vec<_>>(),
        )?;
        write_file(path, &file)?; // Explicit next-launch file edit; startup provenance remains the current session's source.
        Ok(())
    }
}
fn semantic(s: &str) -> bool {
    s.len() <= 128
        && s.split_once(':').is_some_and(|(n, p)| {
            !n.is_empty()
                && !p.is_empty()
                && n.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_/-".contains(&b))
        })
}
fn validate_pair(entries: &BTreeMap<String, Entry>) -> Result<()> {
    use settings::{LOAD_RADIUS, RETAIN_RADIUS};
    if let (Some(a), Some(b)) = (entries.get(LOAD_RADIUS), entries.get(RETAIN_RADIUS))
        && a.winner().1.integer() > b.winner().1.integer()
    {
        return Err("retain radius must be >= load radius (use atomic /config batch)".into());
    }
    Ok(())
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserFile {
    version: u32,
    values: BTreeMap<String, Value>,
}
impl Default for UserFile {
    fn default() -> Self {
        Self {
            version: 1,
            values: BTreeMap::new(),
        }
    }
}
fn read_file(path: &Path) -> Result<UserFile> {
    let m = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if m.len() > 65536 {
        return Err("user configuration exceeds 64 KiB".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err(format!(
            "{}: user configuration exceeds 64 KiB",
            path.display()
        ));
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Raw {
        version: u32,
        values: BTreeMap<String, Json>,
    }
    let raw: Raw =
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut values = BTreeMap::new();
    for (k, v) in raw.values {
        values.insert(
            k.clone(),
            serde_json::from_value(v).map_err(|e| format!("{} key {k}: {e}", path.display()))?,
        );
    }
    let file = UserFile {
        version: raw.version,
        values,
    };
    if file.version != 1 || file.values.len() > MAX_SETTINGS {
        return Err("unsupported configuration version/count".into());
    }
    Ok(file)
}
fn write_file(path: &Path, file: &UserFile) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let bytes = serde_json::to_vec_pretty(file).map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("configuration exceeds 64 KiB".into());
    }
    atomicwrites::AtomicFile::new(path, atomicwrites::AllowOverwrite)
        .write(|f| {
            f.write_all(&bytes)?;
            f.sync_all()
        })
        .map_err(|e| format!("atomic config write {}: {e}", path.display()))
}
pub fn user_path() -> PathBuf {
    if let Some(p) = std::env::var_os("RUSTCRAFT_CONFIG_FILE") {
        return PathBuf::from(p);
    }
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
    };
    base.unwrap_or_else(|| PathBuf::from("."))
        .join("rustcraft")
        .join("config-v1.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use settings::*;
    fn engine() -> Registry {
        let mut r = settings::engine(true);
        r.open();
        r
    }
    fn set(r: &mut Registry, key: &str, text: &str) -> Result<Policy> {
        let v = r.spec(key)?.kind.parse(text)?;
        r.request(&[(key.into(), Some(v))])
    }
    #[test]
    fn validators_and_atomic_pair_preserve_effective() {
        let mut r = engine();
        for text in ["2", "13", "nope", "3.5"] {
            assert!(set(&mut r, LOAD_RADIUS, text).is_err());
            assert_eq!(r.effective(LOAD_RADIUS).integer(), 4);
        }
        assert!(set(&mut r, LOAD_RADIUS, "6").is_err());
        assert!(set(&mut r, "missing:key", "1").is_err());
        assert!(set(&mut r, PRESENT, "Immediate").is_err());
        assert!(set(&mut r, "rustcraft:identity/chunk_width", "16").is_err());
        for (load, retain) in [(3, 3), (12, 16), (4, 5)] {
            r.request(&[
                (LOAD_RADIUS.into(), Some(Value::Integer(load))),
                (RETAIN_RADIUS.into(), Some(Value::Integer(retain))),
            ])
            .unwrap();
            assert_ne!(r.snapshot()["settings"][LOAD_RADIUS]["pending"], false);
            r.apply(Policy::NextTick, 10, |_| Ok(())).unwrap();
            assert_eq!(r.effective(LOAD_RADIUS).integer(), load);
            assert_eq!(r.effective(RETAIN_RADIUS).integer(), retain);
        }
        assert_eq!(r.history.back().unwrap().previous, Value::Integer(16));
        for (kind, v) in [
            (Kind::Bool, Value::Integer(1)),
            (Kind::Float { min: 0., max: 1. }, Value::Float(f64::NAN)),
            (Kind::Bytes { min: 1, max: 2 }, Value::Bytes(3)),
            (
                Kind::DurationMs { min: 50, max: 5000 },
                Value::DurationMs(49),
            ),
            (Kind::Text { max: 2 }, Value::Text("abc".into())),
        ] {
            assert!(kind.validate(&v).is_err());
        }
    }
    #[test]
    fn precedence_reset_and_restart_dont_leak_into_other_apply() {
        let mut r = settings::engine(true);
        for (s, n) in [
            (Source::UserFile, 6),
            (Source::Environment, 8),
            (Source::Cli, 10),
        ] {
            r.seed(
                s,
                &[
                    (LOAD_RADIUS.into(), Value::Integer(n)),
                    (RETAIN_RADIUS.into(), Value::Integer(16)),
                ],
            )
            .unwrap();
        }
        r.open();
        assert_eq!(r.effective(LOAD_RADIUS).integer(), 10);
        set(&mut r, LOAD_RADIUS, "12").unwrap();
        assert_eq!(r.effective(LOAD_RADIUS).integer(), 10);
        r.apply(Policy::NextTick, 1, |_| Ok(())).unwrap();
        assert_eq!(r.effective(LOAD_RADIUS).integer(), 12);
        r.request(&[(LOAD_RADIUS.into(), None)]).unwrap();
        r.apply(Policy::NextTick, 2, |_| Ok(())).unwrap();
        assert_eq!(r.effective(LOAD_RADIUS).integer(), 10);
        let workers = r.effective(MESH_WORKERS).clone();
        set(&mut r, MESH_WORKERS, "32").unwrap();
        assert_eq!(r.effective(MESH_WORKERS), &workers);
        set(&mut r, DIAGNOSTIC_MS, "50").unwrap();
        r.apply(Policy::Immediate, 0, |_| Ok(())).unwrap();
        assert_eq!(r.effective(MESH_WORKERS), &workers);
        assert_eq!(r.snapshot()["settings"][LOAD_RADIUS]["source"], "Cli");
    }
    #[test]
    fn all_boundaries_and_reconfigure_prepare_failure_rollback() {
        let mut r = Registry::default();
        for (name, policy) in [
            ("immediate", Policy::Immediate),
            ("tick", Policy::NextTick),
            ("frame", Policy::NextFrame),
            ("pool", Policy::Reconfigure),
        ] {
            r.register(Spec {
                key: format!("test:{name}"),
                kind: Kind::Integer { min: 1, max: 4 },
                default: Value::Integer(1),
                owner: "test:package".into(),
                description: "test provider".into(),
                unit: "".into(),
                policy,
                persist: true,
                reason: "".into(),
                availability: "available".into(),
            })
            .unwrap();
        }
        r.open();
        for (name, p) in [
            ("immediate", Policy::Immediate),
            ("tick", Policy::NextTick),
            ("frame", Policy::NextFrame),
        ] {
            let k = format!("test:{name}");
            set(&mut r, &k, "4").unwrap();
            assert_eq!(r.effective(&k).integer(), 1);
            assert!(!r.apply(Policy::Reconfigure, 0, |_| Ok(())).unwrap());
            r.apply(p, 1, |_| Ok(())).unwrap();
            assert_eq!(r.effective(&k).integer(), 4);
        }
        let native = 1;
        set(&mut r, "test:pool", "2").unwrap();
        assert!(
            r.apply(Policy::Reconfigure, 2, |_| Err(
                "prepare failed; old pool intact".into()
            ))
            .is_err()
        );
        assert_eq!(native, 1);
        assert_eq!(r.effective("test:pool").integer(), 1);
        assert_eq!(r.snapshot()["settings"]["test:pool"]["requested"], 1);
        assert_eq!(r.history.back().unwrap().result, "failed");
        set(&mut r, "test:pool", "3").unwrap();
        let mut native = 1;
        r.apply(Policy::Reconfigure, 3, |c| {
            native = c.effective("test:pool").integer();
            Ok(())
        })
        .unwrap();
        assert_eq!(native, 3);
    }
    #[test]
    fn user_file_write_reopen_override_errors_and_atomic_failure() {
        let root = std::env::temp_dir().join(format!(
            "rustcraft-c1-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("settings.json");
        let mut r = settings::engine(true);
        r.load_file(path.clone()).unwrap();
        r.open();
        set(&mut r, DIAGNOSTIC_MS, "100").unwrap();
        r.apply(Policy::Immediate, 0, |_| Ok(())).unwrap();
        assert!(!path.exists());
        r.persist(DIAGNOSTIC_MS, false).unwrap();
        let mut reopened = settings::engine(true);
        settings::load(
            &mut reopened,
            &["--set-config".into(), format!("{DIAGNOSTIC_MS}=500")],
            |_| None,
            path.clone(),
        )
        .unwrap();
        assert_eq!(reopened.effective(DIAGNOSTIC_MS).integer(), 500);
        assert_eq!(
            reopened.snapshot()["settings"][DIAGNOSTIC_MS]["layers"]["UserFile"]["value"],
            100
        );
        let bytes = std::fs::read(&path).unwrap();
        let failed: std::result::Result<(), atomicwrites::Error<std::io::Error>> =
            atomicwrites::AtomicFile::new(&path, atomicwrites::AllowOverwrite).write(|f| {
                f.write_all(b"broken")?;
                Err(std::io::Error::other("forced interruption"))
            });
        assert!(failed.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        r.persist(DIAGNOSTIC_MS, true).unwrap();
        let mut removed = settings::engine(true);
        removed.load_file(path.clone()).unwrap();
        removed.open();
        assert_eq!(removed.effective(DIAGNOSTIC_MS).integer(), 250);
        std::fs::write(
            &path,
            br#"{"version":1,"values":{"unknown:key":{"type":"Bool","value":true}}}"#,
        )
        .unwrap();
        assert!(
            settings::engine(true)
                .load_file(path.clone())
                .unwrap_err()
                .contains("unknown:key")
        );
        std::fs::write(&path, b"bad json").unwrap();
        assert!(settings::engine(true).load_file(path).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn startup_aliases_cli_pair_and_registration_bounds() {
        let path = std::env::temp_dir().join("rustcraft-c1-absent-input.json");
        let mut r = settings::engine(true);
        settings::load(
            &mut r,
            &[],
            |k| {
                if k == "RUSTCRAFT_STREAM_RADIUS" {
                    Some("6".into())
                } else {
                    None
                }
            },
            path,
        )
        .unwrap();
        assert_eq!(r.effective(RETAIN_RADIUS).integer(), 7);
        assert_eq!(
            r.snapshot()["settings"][LOAD_RADIUS]["source"],
            "Environment"
        );
        assert!(r.register(r.spec(LOAD_RADIUS).unwrap().clone()).is_err());
        let mut r = settings::engine(true);
        let spec = r.spec(LOAD_RADIUS).unwrap().clone();
        assert!(r.register(spec).is_err());
        r.open();
        assert!(r.seed(Source::Cli, &[]).is_err());
    }
    #[test]
    fn bounded_history_and_control_boundary_cost() {
        let mut r = engine();
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            std::hint::black_box(r.has_pending(Policy::NextTick));
        }
        let inactive = start.elapsed();
        let start = std::time::Instant::now();
        for _ in 0..10000 {
            std::hint::black_box(r.effective(LIGHT_WORK));
        }
        let read = start.elapsed();
        let start = std::time::Instant::now();
        for n in 0..200 {
            set(
                &mut r,
                DIAGNOSTIC_MS,
                if n % 2 == 0 { "100" } else { "250" },
            )
            .unwrap();
            r.apply(Policy::Immediate, n, |_| Ok(())).unwrap();
        }
        assert_eq!(r.history.len(), 128);
        println!(
            "C1_COST inactive_10000={inactive:?} lookup_10000={read:?} request+apply_200={:?}",
            start.elapsed()
        );
    }
}
