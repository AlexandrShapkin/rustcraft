//! Rhai leaf adapter. Scripts see immutable snapshots and bounded semantic command queues.
mod worker;
use rhai::{AST, Dynamic, Engine, EvalAltResult, Scope, module_resolvers::DummyModuleResolver};
use rustcraft_control::{
    Action, CONTROL_API_VERSION, Context, ControlResult, Host, Predicate, Snapshot, Step,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const RHAI_VERSION: &str = "1.26.1";
pub const API_HELP: &str = "console automation: console_open(BOOL), console_line(LINE); DUX1: debug(), chunk_inspection(), entity_inspection(), assert_debug(PATH, BOOL/STRING/INT); /debug ui open/close/next/previous/tab/activate/help/target/entity; /debug chunk X Z SECTION_Y; /debug entity HEX_ID; control_version(), tick(), player_position(), player(), world(), block_at(X,Y,Z), streaming(), entities(), entity(STABLE_ID), persistence(), renderer(), scripts(), lighting(), meshing(), has_capability(ID), command(LINE), pause(), resume(), step(N), teleport(X,Y,Z), set_block(X,Y,Z,SEMANTIC_KEY), capture(NAME), debug_page(NAME), overlay(NAME,BOOL); scenario: reload_script(PATH) requests and cooperatively waits for a compile job; checkpoint(), assert_tick_delta(N), wait_tick_delta(N,MS), assert_player_unchanged(), wait_ticks(N), wait_frames(N), wait_tick(T,MS), assert_tick(T), assert_block(X,Y,Z,KEY), move_player(FORWARD,STRAFE,TICKS); bounded assert_true(BOOL), assert_eq(INT,INT), fail(MESSAGE). No filesystem, network, process, sleep or imports.";
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub session: String,
    pub source: String,
    pub generation: u64,
    pub compile_us: u128,
    pub execution_us: u128,
    pub operations: u64,
    pub result: Option<String>,
    pub error: Option<String>,
    pub output: Vec<String>,
}
#[derive(Clone)]
pub struct Limits {
    pub operations: u64,
    pub deadline: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            operations: 50_000,
            deadline: Duration::from_millis(10),
        }
    }
}
#[derive(Default)]
struct Bridge {
    snapshot: Snapshot,
    actions: Vec<Action>,
    steps: Vec<Step>,
    scenario: bool,
    output: Vec<String>,
    deadline: Option<Instant>,
    operations: u64,
    abort_reason: Option<String>,
}
/// One engine per runtime on its owning thread; sync feature makes callback state safe to own.
/// Each session owns its scope. No engine/Scope is a shared mutable global.
pub struct RhaiRuntime {
    engine: Engine,
    bridge: Arc<Mutex<Bridge>>,
    context: Context,
    limits: Limits,
}
pub struct RhaiSession {
    pub scope: Scope<'static>,
    pub diagnostic: Diagnostic,
}
impl RhaiSession {
    pub fn new(id: &str) -> Self {
        Self {
            scope: Scope::new(),
            diagnostic: Diagnostic {
                session: id.into(),
                source: "REPL".into(),
                generation: 0,
                compile_us: 0,
                execution_us: 0,
                operations: 0,
                result: None,
                error: None,
                output: Vec::new(),
            },
        }
    }
    pub fn reset(&mut self) {
        self.scope = Scope::new();
    }
}
fn err(message: impl Into<String>) -> Box<EvalAltResult> {
    message.into().into()
}
impl RhaiRuntime {
    pub fn new(context: Context, limits: Limits) -> Self {
        let limits = Limits {
            operations: limits.operations.clamp(100, 1_000_000),
            deadline: limits
                .deadline
                .clamp(Duration::from_millis(1), Duration::from_millis(50)),
        };
        let mut engine = Engine::new();
        engine.set_max_operations(limits.operations.clamp(100, 1_000_000));
        engine
            .set_max_call_levels(32)
            .set_max_expr_depths(32, 32)
            .set_max_variables(128)
            .set_max_functions(128)
            .set_max_string_size(8192)
            .set_max_array_size(1024)
            .set_max_map_size(128);
        engine.set_module_resolver(DummyModuleResolver::new());
        engine.disable_symbol("eval");
        engine.disable_symbol("import");
        let bridge = Arc::new(Mutex::new(Bridge::default()));
        let progress = bridge.clone();
        engine.on_progress(move |operations| {
            let mut b = progress.lock().unwrap();
            b.operations = operations;
            if b.deadline.is_some_and(|t| Instant::now() >= t) {
                b.abort_reason = Some("execution deadline exceeded".into());
                Some(Dynamic::from("execution deadline exceeded"))
            } else {
                None
            }
        });
        let output = bridge.clone();
        engine.on_print(move |text| {
            let mut b = output.lock().unwrap();
            if b.output.len() < 128 {
                b.output.push(text.chars().take(512).collect());
            }
        });
        let output = bridge.clone();
        engine.on_debug(move |text, _, _| {
            let mut b = output.lock().unwrap();
            if b.output.len() < 128 {
                b.output.push(text.chars().take(512).collect());
            }
        });
        engine.register_fn(
            "assert_true",
            |condition: bool| -> Result<(), Box<EvalAltResult>> {
                if condition {
                    Ok(())
                } else {
                    Err(err("assertion: expected true, actual false"))
                }
            },
        );
        engine.register_fn(
            "assert_eq",
            |actual: i64, expected: i64| -> Result<(), Box<EvalAltResult>> {
                if actual == expected {
                    Ok(())
                } else {
                    Err(err(format!(
                        "assertion: expected {expected}, actual {actual}"
                    )))
                }
            },
        );
        engine.register_fn("fail", |message: &str| -> Result<(), Box<EvalAltResult>> {
            Err(err(format!(
                "failure: {}",
                message.chars().take(512).collect::<String>()
            )))
        });
        engine.register_fn("control_version", || i64::from(CONTROL_API_VERSION));
        engine.register_fn("control_help", || API_HELP.to_owned());
        let caps = context.clone();
        engine.register_fn("has_capability", move |id: &str| {
            caps.capabilities.contains(id)
        });
        let query = bridge.clone();
        let caps = context.clone();
        engine.register_fn("tick", move || -> Result<i64, Box<EvalAltResult>> {
            caps.require("debug.inspect").map_err(err)?;
            Ok(query.lock().unwrap().snapshot.tick as i64)
        });
        let query = bridge.clone();
        let caps = context.clone();
        engine.register_fn(
            "player_position",
            move || -> Result<rhai::Array, Box<EvalAltResult>> {
                caps.require("player.read").map_err(err)?;
                let b = query.lock().unwrap();
                let p = b.snapshot.player["position"]
                    .as_array()
                    .ok_or_else(|| err("player unavailable"))?;
                Ok(p.iter()
                    .map(|v| Dynamic::from_float(v.as_f64().unwrap_or_default()))
                    .collect())
            },
        );
        for (name, domain, cap) in [
            ("player", "player", "player.read"),
            ("world", "world", "world.read"),
            ("streaming", "streaming", "debug.inspect"),
            ("entities", "entities", "entity.read"),
            ("persistence", "persistence", "debug.inspect"),
            ("renderer", "renderer", "debug.inspect"),
            ("scripts", "scripts", "debug.inspect"),
            ("lighting", "lighting", "debug.inspect"),
            ("meshing", "meshing", "debug.inspect"),
            ("debug", "debug", "debug.inspect"),
            ("chunk_inspection", "chunk", "debug.inspect"),
            ("entity_inspection", "entity", "debug.inspect"),
        ] {
            let query = bridge.clone();
            let caps = context.clone();
            engine.register_fn(name, move || -> Result<Dynamic, Box<EvalAltResult>> {
                caps.require(cap).map_err(err)?;
                let b = query.lock().unwrap();
                let value = match domain {
                    "player" => &b.snapshot.player,
                    "world" => &b.snapshot.world,
                    "streaming" => &b.snapshot.streaming,
                    "entities" => &b.snapshot.entities,
                    "persistence" => &b.snapshot.persistence,
                    "renderer" => &b.snapshot.renderer,
                    "lighting" => &b.snapshot.lighting,
                    "meshing" => &b.snapshot.meshing,
                    "debug" => &b.snapshot.debug,
                    "chunk" => &b.snapshot.chunk,
                    "entity" => &b.snapshot.entity,
                    _ => &b.snapshot.scripts,
                };
                Ok(semantic_value(value, 0))
            });
        }
        let query = bridge.clone();
        let caps = context.clone();
        engine.register_fn("block_at", move |x:i64,y:i64,z:i64| -> Result<String, Box<EvalAltResult>> {
            caps.require("world.read").map_err(err)?;
            let p = coords(x,y,z)?;
            query.lock().unwrap().snapshot.world["blocks"][format!("{},{},{}", p[0],p[1],p[2])].as_str().map(str::to_owned).ok_or_else(||err("block unavailable in bounded nearby snapshot; use scenario assert_block for live checks"))
        });
        let query = bridge.clone();
        let caps = context.clone();
        engine.register_fn(
            "entity",
            move |id: &str| -> Result<Dynamic, Box<EvalAltResult>> {
                caps.require("entity.read").map_err(err)?;
                if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(err("entity needs stable 32-digit hex EntityId"));
                }
                let b = query.lock().unwrap();
                Ok(b.snapshot
                    .entities
                    .as_array()
                    .and_then(|a| a.iter().take(64).find(|e| e["id"].as_str() == Some(id)))
                    .map_or(Dynamic::UNIT, |e| semantic_value(e, 0)))
            },
        );
        macro_rules! action_fn { ($name:literal, ($($args:tt)*), $action:expr) => {{ let b=bridge.clone(); let caps=context.clone(); engine.register_fn($name, move |$($args)*| -> Result<(),Box<EvalAltResult>> { let action: Action=$action; caps.require(action.capability()).map_err(|e|err(format!("{}: {e}",$name)))?; action.validate().map_err(err)?; let mut b=b.lock().unwrap(); if b.actions.len()+b.steps.len()>=1024 { return Err(err("command/step queue limit")); } if b.scenario { b.steps.push(Step::Action(action)); } else { b.actions.push(action); } Ok(()) }); }}; }
        action_fn!("pause", (), Action::Pause);
        action_fn!("resume", (), Action::Resume);
        action_fn!("step", (n: i64), Action::Step(u32::try_from(n).map_err(|_| err("invalid tick count"))?));
        action_fn!("teleport", (x:f64,y:f64,z:f64), Action::Teleport([x as f32,y as f32,z as f32]));
        action_fn!("set_block", (x:i64,y:i64,z:i64,key:&str), Action::SetBlock { position: coords(x,y,z)?, key:key.into(),variant:0 });
        action_fn!("capture", (name:&str), Action::Capture(name.into()));
        action_fn!("debug_page", (name:&str), Action::DebugPage(name.into()));
        action_fn!("overlay", (name:&str,enabled:bool), Action::Overlay { name:name.into(),enabled });
        macro_rules! step_fn { ($name:literal, ($($args:tt)*), $step:expr) => {{ let b=bridge.clone(); engine.register_fn($name, move |$($args)*| -> Result<(),Box<EvalAltResult>> { let mut b=b.lock().unwrap(); if !b.scenario { return Err(err("scenario builder function requires scenario session")); } if b.steps.len()>=1024 { return Err(err("scenario step limit")); } b.steps.push($step); Ok(()) }); }}; }
        step_fn!("command", (line:&str), Step::Command(line.into()));
        step_fn!("wait_ticks", (n:i64), Step::WaitTicks(count(n)?));
        step_fn!("wait_frames", (n:i64), Step::WaitFrames(count(n)?));
        step_fn!("wait_tick", (tick:i64,timeout:i64), Step::Wait { predicate:Predicate::TickAtLeast(count(tick)?),timeout_ms:count(timeout)?.clamp(1,30_000) });
        step_fn!("assert_tick", (tick:i64), Step::Assert { predicate:Predicate::TickAtLeast(count(tick)?),message:format!("tick >= {tick}") });
        step_fn!("assert_block", (x:i64,y:i64,z:i64,key:&str), Step::Assert { predicate:Predicate::BlockEquals { position:coords(x,y,z)?,key:key.into() },message:format!("block ({x},{y},{z}) == {key}") });
        step_fn!("move_player", (forward:f64,strafe:f64,ticks:i64), Step::Intent { forward:forward as f32,strafe:strafe as f32,ticks:count(ticks)? });
        step_fn!("wait_idle",(domain:&str,timeout:i64),Step::Wait {predicate:Predicate::DomainIdle(domain.into()),timeout_ms:count(timeout)?.clamp(1,30000)});
        step_fn!("assert_near",(x:f64,y:f64,z:f64,tolerance:f64),Step::Assert {predicate:Predicate::PlayerNear {position:[x as f32,y as f32,z as f32],tolerance:tolerance as f32},message:format!("player near ({x},{y},{z}) tolerance {tolerance}")});
        step_fn!("console_open", (open:bool), Step::Console { open, line:None });
        step_fn!("console_line", (line:&str), Step::Console {open:true,line:Some(line.into())});
        step_fn!("reload_script", (path:&str), Step::Reload(path.into()));
        step_fn!("checkpoint", (), Step::Checkpoint);
        step_fn!("assert_tick_delta",(n:i64),Step::AssertTickDelta(count(n)?));
        step_fn!("wait_tick_delta",(n:i64,timeout:i64),Step::WaitTickDelta {delta:count(n)?,timeout_ms:count(timeout)?.clamp(1,30000)});
        step_fn!("assert_player_unchanged", (), Step::AssertPlayerUnchanged);
        step_fn!("assert_debug", (path:&str, value:bool), Step::Assert {predicate:Predicate::DebugEquals{path:path.into(),value:serde_json::json!(value)},message:format!("debug {path} == {value}")});
        step_fn!("assert_debug", (path:&str, value:&str), Step::Assert {predicate:Predicate::DebugEquals{path:path.into(),value:serde_json::json!(value)},message:format!("debug {path} == {value}")});
        step_fn!("assert_debug", (path:&str, value:i64), Step::Assert {predicate:Predicate::DebugEquals{path:path.into(),value:serde_json::json!(value)},message:format!("debug {path} == {value}")});
        Self {
            engine,
            bridge,
            context,
            limits,
        }
    }
    pub fn compile(&self, source: &str) -> ControlResult<AST> {
        if source.len() > 65_536 {
            return Err("script source exceeds 64 KiB".into());
        }
        self.engine.compile(source).map_err(|e| e.to_string())
    }
    pub fn eval(
        &mut self,
        session: &mut RhaiSession,
        source: &str,
        snapshot: Snapshot,
    ) -> ControlResult<(String, Vec<Action>)> {
        let start = Instant::now();
        let ast = self.compile(source);
        session.diagnostic.compile_us = start.elapsed().as_micros();
        let ast = ast.inspect_err(|e| {
            session.diagnostic.error = Some(e.clone());
        })?;
        self.execute(session, &ast, snapshot, false)
            .map(|(r, a, _)| (r, a))
    }
    pub fn execute(
        &mut self,
        session: &mut RhaiSession,
        ast: &AST,
        snapshot: Snapshot,
        scenario: bool,
    ) -> ControlResult<(String, Vec<Action>, Vec<Step>)> {
        {
            let mut b = self.bridge.lock().unwrap();
            *b = Bridge {
                snapshot,
                scenario,
                deadline: Some(
                    Instant::now()
                        + self
                            .limits
                            .deadline
                            .clamp(Duration::from_millis(1), Duration::from_millis(50)),
                ),
                ..Default::default()
            };
        }
        let started = Instant::now();
        let value = self
            .engine
            .eval_ast_with_scope::<Dynamic>(&mut session.scope, ast);
        session.diagnostic.execution_us = started.elapsed().as_micros();
        let mut b = self.bridge.lock().unwrap();
        session.diagnostic.operations = b.operations;
        session.diagnostic.output = b.output.clone();
        match value {
            Ok(value) => {
                let value = value.to_string();
                session.diagnostic.result = Some(value.clone());
                session.diagnostic.error = None;
                Ok((
                    value,
                    std::mem::take(&mut b.actions),
                    std::mem::take(&mut b.steps),
                ))
            }
            Err(e) => {
                b.actions.clear();
                b.steps.clear();
                let message = format!(
                    "{}: {e}{}",
                    session.diagnostic.source,
                    b.abort_reason
                        .as_ref()
                        .map_or(String::new(), |reason| format!("; {reason}"))
                );
                session.diagnostic.error = Some(message.clone());
                Err(message)
            }
        }
    }
    fn call(
        &mut self,
        session: &mut RhaiSession,
        ast: &AST,
        name: &str,
        args: rhai::Array,
        snapshot: Snapshot,
    ) -> ControlResult<(Dynamic, Vec<Action>)> {
        {
            let mut b = self.bridge.lock().unwrap();
            *b = Bridge {
                snapshot,
                deadline: Some(Instant::now() + self.limits.deadline),
                ..Default::default()
            };
        }
        let start = Instant::now();
        let result = if name == "command_spec" {
            self.engine.call_fn_with_options::<Dynamic>(
                rhai::CallFnOptions::new().eval_ast(false),
                &mut session.scope,
                ast,
                name,
                (),
            )
        } else {
            self.engine.call_fn_with_options::<Dynamic>(
                rhai::CallFnOptions::new().eval_ast(false),
                &mut session.scope,
                ast,
                name,
                (args,),
            )
        };
        session.diagnostic.execution_us = start.elapsed().as_micros();
        let mut b = self.bridge.lock().unwrap();
        session.diagnostic.operations = b.operations;
        session.diagnostic.output = b.output.clone();
        match result {
            Ok(value) => Ok((value, std::mem::take(&mut b.actions))),
            Err(error) => {
                b.actions.clear();
                Err(format!(
                    "{}: {error}{}",
                    session.diagnostic.source,
                    b.abort_reason
                        .as_ref()
                        .map_or(String::new(), |reason| format!("; {reason}"))
                ))
            }
        }
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
}
fn coords(x: i64, y: i64, z: i64) -> Result<[i32; 3], Box<EvalAltResult>> {
    Ok([
        i32::try_from(x).map_err(|_| err("coordinate overflow"))?,
        i32::try_from(y).map_err(|_| err("coordinate overflow"))?,
        i32::try_from(z).map_err(|_| err("coordinate overflow"))?,
    ])
}
fn count(n: i64) -> Result<u64, Box<EvalAltResult>> {
    u64::try_from(n).map_err(|_| err("negative count"))
}

