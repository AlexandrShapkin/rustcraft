//! Existing engine operational policy inventory and startup compatibility adapter.
use crate::*;
pub const FONT_SCALE: &str = "rustcraft:ui/font_scale";
pub const LOAD_RADIUS: &str = "rustcraft:streaming/load_radius";
pub const RETAIN_RADIUS: &str = "rustcraft:streaming/retain_radius";
pub const LOOKAHEAD: &str = "rustcraft:streaming/lookahead";
pub const DIAGNOSTIC_MS: &str = "rustcraft:diagnostics/sample_interval_ms";
pub const PLAYER_SAVE_MS: &str = "rustcraft:persistence/player_interval_ms";
pub const WORLD_SAVE_MS: &str = "rustcraft:persistence/world_interval_ms";
pub const UPLOAD_SECTIONS: &str = "rustcraft:meshing/upload_sections";
pub const UPLOAD_BYTES: &str = "rustcraft:meshing/upload_bytes";
pub const LIGHT_WORK: &str = "rustcraft:lighting/work_budget";
pub const STREAM_MS: &str = "rustcraft:streaming/main_budget_ms";
pub const MESH_WORKERS: &str = "rustcraft:meshing/workers";
pub const LIGHT_WORKERS: &str = "rustcraft:lighting/workers";
pub const PRESENT: &str = "rustcraft:renderer/present_policy";
pub const SCRIPT_POLL_MS: &str = "rustcraft:scripts/reload_interval_ms";
pub fn engine(graphical: bool) -> Registry {
    let mut r = Registry::default();
    let mesh = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .saturating_sub(5)
        .clamp(1, 3) as i64;
    let defs = [
        (
            FONT_SCALE,
            Kind::Float { min: 0.5, max: 3. },
            Value::Float(1.),
            Policy::NextFrame,
            "Shared text pixel scale; resource-family replacement is startup-only.",
            "scale",
        ),
        (
            LOAD_RADIUS,
            Kind::Integer { min: 3, max: 12 },
            Value::Integer(4),
            Policy::NextTick,
            "Desired column radius; retain must be at least load.",
            "columns",
        ),
        (
            RETAIN_RADIUS,
            Kind::Integer { min: 3, max: 16 },
            Value::Integer(5),
            Policy::NextTick,
            "Retained column radius; existing persistence/entity pins remain authoritative.",
            "columns",
        ),
        (
            LOOKAHEAD,
            Kind::Bool,
            Value::Bool(false),
            Policy::NextTick,
            "Existing directional streaming priority policy.",
            "",
        ),
        (
            DIAGNOSTIC_MS,
            Kind::DurationMs { min: 50, max: 5000 },
            Value::DurationMs(250),
            Policy::Immediate,
            "Demand-driven shared diagnostic and developer text cadence.",
            "ms",
        ),
        (
            PLAYER_SAVE_MS,
            Kind::DurationMs {
                min: 1000,
                max: 2000,
            },
            Value::DurationMs(2000),
            Policy::NextTick,
            "Player checkpoint interval; preserves accepted two-second upper durability window.",
            "ms",
        ),
        (
            WORLD_SAVE_MS,
            Kind::DurationMs {
                min: 100,
                max: 60000,
            },
            Value::DurationMs(2000),
            Policy::NextTick,
            "World-state checkpoint interval; dirty state and flush are independent.",
            "ms",
        ),
        (
            UPLOAD_SECTIONS,
            Kind::Integer { min: 1, max: 64 },
            Value::Integer(4),
            Policy::NextFrame,
            "Maximum completed sections admitted per upload turn; queued meshes are retained.",
            "sections",
        ),
        (
            UPLOAD_BYTES,
            Kind::Bytes {
                min: 262144,
                max: 67108864,
            },
            Value::Bytes(8388608),
            Policy::NextFrame,
            "Existing upload byte budget; oversized first mesh can progress.",
            "bytes",
        ),
        (
            LIGHT_WORK,
            Kind::Integer { min: 1, max: 256 },
            Value::Integer(32),
            Policy::NextTick,
            "Boundary lighting work units; changing budget retains the queue.",
            "units",
        ),
        (
            STREAM_MS,
            Kind::Float {
                min: 0.25,
                max: 8.0,
            },
            Value::Float(2.0),
            Policy::NextFrame,
            "Existing main-thread streaming service time budget.",
            "ms",
        ),
        (
            MESH_WORKERS,
            Kind::Integer { min: 1, max: 32 },
            Value::Integer(mesh),
            Policy::RestartRequired,
            "Mesh workers; no safe live pool drain/rebuild protocol.",
            "workers",
        ),
        (
            LIGHT_WORKERS,
            Kind::Integer { min: 1, max: 4 },
            Value::Integer(1),
            Policy::RestartRequired,
            "Initial lighting workers; no live pool resize.",
            "workers",
        ),
        (
            PRESENT,
            Kind::Enum {
                choices: vec!["PreferFifo".into()],
            },
            Value::Text("PreferFifo".into()),
            Policy::RestartRequired,
            "Existing prefer-Fifo/fallback-first-supported surface policy. Selected mode is in Renderer diagnostics; P1 owns alternatives.",
            "",
        ),
        (
            SCRIPT_POLL_MS,
            Kind::DurationMs {
                min: 100,
                max: 5000,
            },
            Value::DurationMs(500),
            Policy::Immediate,
            "Trusted script reload polling cadence; execution/security limits are unchanged.",
            "ms",
        ),
        (
            "rustcraft:identity/chunk_width",
            Kind::Integer { min: 16, max: 16 },
            Value::Integer(16),
            Policy::ImmutableAfterOpen,
            "Persisted structural chunk dimensions cannot change in an open world.",
            "blocks",
        ),
        (
            "rustcraft:identity/control_version",
            Kind::Integer { min: 1, max: 1 },
            Value::Integer(1),
            Policy::ImmutableAfterOpen,
            "Stable Control API encoding; not a runtime operational knob.",
            "",
        ),
    ];
    for (key, kind, default, policy, description, unit) in defs {
        let local = key == FONT_SCALE
            || key.contains("meshing/")
            || key.contains("renderer/")
            || key == LOOKAHEAD
            || key == LIGHT_WORK
            || key == PLAYER_SAVE_MS
            || key == WORLD_SAVE_MS;
        r.register(Spec {
            key: key.into(),
            kind,
            default,
            owner: "rustcraft:engine".into(),
            description: description.into(),
            unit: unit.into(),
            policy,
            persist: policy != Policy::ImmutableAfterOpen,
            reason: if matches!(policy, Policy::ImmutableAfterOpen | Policy::RestartRequired) {
                description.into()
            } else {
                String::new()
            },
            availability: if local && !graphical {
                "unavailable: no native consumer in headless bootstrap".into()
            } else {
                "available".into()
            },
        })
        .unwrap();
    }
    r
}
pub const ALIASES: [(&str, &str, u64); 11] = [
    ("RUSTCRAFT_STREAM_RADIUS", LOAD_RADIUS, 1),
    ("RUSTCRAFT_STREAM_RETAIN_RADIUS", RETAIN_RADIUS, 1),
    ("RUSTCRAFT_STREAM_LOOKAHEAD", LOOKAHEAD, 1),
    ("RUSTCRAFT_PLAYER_AUTOSAVE_SECONDS", PLAYER_SAVE_MS, 1000),
    ("RUSTCRAFT_WORLD_AUTOSAVE_SECONDS", WORLD_SAVE_MS, 1000),
    ("RUSTCRAFT_MESH_UPLOAD_SECTIONS", UPLOAD_SECTIONS, 1),
    ("RUSTCRAFT_MESH_UPLOAD_BYTES", UPLOAD_BYTES, 1),
    ("RUSTCRAFT_LIGHTING_WORK_BUDGET", LIGHT_WORK, 1),
    ("RUSTCRAFT_STREAM_MAIN_BUDGET_MS", STREAM_MS, 1),
    ("RUSTCRAFT_MESH_WORKERS", MESH_WORKERS, 1),
    ("RUSTCRAFT_LIGHT_WORKERS", LIGHT_WORKERS, 1),
];
pub fn load(
    r: &mut Registry,
    args: &[String],
    env: impl Fn(&str) -> Option<String>,
    path: PathBuf,
) -> Result<()> {
    let path = args
        .iter()
        .position(|a| a == "--config-file")
        .map(|i| {
            args.get(i + 1)
                .map(PathBuf::from)
                .ok_or("--config-file needs PATH")
        })
        .transpose()?
        .unwrap_or(path);
    r.load_file(path)?;
    let mut inputs = Vec::new();
    for (name, key, factor) in ALIASES {
        if let Some(mut text) = env(name) {
            if factor != 1 {
                let n: u64 = text
                    .parse()
                    .map_err(|_| format!("{name}: invalid seconds"))?;
                text = n
                    .checked_mul(factor)
                    .ok_or("duration overflow")?
                    .to_string();
            }
            let value = r
                .spec(key)?
                .kind
                .parse(&text)
                .map_err(|e| format!("{name}: {e}"))?;
            inputs.push((key.into(), value));
        }
    }
    if inputs.iter().any(|(k, _)| k == LOAD_RADIUS)
        && !inputs.iter().any(|(k, _)| k == RETAIN_RADIUS)
        && r.entries[RETAIN_RADIUS].layers.is_empty()
    {
        let n = inputs
            .iter()
            .find(|(k, _)| k == LOAD_RADIUS)
            .unwrap()
            .1
            .integer();
        inputs.push((RETAIN_RADIUS.into(), Value::Integer(n + 1)));
    }
    r.seed(Source::Environment, &inputs)?;
    let mut cli = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--set-config" {
            let pair = args.get(i + 1).ok_or("--set-config needs KEY=VALUE")?;
            let (k, v) = pair.split_once('=').ok_or("--set-config needs KEY=VALUE")?;
            cli.push((
                k.into(),
                r.spec(k)?
                    .kind
                    .parse(v)
                    .map_err(|e| format!("{k} from CLI: {e}"))?,
            ));
        }
    }
    r.seed(Source::Cli, &cli)?;
    r.open();
    Ok(())
}
