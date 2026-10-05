//! Versioned semantic developer control mechanism. No game, renderer or interpreter dependency.
pub mod diagnostics;
pub use rustcraft_config as config;
pub mod configuration;
use diagnostics::{DebugInput, Domain};
pub use rustcraft_agent_api::AgentIntent;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

pub const CONTROL_API_VERSION: u32 = 1;
pub type ControlResult<T> = Result<T, String>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Source {
    DeveloperConsole,
    Script,
    Scenario,
    ServerAdmin,
    FutureChat,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub source: Source,
    pub capabilities: BTreeSet<String>,
}
impl Context {
    pub fn read_only(source: Source) -> Self {
        Self {
            source,
            capabilities: [
                "world.read",
                "player.read",
                "entity.read",
                "debug.inspect",
                "config.read",
            ]
            .map(str::to_owned)
            .into(),
        }
    }
    pub fn developer(source: Source) -> Self {
        if source == Source::FutureChat {
            return Self::read_only(source);
        }
        let mut ctx = Self::read_only(source);
        ctx.capabilities.extend(
            [
                "world.write",
                "player.control",
                "debug.capture",
                "debug.pause",
                "debug.configure",
                "script.load",
                "config.write",
                "config.persist",
            ]
            .map(str::to_owned),
        );
        ctx
    }
    pub fn require(&self, capability: &str) -> ControlResult<()> {
        if self.capabilities.contains(capability) {
            Ok(())
        } else {
            Err(format!(
                "capability denied: {capability} (source {:?})",
                self.source
            ))
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Action {
    ConsoleEdit {
        operation: String,
        text: String,
    },
    DebugKey {
        key: String,
        pressed: bool,
        repeat: bool,
    },
    ConfigSet(Vec<(String, String)>),
    ConfigReset(String),
    ConfigResetBatch(Vec<String>),
    ConfigSelect(String),
    ConfigPersist {
        keys: Vec<String>,
        remove: bool,
    },
    Pause,
    Resume,
    Step(u32),
    Teleport([f32; 3]),
    SetBlock {
        position: [i32; 3],
        key: String,
        variant: u16,
    },
    Capture(String),
    DebugPage(String),
    EntityPage(u32),
    DebugUi(DebugInput),
    InspectChunk {
        x: i32,
        z: i32,
        section_y: i32,
    },
    InspectEntity(String),
    Overlay {
        name: String,
        enabled: bool,
    },
}
impl Action {
    pub fn capability(&self) -> &'static str {
        match self {
            Self::DebugKey { .. } | Self::ConsoleEdit { .. } => "debug.configure",
            Self::ConfigSet(_) | Self::ConfigReset(_) | Self::ConfigResetBatch(_) => "config.write",
            Self::ConfigPersist { .. } => "config.persist",
            Self::ConfigSelect(_) => "debug.configure",
            Self::DebugUi(
                DebugInput::SettingIncrease
                | DebugInput::SettingDecrease
                | DebugInput::SettingReset,
            ) => "config.write",
            Self::Pause | Self::Resume | Self::Step(_) => "debug.pause",
            Self::Teleport(_) => "player.control",
            Self::SetBlock { .. } => "world.write",
            Self::Capture(_) => "debug.capture",
            Self::DebugPage(_) | Self::EntityPage(_) | Self::Overlay { .. } | Self::DebugUi(_) => {
                "debug.configure"
            }
            Self::InspectChunk { .. } | Self::InspectEntity(_) => "debug.configure",
        }
    }
    pub fn validate(&self) -> ControlResult<()> {
        match self {
            Self::ConsoleEdit { operation, text } if operation.len() > 32 || text.len() > 256 => {
                Err("bounded console edit exceeded".into())
            }
            Self::DebugKey { key, .. } if key.len() > 32 => Err("debug key name too long".into()),
            Self::ConfigSet(values)
                if values.is_empty()
                    || values.len() > 16
                    || values.iter().any(|(k, v)| k.len() > 128 || v.len() > 256) =>
            {
                Err("configuration batch/key/value bounds exceeded".into())
            }
            Self::ConfigReset(k) | Self::ConfigSelect(k) if k.len() > 128 => {
                Err("setting identity exceeds 128 bytes".into())
            }
            Self::ConfigResetBatch(keys)
                if keys.is_empty() || keys.len() > 16 || keys.iter().any(|k| k.len() > 128) =>
            {
                Err("reset key/count bounds exceeded".into())
            }
            Self::ConfigPersist { keys, .. }
                if keys.is_empty() || keys.len() > 16 || keys.iter().any(|k| k.len() > 128) =>
            {
                Err("persist key/count bounds exceeded".into())
            }
            Self::Step(n) if *n == 0 || *n > 1000 => Err("step count must be 1..1000".into()),
            Self::Teleport(p) if p.iter().any(|v| !v.is_finite() || v.abs() > 30_000_000.) => {
                Err("invalid position".into())
            }
            Self::Capture(name)
                if name.is_empty()
                    || name.len() > 64
                    || !name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') =>
            {
                Err("capture name must be 1..64 letters/digits/_/-".into())
            }
            Self::DebugPage(name) | Self::Overlay { name, .. } if name.len() > 128 => {
                Err("debug identity too long".into())
            }
            Self::InspectEntity(id)
                if id.len() != 32
                    || !id.bytes().all(|b| b.is_ascii_hexdigit())
                    || id.bytes().all(|b| b == b'0') =>
            {
                Err("entity needs a nonzero 32-digit hexadecimal stable EntityId".into())
            }
            Self::InspectChunk { x, z, section_y }
                if x.abs_diff(0) > 1_875_000
                    || z.abs_diff(0) > 1_875_000
                    || section_y.abs_diff(0) > 1_875_000 =>
            {
                Err("inspection coordinate outside bounded world range".into())
            }
            Self::SetBlock { key, .. } if key.len() > 256 || !key.contains(':') => {
                Err("block needs a semantic namespaced key".into())
            }
            _ => Ok(()),
        }
    }
}
/// Bounded immutable values; absent domains are explicitly null. Adapters supply semantic IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub presentation: Value,
    #[serde(default)]
    pub residency: Value,
    #[serde(default)]
    pub config: Value,
    pub version: u32,
    pub tick: u64,
    pub runtime: Value,
    pub player: Value,
    pub world: Value,
    pub streaming: Value,
    pub entities: Value,
    pub lighting: Value,
    pub meshing: Value,
    pub renderer: Value,
    pub persistence: Value,
    pub scripts: Value,
    #[serde(default)]
    pub debug: Value,
    #[serde(default)]
    pub chunk: Value,
    #[serde(default)]
    pub entity: Value,
    #[serde(default)]
    pub overlay_geometry: Value,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            presentation: Value::Null,
            residency: Value::Null,
            config: Value::Null,
            version: CONTROL_API_VERSION,
            tick: 0,
            runtime: Value::Null,
            player: Value::Null,
            world: Value::Null,
            streaming: Value::Null,
            entities: Value::Null,
            lighting: Value::Null,
            meshing: Value::Null,
            renderer: Value::Null,
            persistence: Value::Null,
            scripts: Value::Null,
            debug: Value::Null,
            chunk: Value::Null,
            entity: Value::Null,
            overlay_geometry: Value::Null,
        }
    }
}
/// All adapters dispatch here; capability checks precede authoritative application.
pub trait Host {
    fn script_poll_interval_ms(&self) -> u64 {
        500
    }
    fn diagnostic_due(&self, _domain: Domain) -> bool {
        true
    }
    fn prepare_diagnostics(&mut self, _domains: &[Domain]) {}
    fn publish_diagnostic(&mut self, _domain: Domain, _value: Value, _started: Instant) {}
    fn snapshot(&self) -> Snapshot;
    fn block(&self, position: [i32; 3]) -> ControlResult<String>;
    fn apply(&mut self, action: &Action) -> ControlResult<Value>;
    fn intent(&mut self, intent: AgentIntent) -> ControlResult<()>;
}
pub fn execute(host: &mut impl Host, context: &Context, action: &Action) -> ControlResult<Value> {
    context
        .require(action.capability())
        .map_err(|e| format!("{action:?}: {e}"))?;
    action.validate()?;
    host.apply(action)
}