/// Canonical root containment, including symlinks. Files are bounded before reading.
#[derive(Clone)]
pub struct ScriptRoot {
    root: PathBuf,
}
impl ScriptRoot {
    pub fn new(root: impl AsRef<Path>) -> ControlResult<Self> {
        Ok(Self {
            root: root.as_ref().canonicalize().map_err(|e| e.to_string())?,
        })
    }
    pub fn resolve(&self, path: impl AsRef<Path>) -> ControlResult<PathBuf> {
        let p = path.as_ref();
        if p.components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err("script traversal denied".into());
        }
        let candidate = if p.is_absolute() {
            p.to_owned()
        } else {
            self.root.join(p)
        };
        let canonical = candidate.canonicalize().map_err(|e| e.to_string())?;
        if !canonical.starts_with(&self.root) || canonical.extension().is_none_or(|e| e != "rhai") {
            return Err("script path outside approved root or not .rhai".into());
        }
        Ok(canonical)
    }
    pub fn read(&self, path: impl AsRef<Path>) -> ControlResult<(PathBuf, String)> {
        let p = self.resolve(path)?;
        let metadata = p.metadata().map_err(|e| e.to_string())?;
        if !metadata.is_file() {
            return Err("script must be a regular file".into());
        }
        let size = metadata.len();
        if size > 65536 {
            return Err("script exceeds 64 KiB".into());
        }
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(&p)
            .map_err(|e| e.to_string())?
            .take(65537)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        if text.len() > 65536 {
            return Err("script exceeds 64 KiB".into());
        }
        Ok((p, text))
    }
}
pub struct LoadedScript {
    pub domains: Vec<rustcraft_control::diagnostics::Domain>,
    pub path: PathBuf,
    pub ast: AST,
    pub generation: u64,
    pub hash: String,
    pub last_error: Option<String>,
    pub compile_us: u128,
}
impl LoadedScript {
    pub fn load(
        root: &ScriptRoot,
        path: impl AsRef<Path>,
        runtime: &RhaiRuntime,
    ) -> ControlResult<Self> {
        let (path, text) = root.read(path)?;
        let ast = runtime.compile(&text)?;
        Ok(Self {
            path,
            ast,
            generation: 1,
            hash: blake3::hash(text.as_bytes()).to_hex().to_string(),
            last_error: None,
            compile_us: 0,
            domains: rustcraft_control::diagnostics::query_domains(&text),
        })
    }
    /// Compile before swapping. Broken edits retain the last working AST and generation.
    pub fn reload(&mut self, root: &ScriptRoot, runtime: &RhaiRuntime) -> ControlResult<bool> {
        let candidate: ControlResult<
            Option<(AST, String, Vec<rustcraft_control::diagnostics::Domain>)>,
        > = (|| {
            let (_, text) = root.read(&self.path)?;
            let hash = blake3::hash(text.as_bytes()).to_hex().to_string();
            if hash == self.hash {
                return Ok(None);
            }
            let ast = runtime.compile(&text)?;
            Ok(Some((
                ast,
                hash,
                rustcraft_control::diagnostics::query_domains(&text),
            )))
        })();
        match candidate {
            Ok(Some((ast, hash, domains))) => {
                self.ast = ast;
                self.hash = hash;
                self.domains = domains;
                self.generation += 1;
                self.last_error = None;
                Ok(true)
            }
            Ok(None) => Ok(false),
            Err(e) => {
                self.last_error = Some(e.clone());
                Err(e)
            }
        }
    }
}

enum Purpose {
    Reload,
    Scenario { path: String, context: Context },
    Script { context: Context },
    Bundle,
    Capture,
}
#[derive(Debug, Clone)]
pub enum ConsoleInput {
    Toggle,
    Close,
    Insert(String),
    Left,
    Right,
    Backspace,
    Delete,
    Home,
    End,
    HistoryUp,
    HistoryDown,
    Complete,
    ScrollUp,
    ScrollDown,
}
/// Generic console/scenario service shared by client and server composition.
pub struct DevTools {
    pub runtime: RhaiRuntime,
    pub repl: RhaiSession,
    pub registry: rustcraft_control::Registry,
    pub root: ScriptRoot,
    pub loaded: std::collections::BTreeMap<PathBuf, LoadedScript>,
    command_files: std::collections::BTreeMap<String, PathBuf>,
    repl_domains: std::collections::BTreeSet<rustcraft_control::diagnostics::Domain>,
    pub scenario: Option<rustcraft_control::Scenario>,
    pub events: rustcraft_control::EventRing,
    pub output: std::collections::VecDeque<String>,
    pub console_open: bool,
    pub line: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_index: usize,
    pub scroll: usize,
    pub frame: u64,
    pending_actions: std::collections::VecDeque<(Context, Action)>,
    pub bundle: Option<PathBuf>,
    pub finished: bool,
    pub scenario_diagnostic: Option<Diagnostic>,
    last_poll: Instant,
    reload_cursor: usize,
    worker: worker::Worker,
    pub jobs: rustcraft_control::Jobs,
    versions: std::collections::BTreeMap<String, u64>,
    purposes: std::collections::BTreeMap<rustcraft_control::JobRef, Purpose>,
    pub compiling_scenario: bool,
    bundle_job: Option<rustcraft_control::JobRef>,
    bundle_started: Instant,
}
impl DevTools {
    pub fn new(root: &Path, registry: rustcraft_control::Registry) -> ControlResult<Self> {
        let root = ScriptRoot::new(root)?;
        let worker = worker::Worker::new(root.clone());
        let mut tools = Self {
            runtime: RhaiRuntime::new(
                Context::developer(rustcraft_control::Source::DeveloperConsole),
                Limits::default(),
            ),
            repl: RhaiSession::new("console"),
            registry,
            root,
            worker,
            jobs: Default::default(),
            versions: Default::default(),
            purposes: Default::default(),
            compiling_scenario: false,
            bundle_job: None,
            bundle_started: Instant::now(),
            loaded: Default::default(),
            command_files: Default::default(),
            repl_domains: Default::default(),
            scenario: None,
            events: Default::default(),
            output: Default::default(),
            console_open: false,
            line: String::new(),
            cursor: 0,
            history: vec![],
            history_index: 0,
            scroll: 0,
            frame: 0,
            pending_actions: Default::default(),
            bundle: None,
            finished: false,
            scenario_diagnostic: None,
            last_poll: Instant::now(),
            reload_cursor: 0,
        };
        tools.load_commands()?;
        for path in tools.loaded.keys() {
            tools.versions.insert(path.to_string_lossy().to_string(), 0);
        }
        for domain in ["dev", "commands", "scenarios"] {
            let directory = tools.root.root.join(domain);
            let mut paths = Vec::new();
            if let Ok(entries) = std::fs::read_dir(directory) {
                for entry in entries.take(128).flatten() {
                    if entry.path().extension().is_some_and(|e| e == "rhai") {
                        paths.push(format!("{domain}/{}", entry.file_name().to_string_lossy()));
                    }
                }
            }
            if domain == "scenarios" {
                tools.registry.cache_completion("scenario", paths);
            } else {
                let name = if domain == "dev" {
                    "script"
                } else {
                    "script-command"
                };
                tools.registry.cache_completion(name, paths);
            }
        }
        Ok(tools)
    }
    fn load_commands(&mut self) -> ControlResult<()> {
        let directory = self.root.root.join("commands");
        if !directory.exists() {
            return Ok(());
        }
        let mut files = std::fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .take(64)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        files.sort_by_key(|e| e.path());
        for entry in files {
            if entry.path().extension().is_none_or(|e| e != "rhai") {
                continue;
            }
            let script = LoadedScript::load(&self.root, entry.path(), &self.runtime)?;
            let spec = self.command_spec(&script.ast)?;
            self.registry.register(
                spec.clone(),
                command_handler(script.ast.clone(), spec.capability.clone()),
            )?;
            self.command_files.insert(spec.name, script.path.clone());
            self.loaded.insert(script.path.clone(), script);
        }
        Ok(())
    }
    fn command_spec(&mut self, ast: &AST) -> ControlResult<rustcraft_control::CommandSpec> {
        command_spec(&mut self.runtime, ast)
    }
    pub fn print(&mut self, text: &str) {
        for line in text.lines().take(64) {
            self.output.push_back(line.chars().take(256).collect());
        }
        while self.output.len() > 128 {
            self.output.pop_front();
        }
        self.scroll = 0;
    }
    pub fn input(&mut self, input: ConsoleInput) {
        match input {
            ConsoleInput::Toggle => self.console_open = !self.console_open,
            ConsoleInput::Close => self.console_open = false,
            ConsoleInput::Insert(text) => self.insert(&text),
            ConsoleInput::Left => self.left(),
            ConsoleInput::Right => self.right(),
            ConsoleInput::Backspace => self.backspace(),
            ConsoleInput::Delete => {
                let start = self.cursor;
                self.right();
                let end = self.cursor;
                self.line.replace_range(start..end, "");
                self.cursor = start;
            }
            ConsoleInput::Home => self.cursor = 0,
            ConsoleInput::End => self.cursor = self.line.len(),
            ConsoleInput::HistoryUp => self.history_move(true),
            ConsoleInput::HistoryDown => self.history_move(false),
            ConsoleInput::Complete => self.complete(),
            ConsoleInput::ScrollUp => {
                self.scroll = (self.scroll + 5).min(self.output.len().saturating_sub(1))
            }
            ConsoleInput::ScrollDown => self.scroll = self.scroll.saturating_sub(5),
        }
    }
    pub fn insert(&mut self, text: &str) {
        if self.line.len() + text.len() <= 4096 {
            self.line.insert_str(self.cursor, text);
            self.cursor += text.len();
        }
    }
    pub fn left(&mut self) {
        self.cursor = self.line[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(i, _)| i);
    }
    pub fn right(&mut self) {
        if self.cursor < self.line.len() {
            self.cursor += self.line[self.cursor..].chars().next().unwrap().len_utf8();
        }
    }
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let end = self.cursor;
            self.left();
            self.line.replace_range(self.cursor..end, "");
        }
    }
    pub fn history_move(&mut self, older: bool) {
        self.history_index = if older {
            self.history_index.saturating_sub(1)
        } else {
            (self.history_index + 1).min(self.history.len())
        };
        self.line = self
            .history
            .get(self.history_index)
            .cloned()
            .unwrap_or_default();
        self.cursor = self.line.len();
    }
    pub fn complete(&mut self) {
        let options = self.registry.complete(&self.line);
        if options.len() == 1 {
            self.line = options[0].clone();
            self.cursor = self.line.len();
        } else {
            self.print(&options.join(" "));
        }
    }
    pub fn text(&self, state: &rustcraft_control::ControlState) -> String {
        rustcraft_control::diagnostics::bounded_text(&self.raw_text(state))
    }
    fn raw_text(&self, state: &rustcraft_control::ControlState) -> String {
        if self.console_open {
            let tail = self
                .output
                .iter()
                .rev()
                .skip(self.scroll)
                .take(12)
                .collect::<Vec<_>>();
            let mut text = String::from("DEV CONSOLE /COMMAND OR RHAI | ESC CLOSE | F10 ABORT\n");
            for line in tail.into_iter().rev() {
                text.push_str(line);
                text.push('\n');
            }
            text.push_str(&self.line[..self.cursor]);
            text.push('|');
            text.push_str(&self.line[self.cursor..]);
            text
        } else {
            state.diagnostic_text()
        }
    }

    pub fn submit(
        &mut self,
        host: &mut impl rustcraft_control::Host,
        state_leased: bool,
    ) -> ControlResult<()> {
        let line = std::mem::take(&mut self.line);
        self.cursor = 0;
        if line.trim().is_empty() {
            return Ok(());
        }
        self.history.push(line.clone());
        if self.history.len() > 128 {
            self.history.remove(0);
        }
        self.history_index = self.history.len();
        self.print(&format!("> {line}"));
        let result = self.evaluate(&line, host, state_leased);
        if let Err(e) = &result {
            self.print(e);
            self.events.push(host.snapshot().tick, "error", e);
        }
        result
    }
    fn enqueue_actions(&mut self, context: &Context, actions: Vec<Action>) -> ControlResult<()> {
        if self.pending_actions.len() + actions.len() > 1024 {
            return Err("control action queue full".into());
        }
        for action in &actions {
            context.require(action.capability())?;
            action.validate()?;
        }
        self.pending_actions
            .extend(actions.into_iter().map(|a| (context.clone(), a)));
        Ok(())
    }
    pub fn evaluate(
        &mut self,
        line: &str,
        host: &mut impl rustcraft_control::Host,
        _state_leased: bool,
    ) -> ControlResult<()> {
        let mut demand = rustcraft_control::diagnostics::query_domains(line);
        if line.contains("fn ") {
            self.repl_domains.extend(demand.iter().copied());
        }
        demand.extend(self.repl_domains.iter().copied());
        if let Some(name) = line
            .trim_start()
            .strip_prefix('/')
            .and_then(|s| s.split_whitespace().next())
            && let Some(script) = self
                .command_files
                .get(name)
                .and_then(|p| self.loaded.get(p))
        {
            demand.extend(script.domains.iter().copied());
        }
        self.prepare(&demand, host);
        let context = self.runtime.context().clone();
        self.events.push(host.snapshot().tick, "console", line);
        if line.starts_with('/') {
            let args = rustcraft_control::parse(line.trim_start_matches('/'))?;
            let validated = self.registry.dispatch(&context, line, &host.snapshot())?;
            match args.first().map(String::as_str) {
                Some("script") if args.get(1).map(String::as_str) == Some("help") => {
                    self.print(API_HELP);
                    return Ok(());
                }
                Some("script") => {
                    context.require("script.load")?;
                    let path = args.get(1).ok_or("usage: /script PATH | help")?;
                    let owner = if context.source == rustcraft_control::Source::Scenario {
                        self.scenario
                            .as_ref()
                            .map(|s| s.result.run_id.clone())
                            .unwrap_or_else(|| "console".into())
                    } else {
                        "console".into()
                    };
                    self.request_compile(
                        path,
                        Purpose::Script {
                            context: Context {
                                source: rustcraft_control::Source::Script,
                                capabilities: context.capabilities.clone(),
                            },
                        },
                        &owner,
                    )?;
                    return Ok(());
                }
                Some("scenario") if args.get(1).map(String::as_str) == Some("abort") => {
                    self.abort(host);
                    return Ok(());
                }
                Some("scenario") => {
                    let path = args.get(1).ok_or("usage: /scenario PATH | abort")?;
                    return self.start_with_context(
                        path,
                        Context {
                            source: rustcraft_control::Source::Scenario,
                            capabilities: context.capabilities.clone(),
                        },
                        host,
                    );
                }
                Some("reload") => {
                    match args.get(1).map(String::as_str).unwrap_or("keep_scope") {
                        "keep_scope" => {}
                        "reset_scope" => self.repl.reset(),
                        _ => return Err("usage: /reload [keep_scope|reset_scope]".into()),
                    }
                    self.request_reloads()?;
                    self.print("reload queued; candidate publication is atomic");
                    return Ok(());
                }
                _ => {}
            }
            let result = validated;
            if let Some(a) = result.action {
                self.enqueue_actions(&context, vec![a])?;
            }
            self.print(&result.message);
        } else {
            let (value, actions) = self.runtime.eval(&mut self.repl, line, host.snapshot())?;
            self.enqueue_actions(&context, actions)?;
            for message in self.repl.diagnostic.output.clone() {
                self.print(&message);
            }
            self.print(&value);
        }
        Ok(())
    }
    pub fn start(
        &mut self,
        path: &str,
        host: &mut impl rustcraft_control::Host,
    ) -> ControlResult<()> {
        self.start_with_context(
            path,
            Context::developer(rustcraft_control::Source::Scenario),
            host,
        )
    }
    pub fn start_with_context(
        &mut self,
        path: &str,
        mut context: Context,
        host: &mut impl rustcraft_control::Host,
    ) -> ControlResult<()> {
        if context.source != rustcraft_control::Source::Scenario {
            return Err("scenario context source required".into());
        }
        context.require("script.load")?;
        if !self.pending_actions.is_empty() {
            return Err(
                "wait for pending control actions or abort them before starting a scenario".into(),
            );
        }
        if self.scenario.as_ref().is_some_and(|s| s.active) {
            return Err("scenario already active".into());
        }
        if self.compiling_scenario {
            return Err("scenario compilation already pending".into());
        }
        host.prepare_diagnostics(&[rustcraft_control::diagnostics::Domain::Renderer]);
        let snapshot = host.snapshot();
        if !snapshot.renderer.is_null()
            && snapshot.renderer["unavailable"].is_null()
            && context.capabilities.contains("debug.capture")
        {
            context.capabilities.insert("render.capture".into());
        }
        self.request_compile(
            path,
            Purpose::Scenario {
                path: path.into(),
                context,
            },
            "scenario-compile",
        )?;
        self.compiling_scenario = true;
        self.finished = false;
        self.bundle = None;
        Ok(())
    }
    fn request_compile(
        &mut self,
        path: &str,
        purpose: Purpose,
        owner: &str,
    ) -> ControlResult<rustcraft_control::JobRef> {
        let path = PathBuf::from(script_relative(path));
        let path = if path.is_absolute() {
            path
        } else {
            self.root.root.join(path)
        };
        let identity = path.to_string_lossy().to_string();
        if !self.versions.contains_key(&identity) && self.versions.len() >= 128 {
            return Err("loaded script identity limit".into());
        }
        let generation = self.versions.get(&identity).copied().unwrap_or(0) + 1;
        let id = self.jobs.create_owned(owner)?;
        let known = self.loaded.get(&path);
        let task = worker::Task::Compile {
            path: path.clone(),
            hash: if matches!(purpose, Purpose::Reload) {
                known.map(|s| s.hash.clone())
            } else {
                None
            },
            command: self.command_files.values().any(|p| p == &path),
        };
        self.request_worker(worker::Request {
            id,
            identity: identity.clone(),
            generation,
            task,
        })?;
        self.versions.insert(identity, generation);
        self.purposes.insert(id, purpose);
        self.events.push(
            0,
            "compile-request",
            &format!("{} generation {generation}", path.display()),
        );
        Ok(id)
    }
    fn request_reloads(&mut self) -> ControlResult<()> {
        let paths = self
            .loaded
            .keys()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        if !paths.is_empty() {
            let count = paths.len().min(4);
            for offset in 0..count {
                self.request_compile(
                    &paths[(self.reload_cursor + offset) % paths.len()],
                    Purpose::Reload,
                    "reload",
                )?;
            }
            self.reload_cursor = (self.reload_cursor + count) % paths.len();
        }
        Ok(())
    }
    fn poll_worker(&mut self, host: &mut impl rustcraft_control::Host) -> ControlResult<()> {
        for _ in 0..16 {
            let Some(reply) = self.worker.poll() else {
                break;
            };
            let purpose = self.purposes.remove(&reply.id);
            if self.jobs.status(reply.id).ok() != Some(&rustcraft_control::JobStatus::Pending) {
                continue;
            }
            if !matches!(purpose, Some(Purpose::Bundle | Purpose::Capture))
                && self.versions.get(&reply.identity) != Some(&reply.generation)
            {
                self.jobs.cancel(reply.id)?;
                self.events
                    .push(host.snapshot().tick, "compile-stale", &reply.identity);
                continue;
            }
            let result: ControlResult<()> = (|| {
                match reply.result? {
                    worker::Payload::Written(directory) => {
                        if matches!(purpose, Some(Purpose::Bundle)) {
                            self.bundle = Some(directory);
                        }
                    }
                    worker::Payload::Compiled(None) => {
                        if let Some(script) = self.loaded.get_mut(&PathBuf::from(&reply.identity)) {
                            script.last_error = None;
                        }
                    }
                    worker::Payload::Compiled(Some(candidate)) => {
                        let (mut script, spec) = *candidate;
                        script.generation = self
                            .loaded
                            .get(&script.path)
                            .map_or(1, |s| s.generation + 1);
                        if let Some(spec) = spec {
                            let previous = self
                                .registry
                                .specs()
                                .into_iter()
                                .find(|s| s.name == spec.name)
                                .ok_or("command contract missing")?;
                            if serde_json::to_value(previous).unwrap()
                                != serde_json::to_value(&spec).unwrap()
                            {
                                return Err(
                                    "hot reload metadata changed; restart to alter contract".into(),
                                );
                            }
                            self.registry.replace_handler(
                                &spec.name,
                                command_handler(script.ast.clone(), spec.capability),
                            )?;
                        }
                        self.prepare(&script.domains, host);
                        match purpose.as_ref() {
                            Some(Purpose::Scenario { path, context }) => {
                                self.compiling_scenario = false;
                                let mut runtime =
                                    RhaiRuntime::new(context.clone(), Limits::default());
                                let mut session = RhaiSession::new("scenario");
                                session.diagnostic.source = script.path.display().to_string();
                                session.diagnostic.generation = script.generation;
                                session.diagnostic.compile_us = script.compile_us;
                                let (_, _, steps) = runtime.execute(
                                    &mut session,
                                    &script.ast,
                                    host.snapshot(),
                                    true,
                                )?;
                                self.scenario = Some(rustcraft_control::Scenario::new(
                                    path.clone(),
                                    steps,
                                    context.clone(),
                                    script.generation,
                                    script.hash.clone(),
                                )?);
                                self.scenario_diagnostic = Some(session.diagnostic);
                                self.events
                                    .push(host.snapshot().tick, "scenario-start", path);
                            }
                            Some(Purpose::Script { context }) => {
                                let mut runtime =
                                    RhaiRuntime::new(context.clone(), Limits::default());
                                let mut session = RhaiSession::new("script");
                                session.diagnostic.source = script.path.display().to_string();
                                session.diagnostic.compile_us = script.compile_us;
                                let (value, actions, _) = runtime.execute(
                                    &mut session,
                                    &script.ast,
                                    host.snapshot(),
                                    false,
                                )?;
                                self.enqueue_actions(context, actions)?;
                                for line in session.diagnostic.output {
                                    self.print(&line);
                                }
                                self.print(&value);
                            }
                            _ => {}
                        }
                        self.events.push(
                            host.snapshot().tick,
                            "compile-publish",
                            &format!("{} generation {}", script.path.display(), script.generation),
                        );
                        self.loaded.insert(script.path.clone(), script);
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => {
                    self.jobs.finish(
                        reply.id,
                        rustcraft_control::JobStatus::Complete(
                            serde_json::json!({"identity":reply.identity}),
                        ),
                    )?;
                }
                Err(error) => {
                    self.jobs.finish(
                        reply.id,
                        rustcraft_control::JobStatus::Failed(error.clone()),
                    )?;
                    if let Some(script) = self.loaded.get_mut(&PathBuf::from(&reply.identity)) {
                        script.last_error = Some(error.clone());
                    }
                    if let Some(Purpose::Scenario { path, context, .. }) = purpose {
                        self.compiling_scenario = false;
                        let mut scenario = rustcraft_control::Scenario::new(
                            path,
                            vec![],
                            context,
                            0,
                            String::new(),
                        )?;
                        scenario.error(error.clone(), host, &mut self.events);
                        self.scenario = Some(scenario);
                    }
                    self.print(&error);
                }
            }
            self.events.push(
                host.snapshot().tick,
                "job-terminal",
                &format!("job {}", reply.id.0),
            );
        }
        Ok(())
    }
    fn request_worker(&mut self, request: worker::Request) -> ControlResult<()> {
        let id = request.id;
        if let Err(error) = self.worker.request(request) {
            self.jobs
                .finish(id, rustcraft_control::JobStatus::Failed(error.clone()))?;
            self.events
                .push(0, "job-admission-failed", &format!("job {}: {error}", id.0));
            return Err(error);
        }
        Ok(())
    }
    pub fn record_capture_failure(
        &mut self,
        error: &str,
    ) -> ControlResult<Option<rustcraft_control::JobRef>> {
        self.print(&format!("capture failed: {error}"));
        self.events.push(0, "capture-failed", error);
        let Some(scenario) = self.scenario.as_mut() else {
            return Ok(None);
        };
        if scenario.result.status == "pass" {
            scenario.result.status = "error".into();
        }
        scenario.result.error = Some(format!(
            "{}; capture failed: {}",
            scenario.result.error.as_deref().unwrap_or(""),
            error.chars().take(1024).collect::<String>()
        ));
        let Some(directory) = self.bundle.clone() else {
            return Ok(None);
        };
        let files = vec![
            (
                "result.json".into(),
                serde_json::to_value(&scenario.result).unwrap(),
            ),
            (
                "capture-error.json".into(),
                serde_json::json!({"error":error,"partial_bundle":true}),
            ),
        ];
        let id = self.jobs.create_owned("capture")?;
        self.request_worker(worker::Request {
            id,
            identity: "capture-error".into(),
            generation: 0,
            task: worker::Task::Write {
                directory,
                files,
                text: vec![],
            },
        })?;
        self.purposes.insert(id, Purpose::Capture);
        Ok(Some(id))
    }
    pub fn write_capture(
        &mut self,
        path: PathBuf,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> ControlResult<rustcraft_control::JobRef> {
        let id = self.jobs.create_owned("capture")?;
        self.request_worker(worker::Request {
            id,
            identity: "capture".into(),
            generation: 0,
            task: worker::Task::Png {
                path,
                width,
                height,
                rgba,
            },
        })?;
        self.purposes.insert(id, Purpose::Capture);
        Ok(id)
    }
    pub fn write_snapshot(
        &mut self,
        directory: PathBuf,
        snapshot: Snapshot,
    ) -> ControlResult<rustcraft_control::JobRef> {
        let id = self.jobs.create_owned("capture")?;
        self.request_worker(worker::Request {
            id,
            identity: "capture-state".into(),
            generation: 0,
            task: worker::Task::Write {
                directory,
                files: vec![("state.json".into(), serde_json::to_value(snapshot).unwrap())],
                text: vec![],
            },
        })?;
        self.purposes.insert(id, Purpose::Capture);
        Ok(id)
    }
    pub fn abort(&mut self, host: &mut impl rustcraft_control::Host) {
        self.pending_actions.clear();
        self.jobs.cancel_owner("scenario-compile");
        if self.compiling_scenario
            && let Some(path) = self.purposes.values().find_map(|p| match p {
                Purpose::Scenario { path, .. } => Some(path.clone()),
                _ => None,
            })
            && let Ok(mut s) = rustcraft_control::Scenario::new(
                path,
                vec![],
                Context::developer(rustcraft_control::Source::Scenario),
                0,
                String::new(),
            )
        {
            s.cancel(host, &mut self.events);
            self.scenario = Some(s);
        }
        self.compiling_scenario = false;
        if let Some(s) = self.scenario.as_mut().filter(|s| s.active) {
            self.jobs.cancel_owner(&s.result.run_id);
            s.cancel(host, &mut self.events);
        }
    }
    pub fn prepare(
        &self,
        domains: &[rustcraft_control::diagnostics::Domain],
        host: &mut impl rustcraft_control::Host,
    ) {
        if domains.contains(&rustcraft_control::diagnostics::Domain::Scripts)
            && host.diagnostic_due(rustcraft_control::diagnostics::Domain::Scripts)
        {
            let started = Instant::now();
            host.publish_diagnostic(
                rustcraft_control::diagnostics::Domain::Scripts,
                serde_json::json!({"repl":self.repl.diagnostic,
                "loaded":self.loaded.values().take(32).map(|s|serde_json::json!({"path":s.path,
                "generation":s.generation,
                "error":s.last_error})).collect::<Vec<_>>(),
                "loaded_truncated":self.loaded.len()>32,
                "jobs":self.jobs.snapshot(),
                "compile_pending":self.compiling_scenario,
                "step":self.scenario.as_ref().map(|s|s.cursor),
                "scenario":self.scenario.as_ref().map(|s|&s.result),
                "recent_output":self.output.iter().rev().take(4).collect::<Vec<_>>(),
                "frame":self.frame}),
                started,
            );
        }
        host.prepare_diagnostics(domains);
    }
    pub fn advance(
        &mut self,
        host: &mut impl rustcraft_control::Host,
        graphical: bool,
    ) -> ControlResult<Option<PathBuf>> {
        use rustcraft_control::diagnostics::query_domains;
        let demand = self
            .scenario
            .as_ref()
            .filter(|s| s.active)
            .and_then(|s| s.steps.get(s.cursor))
            .map(|step| match step {
                Step::Command(line) => query_domains(line),
                Step::Wait {
                    predicate: Predicate::DomainIdle(name),
                    ..
                }
                | Step::Assert {
                    predicate: Predicate::DomainIdle(name),
                    ..
                } => query_domains(name),
                _ => vec![],
            })
            .unwrap_or_default();
        self.prepare(&demand, host);
        if let Some((context, action)) = self.pending_actions.pop_front()
            && let Err(error) = rustcraft_control::execute(host, &context, &action)
        {
            self.print(&error);
            self.events
                .push(host.snapshot().tick, "command-error", &error);
        }
        self.poll_worker(host)?;
        if let Some(s) = self.scenario.as_ref().filter(|s| !s.active) {
            self.jobs.cancel_owner(&s.result.run_id);
        }

        if self.last_poll.elapsed() > Duration::from_millis(500) {
            self.last_poll = Instant::now();
            if let Err(error) = self.request_reloads() {
                self.print(&error);
            }
        }
        let console =
            self.scenario
                .as_ref()
                .filter(|s| s.active)
                .and_then(|s| match s.steps.get(s.cursor) {
                    Some(Step::Console { open, line }) => Some((*open, line.clone())),
                    _ => None,
                });
        if let Some((open, line)) = console {
            if !self
                .scenario
                .as_ref()
                .unwrap()
                .context
                .capabilities
                .contains("debug.configure")
            {
                self.scenario.as_mut().unwrap().error(
                    "console operation denied: debug.configure".into(),
                    host,
                    &mut self.events,
                );
            } else {
                if open {
                    let _ = host.apply(&Action::DebugUi(
                        rustcraft_control::diagnostics::DebugInput::Close,
                    ));
                }
                if open != self.console_open {
                    self.input(ConsoleInput::Toggle);
                }
                if let Some(line) = line {
                    self.line.clear();
                    self.cursor = 0;
                    self.input(ConsoleInput::Insert(line));
                    let context = self.scenario.as_ref().unwrap().context.clone();
                    let previous = std::mem::replace(
                        &mut self.runtime,
                        RhaiRuntime::new(context, Limits::default()),
                    );
                    let _ = self.submit(host, false);
                    self.runtime = previous;
                }
                let s = self.scenario.as_mut().unwrap();
                s.steps[s.cursor] = Step::WaitTicks(0);
            }
        }
        let reload =
            self.scenario
                .as_ref()
                .filter(|s| s.active)
                .and_then(|s| match s.steps.get(s.cursor) {
                    Some(Step::Reload(path)) => Some((path.clone(), s.result.run_id.clone())),
                    _ => None,
                });
        if let Some((path, owner)) = reload {
            self.scenario
                .as_ref()
                .unwrap()
                .context
                .require("script.load")?;
            let id = self.request_compile(&path, Purpose::Reload, &owner)?;
            let s = self.scenario.as_mut().unwrap();
            s.steps[s.cursor] = Step::WaitJob {
                id,
                timeout_ms: 30000,
            };
        }
        let jobs = if self
            .scenario
            .as_ref()
            .is_some_and(|s| s.active || !self.finished)
        {
            self.jobs.snapshot()
        } else {
            serde_json::Value::Null
        };
        let mut wrapped = JobHost { host, jobs };
        if let Some(s) = self.scenario.as_mut() {
            s.advance(
                &mut wrapped,
                &self.registry,
                self.frame,
                graphical,
                &mut self.events,
            );
            if !s.active && !self.finished {
                self.finished = true;
                self.jobs.cancel_owner(&s.result.run_id);
                wrapped.jobs = self.jobs.snapshot();
                let directory = PathBuf::from("target/test-runs")
                    .join("scenario")
                    .join(&s.result.run_id);
                wrapped.prepare_diagnostics(&rustcraft_control::diagnostics::Domain::ALL);
                let mut snapshot = wrapped.snapshot();
                // Expensive host domains may be cadence-cached; terminal control state is current.
                snapshot.scripts["scenario"] = serde_json::to_value(&s.result).unwrap();
                snapshot.scripts["step"] = serde_json::json!(s.cursor);
                let mut files = vec![
                    (
                        "result.json".into(),
                        serde_json::to_value(&s.result).unwrap(),
                    ),
                    (
                        "state.json".into(),
                        serde_json::to_value(&snapshot).unwrap(),
                    ),
                ];
                for (name, value) in [
                    ("player", &snapshot.player),
                    ("entities", &snapshot.entities),
                    ("streaming", &snapshot.streaming),
                    ("lighting", &snapshot.lighting),
                    ("meshing", &snapshot.meshing),
                    ("renderer", &snapshot.renderer),
                    ("persistence", &snapshot.persistence),
                ] {
                    if !value.is_null() {
                        files.push((format!("{name}.json"), value.clone()));
                    }
                }
                files.push((
                    "scripts.json".into(),
                    serde_json::json!({"repl":self.repl.diagnostic,
                    "scenario":self.scenario_diagnostic,
                    "result":s.result,
                    "jobs":self.jobs.snapshot()}),
                ));
                let text = vec![
                    (
                        "recent-events.jsonl".into(),
                        self.events
                            .entries()
                            .iter()
                            .map(|e| serde_json::to_string(e).unwrap())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    ),
                    (
                        "console.log".into(),
                        self.output.iter().cloned().collect::<Vec<_>>().join("\n"),
                    ),
                    ("scenario.log".into(), format!("{:?}", s.result)),
                ];
                let id = self.jobs.create_owned("bundle")?;
                self.request_worker(worker::Request {
                    id,
                    identity: "bundle".into(),
                    generation: 0,
                    task: worker::Task::Write {
                        directory,
                        files,
                        text,
                    },
                })?;
                self.purposes.insert(id, Purpose::Bundle);
                self.bundle_job = Some(id);
                self.bundle_started = Instant::now();
            }
        }
        if let Some(id) = self.bundle_job {
            if self.bundle_started.elapsed() > Duration::from_secs(30) {
                return Err(
                    "failure bundle job deadline exceeded; partial diagnostics may exist".into(),
                );
            }
            match self.jobs.status(id)? {
                rustcraft_control::JobStatus::Complete(_) => {
                    self.bundle_job = None;
                    return Ok(self.bundle.clone());
                }
                rustcraft_control::JobStatus::Failed(error) => {
                    return Err(format!("failure bundle write failed: {error}"));
                }
                _ => {}
            }
        }
        Ok(None)
    }
}
struct JobHost<'a, H> {
    host: &'a mut H,
    jobs: serde_json::Value,
}
impl<H: rustcraft_control::Host> rustcraft_control::Host for JobHost<'_, H> {
    fn snapshot(&self) -> Snapshot {
        let mut s = self.host.snapshot();
        if s.scripts.is_null() {
            s.scripts = serde_json::json!({});
        }
        s.scripts["jobs"] = self.jobs.clone();
        s
    }
    fn diagnostic_due(&self, domain: rustcraft_control::diagnostics::Domain) -> bool {
        self.host.diagnostic_due(domain)
    }
    fn prepare_diagnostics(&mut self, domains: &[rustcraft_control::diagnostics::Domain]) {
        self.host.prepare_diagnostics(domains);
    }
    fn publish_diagnostic(
        &mut self,
        domain: rustcraft_control::diagnostics::Domain,
        value: serde_json::Value,
        started: Instant,
    ) {
        self.host.publish_diagnostic(domain, value, started);
    }
    fn block(&self, p: [i32; 3]) -> ControlResult<String> {
        self.host.block(p)
    }
    fn apply(&mut self, a: &Action) -> ControlResult<serde_json::Value> {
        self.host.apply(a)
    }
    fn intent(&mut self, i: rustcraft_control::AgentIntent) -> ControlResult<()> {
        self.host.intent(i)
    }
}
fn script_relative(path: &str) -> &str {
    path.strip_prefix("scripts/").unwrap_or(path)
}
pub fn write_json(path: &Path, value: &impl Serialize) -> ControlResult<()> {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn check_scripts(path: &Path) -> ControlResult<()> {
    let tools = DevTools::new(Path::new("scripts"), rustcraft_control::engine_registry())?;
    let runtime = &tools.runtime;
    let root = ScriptRoot::new("scripts")?;
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root.root) {
        return Err("script-check path outside approved root".into());
    }
    let mut pending = vec![canonical];
    let mut checked = 0;
    while let Some(path) = pending.pop() {
        if checked + pending.len() > 1024 {
            return Err("script inventory limit".into());
        }
        if path.is_dir() {
            for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
                    continue;
                }
                pending.push(entry.path());
            }
        } else if path.extension().is_some_and(|e| e == "rhai") {
            let (_, source) = root.read(path.canonicalize().map_err(|e| e.to_string())?)?;
            runtime
                .compile(&source)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            checked += 1;
        }
    }
    if checked == 0 {
        return Err("no Rhai scripts found".into());
    }
    println!(
        "SCRIPT_CHECK {checked} scripts, Rhai {RHAI_VERSION}, Control API {CONTROL_API_VERSION}"
    );
    Ok(())
}
pub fn benchmark() {
    let mut runtime = RhaiRuntime::new(
        Context::developer(rustcraft_control::Source::Script),
        Limits::default(),
    );
    let start = Instant::now();
    let ast = runtime.compile("tick()").unwrap();
    let compile = start.elapsed();
    let mut session = RhaiSession::new("bench");
    let start = Instant::now();
    for _ in 0..1000 {
        runtime
            .execute(&mut session, &ast, Snapshot::default(), false)
            .unwrap();
    }
    let eval = start.elapsed();
    let registry = rustcraft_control::engine_registry();
    let context = Context::developer(rustcraft_control::Source::Script);
    let start = Instant::now();
    for _ in 0..1000 {
        registry
            .dispatch(&context, "/pause", &Snapshot::default())
            .unwrap();
    }
    let dispatch = start.elapsed();
    struct BenchHost;
    impl rustcraft_control::Host for BenchHost {
        fn snapshot(&self) -> Snapshot {
            Snapshot::default()
        }
        fn block(&self, _: [i32; 3]) -> ControlResult<String> {
            Ok("sample:air".into())
        }
        fn apply(&mut self, _: &Action) -> ControlResult<serde_json::Value> {
            Ok(serde_json::Value::Null)
        }
        fn intent(&mut self, _: rustcraft_control::AgentIntent) -> ControlResult<()> {
            Ok(())
        }
    }
    let mut scenario = rustcraft_control::Scenario::new(
        "benchmark".into(),
        vec![rustcraft_control::Step::AssertTickDelta(0); 1000]
            .into_iter()
            .enumerate()
            .map(|(i, step)| {
                if i == 0 {
                    rustcraft_control::Step::Checkpoint
                } else {
                    step
                }
            })
            .collect(),
        context,
        1,
        "benchmark".into(),
    )
    .unwrap();
    let mut events = rustcraft_control::EventRing::default();
    let start = Instant::now();
    for _ in 0..1000 {
        scenario.advance(&mut BenchHost, &registry, 0, false, &mut events);
    }
    let steps = start.elapsed();
    let start = Instant::now();
    let limit = runtime
        .eval(&mut session, "loop {}", Snapshot::default())
        .err();
    println!(
        "SCRIPT_BENCH Rhai={RHAI_VERSION} compile_us={} ast_query_1000_us={} command_1000_us={} scenario_steps_1000_us={} runaway_us={} runaway_error={limit:?}",
        compile.as_micros(),
        eval.as_micros(),
        dispatch.as_micros(),
        steps.as_micros(),
        start.elapsed().as_micros()
    );
}