#[derive(Debug, Clone, Serialize)]
pub struct CommandSpec {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub usage: String,
    pub help: String,
    pub capability: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct CommandResult {
    pub status: String,
    pub message: String,
    pub data: Value,
    pub action: Option<Action>,
}
impl CommandResult {
    pub fn output(message: impl Into<String>, data: Value) -> Self {
        Self {
            status: "ok".into(),
            message: message.into(),
            data,
            action: None,
        }
    }
    pub fn action(action: Action) -> Self {
        Self {
            status: "queued".into(),
            message: format!("{action:?}"),
            data: Value::Null,
            action: Some(action),
        }
    }
}
pub type CommandHandler = fn(&[String], &Snapshot) -> ControlResult<CommandResult>;
type SharedCommandHandler =
    std::sync::Arc<dyn Fn(&[String], &Snapshot) -> ControlResult<CommandResult> + Send + Sync>;
#[derive(Default)]
pub struct Registry {
    entries: BTreeMap<String, (CommandSpec, SharedCommandHandler)>,
    aliases: BTreeMap<String, String>,
    completions: BTreeMap<String, Vec<String>>,
}
impl Registry {
    pub fn register(
        &mut self,
        spec: CommandSpec,
        handler: impl Fn(&[String], &Snapshot) -> ControlResult<CommandResult> + Send + Sync + 'static,
    ) -> ControlResult<()> {
        let names = std::iter::once(&spec.name).chain(&spec.aliases);
        let mut unique = BTreeSet::new();
        for name in names {
            if name.is_empty()
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || self.aliases.contains_key(name)
                || !unique.insert(name)
            {
                return Err(format!("invalid/duplicate command name {name}"));
            }
        }
        if !spec.id.contains(':') || self.entries.values().any(|(s, _)| s.id == spec.id) {
            return Err("invalid/duplicate semantic command ID".into());
        }
        for name in std::iter::once(&spec.name).chain(&spec.aliases) {
            self.aliases.insert(name.clone(), spec.name.clone());
        }
        self.entries
            .insert(spec.name.clone(), (spec, std::sync::Arc::new(handler)));
        Ok(())
    }
    pub fn replace_handler(
        &mut self,
        name: &str,
        handler: impl Fn(&[String], &Snapshot) -> ControlResult<CommandResult> + Send + Sync + 'static,
    ) -> ControlResult<()> {
        let entry = self.entries.get_mut(name).ok_or("unknown command")?;
        entry.1 = std::sync::Arc::new(handler);
        Ok(())
    }
    pub fn specs(&self) -> Vec<CommandSpec> {
        self.entries.values().map(|(s, _)| s.clone()).collect()
    }
    pub fn dispatch(
        &self,
        context: &Context,
        line: &str,
        snapshot: &Snapshot,
    ) -> ControlResult<CommandResult> {
        let args = parse(line.trim_start_matches('/'))?;
        let name = args.first().ok_or("empty command")?;
        let canonical = self
            .aliases
            .get(name)
            .ok_or_else(|| format!("unknown command /{name}; use /help"))?;
        let (spec, handler) = &self.entries[canonical];
        context.require(&spec.capability)?;
        if canonical == "help" || canonical == "commands" {
            let specs = if let Some(name) = args.get(1) {
                let canonical = self
                    .aliases
                    .get(name)
                    .ok_or_else(|| format!("Unknown command: {name}. Use /commands or /help"))?;
                vec![self.entries[canonical].0.clone()]
            } else {
                self.specs()
            };
            return Ok(CommandResult::output(
                specs
                    .iter()
                    .map(|s| {
                        format!(
                            "/{} {}\n{}\nAliases: {} | Capability: {}",
                            s.name,
                            s.usage,
                            s.help,
                            s.aliases.join(", "),
                            s.capability
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
                json!(specs),
            ));
        }
        let result = handler(&args[1..], snapshot)
            .map_err(|e| format!("{e}\nUsage: /{} {}", spec.name, spec.usage))?;
        if let Some(action) = &result.action {
            context
                .require(action.capability())
                .map_err(|e| format!("{action:?}: {e}"))?;
            action.validate()?;
        }
        Ok(result)
    }
    pub fn cache_completion(&mut self, name: &str, values: impl IntoIterator<Item = String>) {
        self.completions.insert(
            name.to_owned(),
            values
                .into_iter()
                .filter(|s| s.len() <= 256)
                .take(1024)
                .collect(),
        );
    }
    pub fn complete(&self, prefix: &str) -> Vec<String> {
        let trimmed = prefix.trim_start_matches('/');
        if let Some((command, tail)) = trimmed.split_once(' ') {
            let word = tail.rsplit_once(' ').map_or(tail, |(_, word)| word);
            let stem = &prefix[..prefix.len() - word.len()];
            return self
                .completions
                .get(command)
                .into_iter()
                .flatten()
                .filter(|s| s.starts_with(word))
                .take(32)
                .map(|s| format!("{stem}{s}"))
                .collect();
        }
        self.aliases
            .keys()
            .filter(|n| n.starts_with(trimmed))
            .take(32)
            .map(|n| format!("/{n}"))
            .collect()
    }
}
/// Small command-line lexer, intentionally not a shell. UTF-8 input bounded to 4 KiB/64 args.
pub fn parse(line: &str) -> ControlResult<Vec<String>> {
    if line.len() > 4096 {
        return Err("command exceeds 4096 bytes".into());
    }
    let mut args = Vec::new();
    let mut token = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut started = false;
    for (offset, c) in line.char_indices() {
        if escaped {
            if !matches!(c, '\\' | '\'' | '"' | ' ') {
                return Err(format!("unsupported escape at byte {offset}"));
            }
            token.push(c);
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            started = true;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else {
                token.push(c);
            }
            continue;
        }
        if c == '"' || c == '\'' {
            quote = Some(c);
            started = true;
        } else if c.is_whitespace() {
            if started {
                args.push(std::mem::take(&mut token));
                started = false;
            }
        } else {
            token.push(c);
            started = true;
        }
        if args.len() > 64 {
            return Err("too many arguments".into());
        }
    }
    if escaped || quote.is_some() {
        return Err("unfinished escape or quoted string".into());
    }
    if started {
        args.push(token);
    }
    if args.len() > 64 {
        return Err("too many arguments".into());
    }
    Ok(args)
}
pub fn engine_registry() -> Registry {
    let mut registry = Registry::default();
    for (name, usage, help, capability) in [
        (
            "help",
            "",
            "Discover commands and capabilities",
            "debug.inspect",
        ),
        ("commands", "", "List command metadata", "debug.inspect"),
        (
            "script",
            "PATH | help",
            "Run approved script or inspect API",
            "script.load",
        ),
        (
            "scenario",
            "PATH | abort",
            "Run or cancel isolated scenario",
            "script.load",
        ),
        (
            "reload",
            "",
            "Compile candidates before atomic swap",
            "script.load",
        ),
        (
            "inspect",
            "",
            "Immutable semantic debug snapshot",
            "debug.inspect",
        ),
        (
            "config",
            "list|get|describe|set|batch|reset|persist|unpersist",
            "Typed effective/requested configuration; mutation is privileged",
            "config.read",
        ),
        ("pause", "", "Pause fixed simulation", "debug.pause"),
        ("resume", "", "Resume fixed simulation", "debug.pause"),
        (
            "step",
            "[1..1000]",
            "Advance exact fixed ticks while paused",
            "debug.pause",
        ),
        (
            "capture",
            "NAME",
            "Capture state and next graphical frame",
            "debug.capture",
        ),
        (
            "debug",
            "page NAME | overlay NAME on/off",
            "Configure diagnostic presentation",
            "debug.configure",
        ),
    ] {
        registry
            .register(
                CommandSpec {
                    id: format!("rustcraft:{name}"),
                    name: name.into(),
                    aliases: vec![],
                    usage: usage.into(),
                    help: help.into(),
                    capability: capability.into(),
                },
                match name {
                    "pause" => pause_command as CommandHandler,
                    "resume" => resume_command,
                    "step" => step_command,
                    "capture" => capture_command,
                    "debug" => debug_command,
                    "config" => configuration::command,
                    "script" | "scenario" | "reload" => service_command,
                    _ => inspect_command,
                },
            )
            .expect("static commands valid");
    }
    registry.cache_completion(
        "config",
        config::settings::engine(true)
            .keys()
            .map(str::to_owned)
            .chain(
                [
                    "list",
                    "get",
                    "describe",
                    "set",
                    "batch",
                    "reset",
                    "persist",
                    "unpersist",
                    "select",
                ]
                .map(str::to_owned),
            ),
    );
    let views = diagnostics::ViewRegistry::engine();
    registry.cache_completion(
        "debug",
        views
            .views(diagnostics::ViewKind::Page)
            .chain(views.views(diagnostics::ViewKind::Overlay))
            .map(|v| v.name.clone()),
    );
    registry
}
fn inspect_command(args: &[String], snapshot: &Snapshot) -> ControlResult<CommandResult> {
    if !args.is_empty() {
        return Err("usage: /inspect".into());
    }
    Ok(CommandResult::output(
        format!("tick {} player {}", snapshot.tick, snapshot.player),
        json!(snapshot),
    ))
}
fn pause_command(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    if !args.is_empty() {
        return Err("usage: /pause".into());
    }
    Ok(CommandResult::action(Action::Pause))
}
fn resume_command(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    if !args.is_empty() {
        return Err("usage: /resume".into());
    }
    Ok(CommandResult::action(Action::Resume))
}
fn step_command(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    if args.len() > 1 {
        return Err("usage: /step [N]".into());
    }
    let n = args.first().map_or(Ok(1), |s| {
        s.parse().map_err(|_| "invalid tick count".to_string())
    })?;
    Ok(CommandResult::action(Action::Step(n)))
}
fn capture_command(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    let [name] = args else {
        return Err("usage: /capture NAME".into());
    };
    Ok(CommandResult::action(Action::Capture(name.clone())))
}
fn debug_command(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    if args.first().is_some_and(|s| s == "console") {
        return Ok(CommandResult::action(Action::ConsoleEdit {
            operation: args.get(1).ok_or("Missing console edit operation")?.clone(),
            text: args.get(2).cloned().unwrap_or_default(),
        }));
    }
    if args.first().is_some_and(|s| s == "key") {
        let key = args.get(1).ok_or("Missing argument: key")?.clone();
        let pressed = match args.get(2).map(String::as_str) {
            Some("down") => true,
            Some("up") => false,
            _ => return Err("key requires down/up".into()),
        };
        return Ok(CommandResult::action(Action::DebugKey {
            key,
            pressed,
            repeat: args.get(3).is_some_and(|s| s == "repeat"),
        }));
    }
    match args {
        [verb, input] if verb == "ui" => Ok(CommandResult::action(Action::DebugUi(
            match input.as_str() {
                "open" => DebugInput::Open,
                "close" => DebugInput::Close,
                "next" => DebugInput::Next,
                "previous" => DebugInput::Previous,
                "tab" => DebugInput::Tab,
                "activate" => DebugInput::Activate,
                "help" => DebugInput::Help,
                "target" => DebugInput::TargetChunk,
                "setting_next" => DebugInput::SettingNext,
                "setting_previous" => DebugInput::SettingPrevious,
                "setting_increase" => DebugInput::SettingIncrease,
                "setting_decrease" => DebugInput::SettingDecrease,
                "setting_reset" => DebugInput::SettingReset,
                "entity" => DebugInput::NextEntity,
                _ => return Err("unknown selector input".into()),
            },
        ))),
        [verb, x, z, y] if verb == "chunk" => {
            let number = |s: &str| {
                s.parse::<i32>()
                    .map_err(|_| "invalid coordinate".to_owned())
            };
            Ok(CommandResult::action(Action::InspectChunk {
                x: number(x)?,
                z: number(z)?,
                section_y: number(y)?,
            }))
        }
        [verb, id] if verb == "entity" => {
            Ok(CommandResult::action(Action::InspectEntity(id.clone())))
        }
        [verb, name, page] if verb == "page" && name == "entities" => Ok(CommandResult::action(
            Action::EntityPage(page.parse().map_err(|_| "invalid entity page")?),
        )),
        [verb, name] if verb == "page" => {
            Ok(CommandResult::action(Action::DebugPage(name.clone())))
        }
        [verb, name, state] if verb == "overlay" && ["on", "off"].contains(&state.as_str()) => {
            Ok(CommandResult::action(Action::Overlay {
                name: name.clone(),
                enabled: state == "on",
            }))
        }
        _ => Err("usage: /debug page NAME | overlay NAME on/off".into()),
    }
}

/// Gate shared by headless and graphical composition. Event/worker/render turns remain active.
#[derive(Debug, Default)]
pub struct FixedControl {
    pub paused: bool,
    pending: u32,
}
impl FixedControl {
    pub fn pause(&mut self) {
        self.paused = true;
        self.pending = 0;
    }
    pub fn resume(&mut self) {
        self.paused = false;
        self.pending = 0;
    }
    pub fn step(&mut self, n: u32) -> ControlResult<()> {
        Action::Step(n).validate()?;
        if !self.paused {
            return Err("pause before stepping".into());
        }
        self.pending = self
            .pending
            .checked_add(n)
            .filter(|v| *v <= 1000)
            .ok_or("pending tick limit")?;
        Ok(())
    }
    pub fn take_ticks(&mut self, normal: u32) -> u32 {
        if self.paused {
            let n = self.pending.min(8);
            self.pending -= n;
            n
        } else {
            normal
        }
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub tick: u64,
    pub kind: String,
    pub message: String,
}
#[derive(Debug, Default)]
pub struct EventRing {
    entries: VecDeque<Event>,
    bytes: usize,
}
impl EventRing {
    pub fn push(&mut self, tick: u64, kind: &str, message: &str) {
        let truncate = |s: &str| s.chars().take(512).collect::<String>();
        let event = Event {
            tick,
            kind: truncate(kind),
            message: truncate(message),
        };
        self.bytes += event.kind.len() + event.message.len();
        self.entries.push_back(event);
        while self.entries.len() > 128 || self.bytes > 32_768 {
            if let Some(e) = self.entries.pop_front() {
                self.bytes -= e.kind.len() + e.message.len();
            }
        }
    }
    pub fn entries(&self) -> &VecDeque<Event> {
        &self.entries
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Predicate {
    TickAtLeast(u64),
    BlockEquals {
        position: [i32; 3],
        key: String,
    },
    PlayerNear {
        position: [f32; 3],
        tolerance: f32,
    },
    DomainIdle(String),
    DebugEquals {
        path: String,
        value: Value,
    },
    ConfigEquals {
        key: String,
        field: String,
        value: Value,
    },
}
impl Predicate {
    fn evaluate(&self, host: &impl Host) -> ControlResult<bool> {
        let snapshot = host.snapshot();
        Ok(match self {
            Self::TickAtLeast(t) => snapshot.tick >= *t,
            Self::BlockEquals { position, key } => host.block(*position)? == *key,
            Self::PlayerNear {
                position,
                tolerance,
            } => snapshot.player["position"].as_array().is_some_and(|p| {
                p.len() == 3
                    && p.iter().zip(position).all(|(v, p)| {
                        v.as_f64()
                            .is_some_and(|v| (v - f64::from(*p)).abs() <= f64::from(*tolerance))
                    })
            }),
            Self::ConfigEquals { key, field, value } => {
                if key.len() > 128 || field.len() > 32 {
                    return Err("bounded setting identity/field required".into());
                }
                snapshot.config["settings"][key][field] == *value
            }
            Self::DebugEquals { path, value } => {
                if path.len() > 128 || path.split('.').count() > 8 {
                    return Err("bounded diagnostic path required".into());
                }
                let observed = path.split('.').fold(&snapshot.debug, |v, k| &v[k]);
                observed == value
            }
            Self::DomainIdle(domain) => match domain.as_str() {
                "residency" => snapshot.residency["idle"].as_bool().unwrap_or(false),
                "streaming" => snapshot.streaming["idle"].as_bool().unwrap_or(false),
                "meshing" => snapshot.meshing["idle"].as_bool().unwrap_or(false),
                _ => return Err("unknown idle domain".into()),
            },
        })
    }
    fn capability(&self) -> &'static str {
        match self {
            Self::ConfigEquals { .. } => "config.read",
            Self::BlockEquals { .. } => "world.read",
            Self::PlayerNear { .. } => "player.read",
            _ => "debug.inspect",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Step {
    Checkpoint,
    AssertTickDelta(u64),
    WaitTickDelta {
        delta: u64,
        timeout_ms: u64,
    },
    AssertPlayerUnchanged,
    Action(Action),
    Command(String),
    WaitTicks(u64),
    WaitFrames(u64),
    Wait {
        predicate: Predicate,
        timeout_ms: u64,
    },
    Assert {
        predicate: Predicate,
        message: String,
    },
    Intent {
        forward: f32,
        strafe: f32,
        ticks: u64,
    },
    Capture(String),
    Console {
        open: bool,
        line: Option<String>,
    },
    Reload(String),
    WaitJob {
        id: JobRef,
        timeout_ms: u64,
    },
}
#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    pub version: u32,
    pub scenario: String,
    pub run_id: String,
    pub status: String,
    pub failed_step: Option<usize>,
    pub assertion: Option<String>,
    pub tick: u64,
    pub elapsed_ms: u128,
    pub capabilities: BTreeSet<String>,
    pub generation: u64,
    pub hash: String,
    pub error: Option<String>,
}
pub struct Scenario {
    pub steps: Vec<Step>,
    pub cursor: usize,
    pub result: RunResult,
    pub context: Context,
    started: Instant,
    step_started: Instant,
    baseline_tick: Option<u64>,
    baseline_frame: u64,
    checkpoint_tick: Option<u64>,
    checkpoint_player: Value,
    pub active: bool,
}
impl Scenario {
    pub fn new(
        id: String,
        steps: Vec<Step>,
        context: Context,
        generation: u64,
        hash: String,
    ) -> ControlResult<Self> {
        if steps.len() > 1024 {
            return Err("scenario exceeds 1024 steps".into());
        }
        let now = Instant::now();
        Ok(Self {
            steps,
            cursor: 0,
            result: RunResult {
                version: CONTROL_API_VERSION,
                scenario: id,
                run_id: format!(
                    "{}-{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                ),
                status: "running".into(),
                failed_step: None,
                assertion: None,
                tick: 0,
                elapsed_ms: 0,
                capabilities: context.capabilities.clone(),
                generation,
                hash,
                error: None,
            },
            context,
            started: now,
            step_started: now,
            baseline_tick: None,
            baseline_frame: 0,
            checkpoint_tick: None,
            checkpoint_player: Value::Null,
            active: true,
        })
    }
    pub fn error(&mut self, error: String, host: &mut impl Host, events: &mut EventRing) {
        self.finish("error", Some(error), host, events);
    }
    pub fn cancel(&mut self, host: &mut impl Host, events: &mut EventRing) {
        self.finish(
            "cancelled",
            Some("cancelled explicitly".into()),
            host,
            events,
        );
    }
    fn finish(
        &mut self,
        status: &str,
        error: Option<String>,
        host: &mut impl Host,
        events: &mut EventRing,
    ) {
        self.active = false;
        self.result.status = status.into();
        self.result.error = error.map(|e| e.chars().take(4096).collect());
        self.result.failed_step = (status != "pass").then_some(self.cursor);
        self.result.tick = host.snapshot().tick;
        self.result.elapsed_ms = self.started.elapsed().as_millis();
        if let Err(error) = host.intent(AgentIntent::default()) {
            self.result.error.get_or_insert(error);
        }
        events.push(self.result.tick, "scenario", status);
    }
    /// Exactly one step per event turn. Waits poll state and never sleep or interpret code.
    pub fn advance(
        &mut self,
        host: &mut impl Host,
        registry: &Registry,
        frame: u64,
        graphical: bool,
        events: &mut EventRing,
    ) {
        if !self.active {
            return;
        }
        let snapshot = host.snapshot();
        if self.started.elapsed() > Duration::from_secs(120)
            || self.step_started.elapsed() > Duration::from_secs(30)
        {
            self.finish(
                "timeout",
                Some("scenario/run safety deadline".into()),
                host,
                events,
            );
            return;
        }
        let Some(step) = self.steps.get(self.cursor).cloned() else {
            self.finish("pass", None, host, events);
            return;
        };
        if self.baseline_tick.is_none() {
            self.baseline_tick = Some(snapshot.tick);
            self.baseline_frame = frame;
            events.push(snapshot.tick, "step", &format!("{} {step:?}", self.cursor));
        }
        let outcome: ControlResult<bool> = (|| {
            Ok(match step {
                Step::Console { .. } => return Err("console needs tooling adapter".into()),
                Step::Reload(_) => return Err("reload needs tooling job adapter".into()),
                Step::WaitJob { id, timeout_ms } => {
                    if self.step_started.elapsed()
                        > Duration::from_millis(timeout_ms.clamp(1, 30000))
                    {
                        return Err(format!("job {} wait timeout", id.0));
                    }
                    let status = snapshot.scripts["jobs"][id.0.to_string()]["status"].clone();
                    match serde_json::from_value::<JobStatus>(status) {
                        Ok(JobStatus::Pending) => false,
                        Ok(JobStatus::Complete(_)) => true,
                        Ok(JobStatus::Failed(e)) => {
                            return Err(format!("job {} failed: {e}", id.0));
                        }
                        Ok(JobStatus::Cancelled) => return Err(format!("job {} cancelled", id.0)),
                        Err(_) => return Err(format!("job {} unavailable", id.0)),
                    }
                }
                Step::Checkpoint => {
                    self.context.require("player.read")?;
                    self.checkpoint_tick = Some(snapshot.tick);
                    self.checkpoint_player = snapshot.player["position"].clone();
                    true
                }
                Step::AssertTickDelta(expected) => {
                    let tick = self.checkpoint_tick.ok_or("checkpoint required")?;
                    let actual = snapshot.tick.saturating_sub(tick);
                    if actual != expected {
                        return Err(format!(
                            "tick delta assertion: expected {expected}, actual {actual}"
                        ));
                    }
                    true
                }
                Step::WaitTickDelta { delta, timeout_ms } => {
                    let tick = self.checkpoint_tick.ok_or("checkpoint required")?;
                    if self.step_started.elapsed()
                        > Duration::from_millis(timeout_ms.clamp(1, 30000))
                    {
                        return Err(format!(
                            "tick delta wait timeout: expected {delta}, actual {}",
                            snapshot.tick.saturating_sub(tick)
                        ));
                    }
                    snapshot.tick.saturating_sub(tick) >= delta
                }
                Step::AssertPlayerUnchanged => {
                    self.context.require("player.read")?;
                    if self.checkpoint_player.is_null()
                        || self.checkpoint_player != snapshot.player["position"]
                    {
                        return Err(format!(
                            "player assertion: expected {}, actual {}",
                            self.checkpoint_player, snapshot.player["position"]
                        ));
                    }
                    true
                }
                Step::Action(action) => {
                    execute(host, &self.context, &action)?;
                    true
                }
                Step::Command(line) => {
                    let r = registry.dispatch(&self.context, &line, &snapshot)?;
                    if let Some(a) = r.action {
                        execute(host, &self.context, &a)?;
                    }
                    true
                }
                Step::WaitTicks(n) => {
                    snapshot.tick.saturating_sub(self.baseline_tick.unwrap()) >= n
                }
                Step::WaitFrames(n) => {
                    if !graphical {
                        return Err("frame waits require graphics".into());
                    }
                    frame.saturating_sub(self.baseline_frame) >= n
                }
                Step::Wait {
                    predicate,
                    timeout_ms,
                } => {
                    self.context.require(predicate.capability())?;
                    if self.step_started.elapsed()
                        > Duration::from_millis(timeout_ms.clamp(1, 30_000))
                    {
                        return Err(format!("wait timeout: {predicate:?}"));
                    }
                    predicate.evaluate(host)?
                }
                Step::Assert { predicate, message } => {
                    self.context.require(predicate.capability())?;
                    if !predicate.evaluate(host)? {
                        self.result.assertion = Some(message.clone());
                        return Err(format!(
                            "assertion: {message}; expected {predicate:?}; actual snapshot {}",
                            serde_json::to_string(&snapshot).unwrap_or_default()
                        ));
                    }
                    true
                }
                Step::Intent {
                    forward,
                    strafe,
                    ticks,
                } => {
                    self.context.require("player.control")?;
                    if !forward.is_finite()
                        || !strafe.is_finite()
                        || forward.abs() > 1.
                        || strafe.abs() > 1.
                    {
                        return Err("invalid move intent".into());
                    }
                    let done = snapshot.tick.saturating_sub(self.baseline_tick.unwrap()) >= ticks;
                    host.intent(if done {
                        AgentIntent::default()
                    } else {
                        AgentIntent {
                            movement: rustcraft_agent_api::MoveIntent { forward, strafe },
                            ..Default::default()
                        }
                    })?;
                    done
                }
                Step::Capture(name) => {
                    execute(host, &self.context, &Action::Capture(name))?;
                    true
                }
            })
        })();
        match outcome {
            Ok(true) => {
                self.cursor += 1;
                self.baseline_tick = None;
                self.step_started = Instant::now();
            }
            Ok(false) => {}
            Err(e) => {
                let status = if e.contains("timeout") {
                    "timeout"
                } else {
                    "fail"
                };
                self.finish(status, Some(e), host, events);
            }
        }
    }
}

/// Composition-owned transient state. A scenario lease overrides human/legacy drivers per tick.
#[derive(Default)]
pub struct ControlState {
    pub config: config::Registry,
    pub script_poll_ms: u64,
    pub selected_setting: String,
    pub config_status: String,
    pub diagnostics: diagnostics::Diagnostics,
    pub selector: diagnostics::Selector,
    pub selected_chunk: Option<[i32; 3]>,
    pub selected_entity: Option<String>,
    pub fixed: FixedControl,
    pub leased: bool,
    pub intent: AgentIntent,
    pub page: String,
    pub entity_offset: usize,
    pub overlays: BTreeSet<String>,
    pub captures: VecDeque<String>,
    pub domains: Snapshot,
}
impl ControlState {
    pub fn tooling_action(&mut self, action: &Action) -> Option<ControlResult<Value>> {
        if matches!(action, Action::Teleport(_) | Action::SetBlock { .. }) {
            return None;
        }
        if matches!(
            action,
            Action::ConfigSet(_)
                | Action::ConfigReset(_)
                | Action::ConfigResetBatch(_)
                | Action::ConfigSelect(_)
                | Action::ConfigPersist { .. }
        ) {
            return Some(self.configuration_action(action));
        }
        Some((|| {
            match action {
                Action::Pause => self.fixed.pause(),
                Action::Resume => self.fixed.resume(),
                Action::Step(n) => self.fixed.step(*n)?,
                Action::DebugPage(_)
                | Action::Overlay { .. }
                | Action::DebugUi(_)
                | Action::InspectChunk { .. }
                | Action::InspectEntity(_) => self.view_action(action)?,
                Action::EntityPage(page) => {
                    self.page = "entities".into();
                    self.entity_offset = (*page as usize).min(7) * 8;
                }
                Action::Capture(name) => {
                    if self.captures.len() >= 8 {
                        return Err("capture queue full".into());
                    }
                    self.captures.push_back(name.clone());
                }
                _ => return Err("not a tooling action".into()),
            }
            Ok(Value::Null)
        })())
    }
}

/// Opaque job identity and bounded status store for cooperative host work (including capture).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct JobRef(pub u64);
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum JobStatus {
    Pending,
    Complete(Value),
    Failed(String),
    Cancelled,
}
#[derive(Default)]
pub struct Jobs {
    next: u64,
    jobs: BTreeMap<JobRef, JobStatus>,
    owners: BTreeMap<JobRef, String>,
    times: BTreeMap<JobRef, (u128, Option<u128>)>,
}
impl Jobs {
    pub fn create(&mut self) -> ControlResult<JobRef> {
        self.prune();
        if self.jobs.len() >= 128 {
            return Err("job capacity reached".into());
        }
        self.next += 1;
        let id = JobRef(self.next);
        self.jobs.insert(id, JobStatus::Pending);
        self.times.insert(
            id,
            (
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                None,
            ),
        );
        Ok(id)
    }
    pub fn create_owned(&mut self, owner: &str) -> ControlResult<JobRef> {
        let id = self.create()?;
        self.owners.insert(id, owner.chars().take(128).collect());
        Ok(id)
    }
    fn prune(&mut self) {
        let terminal = self
            .jobs
            .iter()
            .filter(|(_, s)| **s != JobStatus::Pending)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in terminal.iter().take(terminal.len().saturating_sub(32)) {
            self.jobs.remove(id);
            self.owners.remove(id);
            self.times.remove(id);
        }
    }
    pub fn cancel_owner(&mut self, owner: &str) {
        let ids = self
            .owners
            .iter()
            .filter(|(_, o)| o.as_str() == owner)
            .map(|(id, _)| *id)
            .collect::<Vec<_>>();
        for id in ids {
            if self.jobs.get(&id) == Some(&JobStatus::Pending) {
                let _ = self.cancel(id);
            }
        }
    }
    pub fn snapshot(&self) -> Value {
        Value::Object(
            self.jobs
                .iter()
                .map(|(id, status)| {
                    (
                        id.0.to_string(),
                        json!({"owner":self.owners.get(id),"status":status,"created_completed_ms":self.times.get(id)}),
                    )
                })
                .collect(),
        )
    }
    pub fn status(&self, id: JobRef) -> ControlResult<&JobStatus> {
        self.jobs.get(&id).ok_or("unknown job".into())
    }
    pub fn finish(&mut self, id: JobRef, status: JobStatus) -> ControlResult<()> {
        let value = self.jobs.get_mut(&id).ok_or("unknown job")?;
        if *value != JobStatus::Pending {
            return Err("job already terminal".into());
        }
        *value = match status {
            JobStatus::Failed(e) => JobStatus::Failed(e.chars().take(1024).collect()),
            other => other,
        };
        if let Some(times) = self.times.get_mut(&id) {
            times.1 = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
            );
        }
        Ok(())
    }
    pub fn cancel(&mut self, id: JobRef) -> ControlResult<()> {
        self.finish(id, JobStatus::Cancelled)
    }
}

fn service_command(args: &[String], _: &Snapshot) -> ControlResult<CommandResult> {
    Ok(CommandResult::output("tooling request", json!(args)))
}

#[cfg(test)]
mod scenario_tests {
    use super::*;
    #[derive(Default)]
    struct TestHost {
        tick: u64,
        intent: AgentIntent,
    }
    impl Host for TestHost {
        fn snapshot(&self) -> Snapshot {
            Snapshot {
                tick: self.tick,
                ..Default::default()
            }
        }
        fn block(&self, _: [i32; 3]) -> ControlResult<String> {
            Ok("sample:air".into())
        }
        fn apply(&mut self, _: &Action) -> ControlResult<Value> {
            Ok(Value::Null)
        }
        fn intent(&mut self, i: AgentIntent) -> ControlResult<()> {
            self.intent = i;
            Ok(())
        }
    }
    #[test]
    fn state_waits_and_cancel_release_intent() {
        let mut h = TestHost::default();
        let mut e = EventRing::default();
        let r = engine_registry();
        let mut s = Scenario::new(
            "test".into(),
            vec![
                Step::Wait {
                    predicate: Predicate::TickAtLeast(2),
                    timeout_ms: 1000,
                },
                Step::Intent {
                    forward: 1.,
                    strafe: 0.,
                    ticks: 10,
                },
            ],
            Context::developer(Source::Scenario),
            1,
            "hash".into(),
        )
        .unwrap();
        s.advance(&mut h, &r, 0, false, &mut e);
        assert_eq!(s.cursor, 0);
        h.tick = 2;
        s.advance(&mut h, &r, 0, false, &mut e);
        assert_eq!(s.cursor, 1);
        s.advance(&mut h, &r, 0, false, &mut e);
        assert_eq!(h.intent.movement.forward, 1.);
        s.cancel(&mut h, &mut e);
        assert_eq!(h.intent, AgentIntent::default());
        assert_eq!(s.result.status, "cancelled");
    }
    #[test]
    fn assertion_result_contains_failed_step() {
        let mut h = TestHost::default();
        let mut e = EventRing::default();
        let mut s = Scenario::new(
            "fail".into(),
            vec![Step::Assert {
                predicate: Predicate::TickAtLeast(5),
                message: "expected five".into(),
            }],
            Context::developer(Source::Scenario),
            1,
            "hash".into(),
        )
        .unwrap();
        s.advance(&mut h, &engine_registry(), 0, false, &mut e);
        assert_eq!(s.result.failed_step, Some(0));
        assert_eq!(s.result.assertion.as_deref(), Some("expected five"));
        assert_eq!(s.result.status, "fail");
    }
    #[test]
    fn checkpoint_detects_excess_ticks() {
        let mut h = TestHost::default();
        let mut e = EventRing::default();
        let r = engine_registry();
        let mut s = Scenario::new(
            "exact-step".into(),
            vec![Step::Checkpoint, Step::AssertTickDelta(1)],
            Context::developer(Source::Scenario),
            1,
            "hash".into(),
        )
        .unwrap();
        s.advance(&mut h, &r, 0, false, &mut e);
        h.tick = 2;
        s.advance(&mut h, &r, 0, false, &mut e);
        assert_eq!(s.result.status, "fail");
        assert_eq!(s.result.failed_step, Some(1));
        assert!(s.result.error.as_deref().unwrap().contains("actual 2"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lexer() {
        assert_eq!(
            parse(r#"foo "hello world" 'x' "a\"b" """#).unwrap(),
            ["foo", "hello world", "x", "a\"b", ""]
        );
        assert!(parse("x \"").is_err());
        assert!(parse("x \\").is_err());
    }
    #[test]
    fn fixed_steps() {
        let mut f = FixedControl::default();
        assert_eq!(f.take_ticks(2), 2);
        f.pause();
        assert_eq!(f.take_ticks(3), 0);
        f.step(1).unwrap();
        assert_eq!(f.take_ticks(0), 1);
        f.step(17).unwrap();
        assert_eq!(f.take_ticks(0) + f.take_ticks(0) + f.take_ticks(0), 17);
        f.resume();
        assert_eq!(f.take_ticks(2), 2);
    }
    #[test]
    fn ring_is_bounded() {
        let mut ring = EventRing::default();
        for n in 0..1000 {
            ring.push(n, "test", &"x".repeat(1000));
        }
        assert!(ring.entries.len() <= 128);
        assert!(ring.bytes <= 32768);
    }
    #[test]
    fn future_chat_has_no_developer_power() {
        let c = Context::read_only(Source::FutureChat);
        assert!(c.require("world.read").is_ok());
        for action in [
            Action::InspectChunk {
                x: 0,
                z: 0,
                section_y: 0,
            },
            Action::InspectEntity("00000000000000000000000000000001".into()),
            Action::DebugUi(DebugInput::Open),
        ] {
            assert!(c.require(action.capability()).is_err());
        }
        for cap in [
            "world.write",
            "player.control",
            "persistence.admin",
            "debug.pause",
        ] {
            assert!(c.require(cap).unwrap_err().contains(cap));
        }
    }
}