fn command_handler(
    ast: AST,
    capability: String,
) -> impl Fn(&[String], &Snapshot) -> ControlResult<rustcraft_control::CommandResult> + Send + Sync
{
    move |args, snapshot| {
        let mut context = Context::read_only(rustcraft_control::Source::Script);
        context.capabilities.insert(capability.clone());
        let mut runtime = RhaiRuntime::new(context, Limits::default());
        let (value, mut actions) = runtime.call(
            &mut RhaiSession::new("command"),
            &ast,
            "command",
            args.iter().cloned().map(Dynamic::from).collect(),
            snapshot.clone(),
        )?;
        if actions.len() > 1 {
            return Err(
                "command script may emit one semantic mutation; use a scenario for sequences"
                    .into(),
            );
        }
        let mut result = rustcraft_control::CommandResult::output(
            value.to_string(),
            serde_json::json!({"value":value.to_string()}),
        );
        result.action = actions.pop();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustcraft_control::Source;
    #[test]
    fn persistent_scope_and_isolation() {
        let mut r = RhaiRuntime::new(
            Context::developer(Source::DeveloperConsole),
            Limits::default(),
        );
        let mut a = RhaiSession::new("a");
        r.eval(&mut a, "let x=40;", Snapshot::default()).unwrap();
        assert_eq!(r.eval(&mut a, "x+2", Snapshot::default()).unwrap().0, "42");
        assert!(
            r.eval(&mut RhaiSession::new("b"), "x", Snapshot::default())
                .is_err()
        );
    }
    #[test]
    fn bounded_errors() {
        let mut r = RhaiRuntime::new(Context::developer(Source::Script), Limits::default());
        for script in [
            "loop {}",
            "fn f(){f()} f()",
            "let a=[]; loop { a.push(1); }",
            "let s=\"x\"; loop { s+=s; }",
            "import \"/etc/passwd\" as x;",
        ] {
            assert!(
                r.eval(&mut RhaiSession::new("limit"), script, Snapshot::default())
                    .is_err(),
                "{script}"
            );
        }
    }
    #[test]
    fn denied_mutations() {
        let mut r = RhaiRuntime::new(Context::read_only(Source::Script), Limits::default());
        for script in [
            "set_block(0,1,0,\"sample:stone\")",
            "teleport(1.0,2.0,3.0)",
            "pause()",
        ] {
            assert!(
                r.eval(&mut RhaiSession::new("read"), script, Snapshot::default())
                    .unwrap_err()
                    .contains("capability denied")
            );
        }
    }
    #[test]
    fn hot_reload_and_paths() {
        let path = std::env::temp_dir().join(format!("dx1-reload-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let file = path.join("test.rhai");
        std::fs::write(&file, "40+2").unwrap();
        let root = ScriptRoot::new(&path).unwrap();
        let mut r = RhaiRuntime::new(Context::developer(Source::Script), Limits::default());
        let mut s = LoadedScript::load(&root, "test.rhai", &r).unwrap();
        let mut session = RhaiSession::new("reload");
        assert_eq!(
            r.execute(&mut session, &s.ast, Snapshot::default(), false)
                .unwrap()
                .0,
            "42"
        );
        std::fs::write(&file, "40+3").unwrap();
        assert!(s.reload(&root, &r).unwrap());
        assert_eq!(
            r.execute(&mut session, &s.ast, Snapshot::default(), false)
                .unwrap()
                .0,
            "43"
        );
        std::fs::write(&file, "let =").unwrap();
        assert!(s.reload(&root, &r).is_err());
        assert_eq!(s.generation, 2);
        assert_eq!(
            r.execute(&mut session, &s.ast, Snapshot::default(), false)
                .unwrap()
                .0,
            "43"
        );
        assert!(root.resolve("../outside.rhai").is_err());
        assert!(root.resolve("/etc/passwd").is_err());
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[cfg(test)]
mod tooling_tests {
    use super::*;
    use rustcraft_control::{Action, Host, Registry};
    struct TestHost;
    impl Host for TestHost {
        fn snapshot(&self) -> Snapshot {
            Snapshot {
                player: serde_json::json!({"position":[1.,2.,3.]}),
                ..Default::default()
            }
        }
        fn block(&self, _: [i32; 3]) -> ControlResult<String> {
            Ok("sample:air".into())
        }
        fn apply(&mut self, _: &Action) -> ControlResult<serde_json::Value> {
            Ok(serde_json::Value::Null)
        }
        fn intent(&mut self, _: rustcraft_control::AgentIntent) -> ControlResult<()> {
            Ok(())
        }
    }
    fn drain(tools: &mut DevTools) {
        let started = Instant::now();
        while !tools.purposes.is_empty() {
            tools.poll_worker(&mut TestHost).unwrap();
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    #[test]
    fn scenario_declared_capabilities_do_not_escalate() {
        let path = std::env::temp_dir().join(format!("dx1-scenario-caps-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("run.rhai"), "set_block(0,1,0,\"sample:stone\");").unwrap();
        let mut tools = DevTools::new(&path, Registry::default()).unwrap();
        let mut context = Context::read_only(rustcraft_control::Source::Scenario);
        context.capabilities.insert("script.load".into());
        tools
            .start_with_context("run.rhai", context, &mut TestHost)
            .unwrap();
        drain(&mut tools);
        let result = &tools.scenario.as_ref().unwrap().result;
        assert_eq!(result.status, "error");
        assert!(result.error.as_ref().unwrap().contains("world.write"));
        assert!(!result.capabilities.contains("world.write"));
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn capture_failure_updates_machine_result_and_partial_manifest() {
        let path = std::env::temp_dir().join(format!("dx1-capture-fail-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("run.rhai"), "wait_ticks(0);").unwrap();
        let mut tools = DevTools::new(&path, Registry::default()).unwrap();
        tools.start("run.rhai", &mut TestHost).unwrap();
        let started = Instant::now();
        let bundle = loop {
            if let Some(dir) = tools.advance(&mut TestHost, false).unwrap() {
                break dir;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        };
        let png = path.join("existing.png");
        std::fs::write(&png, b"existing").unwrap();
        let id = tools.write_capture(png, 1, 1, vec![0; 4]).unwrap();
        drain(&mut tools);
        let error = match tools.jobs.status(id).unwrap() {
            rustcraft_control::JobStatus::Failed(error) => error.clone(),
            other => panic!("expected failure, got {other:?}"),
        };
        tools.record_capture_failure(&error).unwrap();
        drain(&mut tools);
        let result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(bundle.join("result.json")).unwrap()).unwrap();
        assert_eq!(result["status"], "error");
        assert!(bundle.join("capture-error.json").exists());
        std::fs::remove_dir_all(bundle).unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn full_worker_queue_does_not_orphan_jobs() {
        let path = std::env::temp_dir().join(format!("dx1-backpressure-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let mut tools = DevTools::new(&path, Registry::default()).unwrap();
        let mut denied = 0;
        for _ in 0..100 {
            if tools
                .write_snapshot(path.join("output"), Snapshot::default())
                .is_err()
            {
                denied += 1;
            }
        }
        assert!(denied > 0);
        let count = tools
            .jobs
            .snapshot()
            .as_object()
            .unwrap()
            .values()
            .filter(|s| s["status"] == "Pending")
            .count();
        assert_eq!(count, tools.purposes.len());
        drain(&mut tools);
        assert!(
            !tools
                .jobs
                .snapshot()
                .as_object()
                .unwrap()
                .values()
                .any(|s| s["status"] == "Pending")
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn stale_candidate_and_pending_shutdown_are_safe() {
        let path = std::env::temp_dir().join(format!("dx1-race-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let file = path.join("test.rhai");
        std::fs::write(&file, "41").unwrap();
        let mut tools = DevTools::new(&path, Registry::default()).unwrap();
        let first = tools
            .request_compile("test.rhai", Purpose::Reload, "reload")
            .unwrap();
        // Force V3 to be queued before publication of V2, independent of worker timing.
        std::fs::write(&file, "42").unwrap();
        let second = tools
            .request_compile("test.rhai", Purpose::Reload, "reload")
            .unwrap();
        drain(&mut tools);
        assert_eq!(
            tools.jobs.status(first).unwrap(),
            &rustcraft_control::JobStatus::Cancelled
        );
        assert!(matches!(
            tools.jobs.status(second).unwrap(),
            rustcraft_control::JobStatus::Complete(_)
        ));
        let ast = &tools.loaded[&file.canonicalize().unwrap()].ast;
        let (value, _, _) = tools
            .runtime
            .execute(
                &mut RhaiSession::new("race"),
                ast,
                Snapshot::default(),
                false,
            )
            .unwrap();
        assert_eq!(value, "42");
        for _ in 0..8 {
            tools
                .request_compile("test.rhai", Purpose::Reload, "reload")
                .unwrap();
        }
        let started = Instant::now();
        drop(tools);
        assert!(started.elapsed() < Duration::from_secs(2));
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn scenario_reload_yields_and_cancel_ignores_late_compile() {
        let path = std::env::temp_dir().join(format!("dx1-jobwait-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("candidate.rhai"), "42").unwrap();
        std::fs::write(
            path.join("scenario.rhai"),
            "reload_script(\"candidate.rhai\"); wait_ticks(100);",
        )
        .unwrap();
        let mut tools = DevTools::new(&path, Registry::default()).unwrap();
        tools.start("scenario.rhai", &mut TestHost).unwrap();
        drain(&mut tools);
        tools.advance(&mut TestHost, false).unwrap();
        assert!(matches!(
            tools.scenario.as_ref().unwrap().steps[0],
            Step::WaitJob { .. }
        ));
        tools.abort(&mut TestHost);
        drain(&mut tools);
        assert_eq!(tools.scenario.as_ref().unwrap().result.status, "cancelled");
        assert!(!tools.loaded.contains_key(&path.join("candidate.rhai")));
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn console_editor_commands_repl_and_recovery() {
        let path = std::env::temp_dir().join(format!("dx1-editor-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        let mut tools = DevTools::new(&path, rustcraft_control::engine_registry()).unwrap();
        tools.input(ConsoleInput::Toggle);
        assert!(tools.console_open);
        tools.input(ConsoleInput::Insert("a🦀b".into()));
        tools.input(ConsoleInput::Left);
        tools.input(ConsoleInput::Backspace);
        assert_eq!(tools.line, "ab");
        tools.input(ConsoleInput::Home);
        tools.input(ConsoleInput::Delete);
        assert_eq!(tools.line, "b");
        tools.input(ConsoleInput::End);
        tools.input(ConsoleInput::Backspace);
        tools.input(ConsoleInput::Insert("/hel".into()));
        tools.input(ConsoleInput::Complete);
        assert_eq!(tools.line, "/help");
        tools.submit(&mut TestHost, false).unwrap();
        tools.input(ConsoleInput::HistoryUp);
        assert_eq!(tools.line, "/help");
        tools.input(ConsoleInput::HistoryDown);
        assert!(tools.line.is_empty());
        for line in ["/commands", "/inspect", "player()", "let x = 40;", "x + 2"] {
            tools.input(ConsoleInput::Insert(line.into()));
            tools.submit(&mut TestHost, false).unwrap();
        }
        assert_eq!(tools.output.back().unwrap(), "42");
        for line in ["let bad = ;", "/missing", "1 / 0"] {
            tools.input(ConsoleInput::Insert(line.into()));
            assert!(tools.submit(&mut TestHost, false).is_err());
        }
        tools.input(ConsoleInput::Insert("x + 2".into()));
        tools.submit(&mut TestHost, false).unwrap();
        assert_eq!(tools.output.back().unwrap(), "42");
        tools.input(ConsoleInput::ScrollUp);
        assert!(tools.scroll > 0);
        tools.input(ConsoleInput::ScrollDown);
        tools.input(ConsoleInput::Close);
        assert!(!tools.console_open);
        let mut other = DevTools::new(&path, rustcraft_control::engine_registry()).unwrap();
        assert!(other.evaluate("x", &mut TestHost, false).is_err());
        for _ in 0..300 {
            tools.print("bounded output");
            let id = tools.jobs.create_owned("soak").unwrap();
            tools
                .jobs
                .finish(
                    id,
                    rustcraft_control::JobStatus::Complete(serde_json::Value::Null),
                )
                .unwrap();
            tools.events.push(0, "soak", "bounded");
        }
        assert!(tools.output.len() <= 128);
        assert!(tools.events.entries().len() <= 128);
        assert!(tools.jobs.snapshot().as_object().unwrap().len() <= 34);
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn repeated_scenario_job_soak_stabilizes() {
        let path = std::env::temp_dir().join(format!("dx1-soak-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("candidate.rhai"), "42").unwrap();
        std::fs::write(
            path.join("run.rhai"),
            "reload_script(\"candidate.rhai\"); wait_ticks(0);",
        )
        .unwrap();
        let mut tools = DevTools::new(&path, rustcraft_control::engine_registry()).unwrap();
        for n in 0..40 {
            tools.start("run.rhai", &mut TestHost).unwrap();
            drain(&mut tools);
            if n % 2 == 0 {
                tools.abort(&mut TestHost);
            }
            let started = Instant::now();
            loop {
                if let Some(bundle) = tools.advance(&mut TestHost, false).unwrap() {
                    let read = |name: &str| -> serde_json::Value {
                        serde_json::from_slice(&std::fs::read(bundle.join(name)).unwrap()).unwrap()
                    };
                    let result = read("result.json");
                    assert_eq!(read("state.json")["scripts"]["scenario"], result);
                    assert_eq!(read("scripts.json")["result"], result);
                    std::fs::remove_dir_all(bundle).unwrap();
                    break;
                }
                assert!(started.elapsed() < Duration::from_secs(5));
                std::thread::sleep(Duration::from_millis(1));
            }
            // The periodic reload poll may have admitted work during bundle publication.
            // Observe quiescence before testing retained state, rather than machine speed.
            drain(&mut tools);
            assert!(tools.purposes.is_empty());
            assert!(tools.loaded.len() <= 2);
            assert!(tools.versions.len() <= 2);
            assert!(tools.jobs.snapshot().as_object().unwrap().len() <= 35);
            assert!(tools.events.entries().len() <= 128);
            assert!(!tools.scenario.as_ref().unwrap().active);
        }
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn command_reload_retains_valid_contract() {
        let path = std::env::temp_dir().join(format!("dx1-command-{}", std::process::id()));
        std::fs::create_dir_all(path.join("commands")).unwrap();
        let file = path.join("commands/test.rhai");
        let code = |n| {
            format!(
                "fn command_spec() {{ #{{id:\"sample:where\",name:\"where\",usage:\"\",help:\"test\",capability:\"player.read\"}} }} fn command(args) {{{n}}}"
            )
        };
        std::fs::write(&file, code(42)).unwrap();
        let mut tools = DevTools::new(&path, Registry::default()).unwrap();
        let ctx = Context::developer(rustcraft_control::Source::Scenario);
        assert_eq!(
            tools
                .registry
                .dispatch(&ctx, "/where", &Snapshot::default())
                .unwrap()
                .message,
            "42"
        );
        std::fs::write(&file, code(43)).unwrap();
        tools.request_reloads().unwrap();
        drain(&mut tools);
        assert_eq!(
            tools
                .registry
                .dispatch(&ctx, "/where", &Snapshot::default())
                .unwrap()
                .message,
            "43"
        );
        std::fs::write(&file, "fn command( {").unwrap();
        tools.request_reloads().unwrap();
        drain(&mut tools);
        assert!(
            tools.loaded[&file.canonicalize().unwrap()]
                .last_error
                .is_some()
        );
        assert_eq!(
            tools
                .registry
                .dispatch(&ctx, "/where", &Snapshot::default())
                .unwrap()
                .message,
            "43"
        );
        std::fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn failure_bundle_is_structured_and_headless() {
        let path = std::env::temp_dir().join(format!("dx1-failure-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("fail.rhai"), "assert_tick(5);").unwrap();
        let mut tools = DevTools::new(&path, rustcraft_control::engine_registry()).unwrap();
        let mut host = TestHost;
        tools.start("fail.rhai", &mut host).unwrap();
        let started = Instant::now();
        let bundle = loop {
            if let Some(bundle) = tools.advance(&mut host, false).unwrap() {
                break bundle;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            std::thread::sleep(Duration::from_millis(1));
        };
        for file in [
            "result.json",
            "state.json",
            "scripts.json",
            "recent-events.jsonl",
            "scenario.log",
            "console.log",
        ] {
            assert!(bundle.join(file).exists(), "{file}");
        }
        let result: serde_json::Value =
            serde_json::from_slice(&std::fs::read(bundle.join("result.json")).unwrap()).unwrap();
        assert_eq!(result["status"], "fail");
        assert_eq!(result["failed_step"], 0);
        assert!(!bundle.join("frame.png").exists());
        assert!(!bundle.join("renderer.json").exists());
        std::fs::remove_dir_all(bundle).unwrap();
        std::fs::remove_dir_all(path).unwrap();
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    #[test]
    fn deadline_interrupts_and_print_is_bounded() {
        let mut runtime = RhaiRuntime::new(
            Context::read_only(rustcraft_control::Source::Script),
            Limits {
                operations: 1_000_000,
                deadline: Duration::from_millis(1),
            },
        );
        let mut session = RhaiSession::new("deadline");
        let start = Instant::now();
        let error = runtime
            .eval(&mut session, "loop {}", Snapshot::default())
            .unwrap_err();
        assert!(error.contains("deadline"), "{error}");
        assert!(start.elapsed() < Duration::from_secs(1));
        runtime = RhaiRuntime::new(
            Context::read_only(rustcraft_control::Source::Script),
            Limits {
                operations: 50000,
                deadline: Duration::from_millis(50),
            },
        );
        runtime
            .eval(&mut session, "print(\"hello\"); 42", Snapshot::default())
            .unwrap();
        assert_eq!(session.diagnostic.output, ["hello"]);
    }
    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_denied() {
        let directory = std::env::temp_dir().join(format!("dx1-symlink-{}", std::process::id()));
        std::fs::create_dir_all(directory.join("root")).unwrap();
        std::fs::write(directory.join("outside.rhai"), "42").unwrap();
        std::os::unix::fs::symlink(
            directory.join("outside.rhai"),
            directory.join("root/link.rhai"),
        )
        .unwrap();
        let root = ScriptRoot::new(directory.join("root")).unwrap();
        assert!(root.read("link.rhai").unwrap_err().contains("outside"));
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn unicode_cursor_preserves_boundaries() {
        let directory = std::env::temp_dir().join(format!("dx1-console-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let mut tools = DevTools::new(&directory, rustcraft_control::engine_registry()).unwrap();
        tools.insert("aλz");
        tools.left();
        tools.backspace();
        assert_eq!(tools.line, "az");
        assert_eq!(tools.cursor, 1);
        tools.right();
        assert_eq!(tools.cursor, 2);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

fn command_spec(
    runtime: &mut RhaiRuntime,
    ast: &AST,
) -> ControlResult<rustcraft_control::CommandSpec> {
    if !ast
        .iter_functions()
        .any(|f| f.name == "command" && f.params.len() == 1)
    {
        return Err("command script needs command(args)".into());
    }
    let (value, actions) = runtime.call(
        &mut RhaiSession::new("metadata"),
        ast,
        "command_spec",
        vec![],
        Snapshot::default(),
    )?;
    if !actions.is_empty() {
        return Err("command_spec must be pure metadata".into());
    }
    let map = value
        .try_cast::<rhai::Map>()
        .ok_or("command_spec must return a map")?;
    let text = |name: &str| {
        map.get(name)
            .and_then(|v| v.clone().try_cast::<rhai::ImmutableString>())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("command_spec missing {name}"))
    };
    Ok(rustcraft_control::CommandSpec {
        id: text("id")?,
        name: text("name")?,
        aliases: vec![],
        usage: text("usage")?,
        help: text("help")?,
        capability: text("capability")?,
    })
}

fn semantic_value(v: &serde_json::Value, depth: usize) -> Dynamic {
    if depth > 8 {
        return Dynamic::UNIT;
    }
    match v {
        serde_json::Value::Null => Dynamic::UNIT,
        serde_json::Value::Bool(b) => Dynamic::from(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Dynamic::from)
            .unwrap_or_else(|| Dynamic::from_float(n.as_f64().unwrap_or_default())),
        serde_json::Value::String(s) => Dynamic::from(s.chars().take(8192).collect::<String>()),
        serde_json::Value::Array(a) => Dynamic::from(
            a.iter()
                .take(1024)
                .map(|v| semantic_value(v, depth + 1))
                .collect::<rhai::Array>(),
        ),
        serde_json::Value::Object(o) => Dynamic::from(
            o.iter()
                .take(128)
                .map(|(k, v)| (k.clone().into(), semantic_value(v, depth + 1)))
                .collect::<rhai::Map>(),
        ),
    }
}
