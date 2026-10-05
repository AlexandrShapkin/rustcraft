//! Native developer views and bounded, demand-driven shared observations. No game or UI dependency.
use crate::{Action, ControlResult, ControlState, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Presentation,
    Residency,
    World,
    Entities,
    Streaming,
    Lighting,
    Meshing,
    Renderer,
    Persistence,
    Scripts,
    Chunk,
    Entity,
    Overlays,
}
impl Domain {
    pub const ALL: [Self; 13] = [
        Self::Presentation,
        Self::Residency,
        Self::World,
        Self::Entities,
        Self::Streaming,
        Self::Lighting,
        Self::Meshing,
        Self::Renderer,
        Self::Persistence,
        Self::Scripts,
        Self::Chunk,
        Self::Entity,
        Self::Overlays,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::Presentation => "presentation",
            Self::Residency => "residency",
            Self::World => "world",
            Self::Entities => "entities",
            Self::Streaming => "streaming",
            Self::Lighting => "lighting",
            Self::Meshing => "meshing",
            Self::Renderer => "renderer",
            Self::Persistence => "persistence",
            Self::Scripts => "scripts",
            Self::Chunk => "chunk",
            Self::Entity => "entity",
            Self::Overlays => "overlays",
        }
    }
    pub fn value(self, s: &Snapshot) -> &Value {
        match self {
            Self::Presentation => &s.presentation,
            Self::Residency => &s.residency,
            Self::World => &s.world,
            Self::Entities => &s.entities,
            Self::Streaming => &s.streaming,
            Self::Lighting => &s.lighting,
            Self::Meshing => &s.meshing,
            Self::Renderer => &s.renderer,
            Self::Persistence => &s.persistence,
            Self::Scripts => &s.scripts,
            Self::Chunk => &s.chunk,
            Self::Entity => &s.entity,
            Self::Overlays => &s.overlay_geometry,
        }
    }
    pub fn set(self, s: &mut Snapshot, v: Value) {
        match self {
            Self::Presentation => s.presentation = v,
            Self::Residency => s.residency = v,
            Self::World => s.world = v,
            Self::Entities => s.entities = v,
            Self::Streaming => s.streaming = v,
            Self::Lighting => s.lighting = v,
            Self::Meshing => s.meshing = v,
            Self::Renderer => s.renderer = v,
            Self::Persistence => s.persistence = v,
            Self::Scripts => s.scripts = v,
            Self::Chunk => s.chunk = v,
            Self::Entity => s.entity = v,
            Self::Overlays => s.overlay_geometry = v,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    Page,
    Overlay,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Cost {
    Low,
    Medium,
    High,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugView {
    pub id: String,
    pub name: String,
    pub title: String,
    pub description: String,
    pub owner: String,
    pub kind: ViewKind,
    pub cost: Cost,
    pub shortcut: Option<String>,
    pub requirements: Vec<Domain>,
}
#[derive(Debug, Default)]
pub struct ViewRegistry {
    entries: BTreeMap<String, DebugView>,
}
impl ViewRegistry {
    pub fn register(&mut self, view: DebugView) -> ControlResult<()> {
        let semantic = |s: &str| {
            s.len() <= 128
                && s.split_once(':').is_some_and(|(n, p)| {
                    !n.is_empty()
                        && !p.is_empty()
                        && n.bytes()
                            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
                        && p.bytes().all(|b| {
                            b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-/".contains(&b)
                        })
                })
        };
        if !semantic(&view.id)
            || !semantic(&view.owner)
            || view.name.is_empty()
            || view.name.len() > 32
            || !view
                .name
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'_')
            || view.title.len() > 64
            || view.description.len() > 256
            || view.requirements.len() > 8
            || view.shortcut.as_ref().is_some_and(|s| s.len() > 64)
        {
            return Err("invalid bounded debug view metadata".into());
        }
        if self.entries.len() >= 64
            || self.entries.contains_key(&view.id)
            || self
                .entries
                .values()
                .any(|v| v.name == view.name && v.kind == view.kind)
        {
            return Err("duplicate debug identity/name or registry full".into());
        }
        self.entries.insert(view.id.clone(), view);
        Ok(())
    }
    pub fn views(&self, kind: ViewKind) -> impl Iterator<Item = &DebugView> {
        self.entries.values().filter(move |v| v.kind == kind)
    }
    pub fn get(&self, name: &str, kind: ViewKind) -> Option<&DebugView> {
        self.entries
            .values()
            .find(|v| v.kind == kind && (v.name == name || v.id == name))
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn engine() -> Self {
        let mut r = Self::default();
        for (name, title, description, cost, requirements) in [
            (
                "presentation",
                "Presentation",
                "Application timing, transform duplication and configured refresh. Physical scanout unavailable.",
                Cost::Medium,
                vec![Domain::Presentation],
            ),
            (
                "memory",
                "Residency / Memory",
                "Scalar lifetime ledger: live ownership, retired work, reusable capacity and provider scope.",
                Cost::Medium,
                vec![Domain::Residency],
            ),
            (
                "overview",
                "Overview",
                "F3 overview; F4 selector; backquote console; F10 abort. No broad diagnostic scans.",
                Cost::Low,
                vec![],
            ),
            (
                "streaming",
                "Streaming",
                "Desired/Retained/Safe/Visible, queues and forward frontier blockers.",
                Cost::Medium,
                vec![Domain::Streaming, Domain::Lighting, Domain::Meshing],
            ),
            (
                "world",
                "World",
                "Semantic nearby voxels, generator, time and resident/dirty summaries.",
                Cost::Medium,
                vec![Domain::World],
            ),
            (
                "entities",
                "Entities",
                "Up to 64 active semantic entities, eight per page. E selects a stable EntityId.",
                Cost::Medium,
                vec![Domain::Entities],
            ),
            (
                "settings",
                "Runtime settings",
                "Typed operational policy. Left/right choose; +/- change; R reset; console /config supports exact values and batches.",
                Cost::Low,
                vec![],
            ),
            (
                "lighting",
                "Lighting",
                "Initial/boundary lighting progress. No full voxel scan.",
                Cost::Low,
                vec![Domain::Lighting],
            ),
            (
                "meshing",
                "Meshing",
                "Queue/snapshot bytes, stale replies and generation-history count; no lifetime-policy change.",
                Cost::Medium,
                vec![Domain::Meshing],
            ),
            (
                "renderer",
                "Renderer",
                "Adapter/surface/present mode and logical/capacity bytes. Not physical display timing or VRAM.",
                Cost::High,
                vec![Domain::Renderer],
            ),
            (
                "persistence",
                "Persistence",
                "Dirty/pending writes, checkpoint revisions and failures. No storage mutation.",
                Cost::Medium,
                vec![Domain::Persistence],
            ),
            (
                "scripts",
                "Scripts / Scenarios / Jobs",
                "Loaded generations, scenario step, bounded jobs and recent errors.",
                Cost::Medium,
                vec![Domain::Scripts],
            ),
            (
                "chunk",
                "Selected chunk / section",
                "C selects crosshair/player section; /debug chunk X Z Y selects coordinates. Why visible/retained/blocked.",
                Cost::Medium,
                vec![Domain::Chunk],
            ),
            (
                "entity",
                "Selected entity",
                "E cycles the bounded active list; /debug entity HEX_ID selects identity. Missing/unloaded is contextual.",
                Cost::Medium,
                vec![Domain::Entity],
            ),
        ] {
            r.register(DebugView {
                id: format!("rustcraft:debug/page/{name}"),
                name: name.into(),
                title: title.into(),
                description: description.into(),
                owner: "rustcraft:engine".into(),
                kind: ViewKind::Page,
                cost,
                shortcut: match name {
                    "overview" => Some("F3"),
                    "streaming" => Some("F3+1"),
                    "world" => Some("F3+2"),
                    "entities" => Some("F3+3"),
                    "lighting" => Some("F3+4"),
                    "meshing" => Some("F3+5"),
                    "renderer" => Some("F3+6"),
                    "persistence" => Some("F3+7"),
                    "scripts" => Some("F3+8"),
                    "settings" => Some("F3+9"),
                    _ => None,
                }
                .map(str::to_owned),
                requirements,
            })
            .unwrap();
        }
        for (name, title, description, cost, requirements) in [
            (
                "streaming",
                "Residency",
                "5x5 nearby columns: green Visible, yellow Safe, red Desired/unready, blue Retained, grey outside.",
                Cost::Medium,
                vec![Domain::Overlays],
            ),
            (
                "entities",
                "Entity markers / owners",
                "Up to twelve active entities and their owner columns; transient diagnostic boxes.",
                Cost::Medium,
                vec![Domain::Entities, Domain::Overlays],
            ),
            (
                "collision",
                "Player collision",
                "Sampled player collision AABB (yellow).",
                Cost::Low,
                vec![Domain::Overlays],
            ),
            (
                "target",
                "Raycast target",
                "Sampled semantic ray target voxel (white).",
                Cost::Low,
                vec![Domain::Overlays],
            ),
            (
                "chunks",
                "Selected section boundary",
                "Selected chunk section boundary (cyan); selection is diagnostic only.",
                Cost::Low,
                vec![Domain::Chunk, Domain::Overlays],
            ),
            (
                "selected_entity",
                "Selected entity / owner",
                "Stable selected entity AABB (orange) and owner column (blue); never reused by list index.",
                Cost::Medium,
                vec![Domain::Entity, Domain::Overlays],
            ),
        ] {
            r.register(DebugView {
                id: format!("rustcraft:debug/overlay/{name}"),
                name: name.into(),
                title: title.into(),
                description: description.into(),
                owner: "rustcraft:engine".into(),
                kind: ViewKind::Overlay,
                cost,
                shortcut: None,
                requirements,
            })
            .unwrap();
        }
        r
    }
}
#[derive(Debug, Default)]
pub struct Sample {
    pub collections: u64,
    pub requests: u64,
    pub last_us: u128,
    pub max_us: u128,
    pub total_us: u128,
    pub at: Option<Instant>,
}
/// Four Hz is the accepted DX1 diagnostic cadence; only requested domains use it. Selection changes
/// invalidate the selected-domain sample. Cheap player/tick state remains current in Host::snapshot.
#[derive(Debug)]
pub struct Diagnostics {
    pub registry: ViewRegistry,
    pub samples: BTreeMap<Domain, Sample>,
    pub managed: bool,
    pub cadence: Duration,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            registry: ViewRegistry::engine(),
            samples: Default::default(),
            managed: false,
            cadence: Self::CADENCE,
        }
    }
}
impl Diagnostics {
    pub const CADENCE: Duration = Duration::from_millis(250);
    pub fn due(&mut self, d: Domain, now: Instant) -> bool {
        let s = self.samples.entry(d).or_default();
        s.requests += 1;
        s.at.is_none_or(|at| now.saturating_duration_since(at) >= self.cadence)
    }
    pub fn collected(&mut self, d: Domain, started: Instant) {
        let s = self.samples.entry(d).or_default();
        s.collections += 1;
        s.last_us = started.elapsed().as_micros();
        s.total_us += s.last_us;
        s.max_us = s.max_us.max(s.last_us);
        s.at = Some(Instant::now());
    }
    pub fn invalidate(&mut self, d: Domain) {
        if let Some(s) = self.samples.get_mut(&d) {
            s.at = None;
        }
    }
    pub fn metrics(&self) -> Value {
        Value::Object(
            self.samples
                .iter()
                .map(|(d, s)| {
                    (
                        d.key().into(),
                        json!({"collections":s.collections,
            "requests":s.requests,
            "last_us":s.last_us,
            "max_us":s.max_us,
            "total_us":s.total_us,
            "age_ms":s.at.map(|t|t.elapsed().as_millis()),
            "cadence_ms":self.cadence.as_millis(),
            "stale":s.at.is_none_or(|t|t.elapsed()>self.cadence*2)}),
                    )
                })
                .collect(),
        )
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DebugInput {
    Open,
    Close,
    Next,
    Previous,
    Tab,
    Activate,
    Help,
    TargetChunk,
    NextEntity,
    SettingNext,
    SettingPrevious,
    SettingIncrease,
    SettingDecrease,
    SettingReset,
}
#[derive(Debug)]
pub struct Selector {
    pub open: bool,
    pub kind: ViewKind,
    pub selected: String,
    pub help: bool,
}
impl Default for Selector {
    fn default() -> Self {
        Self {
            open: false,
            kind: ViewKind::Page,
            selected: "rustcraft:debug/page/overview".into(),
            help: false,
        }
    }
}
impl ControlState {
    pub fn view_action(&mut self, a: &Action) -> ControlResult<()> {
        match a {
            Action::DebugPage(n) => {
                self.page = self
                    .diagnostics
                    .registry
                    .get(n, ViewKind::Page)
                    .ok_or("unknown registered page")?
                    .name
                    .clone()
            }
            Action::Overlay { name, enabled } => {
                let n = self
                    .diagnostics
                    .registry
                    .get(name, ViewKind::Overlay)
                    .ok_or("unknown registered overlay")?
                    .name
                    .clone();
                if *enabled {
                    self.overlays.insert(n);
                } else {
                    self.overlays.remove(&n);
                }
                self.diagnostics.invalidate(Domain::Overlays);
            }
            Action::InspectChunk { x, z, section_y } => {
                self.selected_chunk = Some([*x, *z, *section_y]);
                self.page = "chunk".into();
                self.diagnostics.invalidate(Domain::Chunk);
                self.diagnostics.invalidate(Domain::Overlays);
            }
            Action::InspectEntity(id) => {
                self.selected_entity = Some(id.to_ascii_lowercase());
                self.page = "entity".into();
                self.diagnostics.invalidate(Domain::Entity);
                self.diagnostics.invalidate(Domain::Overlays);
            }
            Action::DebugUi(input) => self.selector_input(*input)?,
            _ => return Err("not a view action".into()),
        }
        Ok(())
    }
    pub fn selector_input(&mut self, input: DebugInput) -> ControlResult<()> {
        if matches!(
            input,
            DebugInput::SettingNext
                | DebugInput::SettingPrevious
                | DebugInput::SettingIncrease
                | DebugInput::SettingDecrease
                | DebugInput::SettingReset
        ) {
            return self.setting_input(input);
        }
        match input {
            DebugInput::Open => self.selector.open = true,
            DebugInput::Close => self.selector.open = false,
            DebugInput::Help => self.selector.help = !self.selector.help,
            DebugInput::Tab => {
                self.selector.kind = if self.selector.kind == ViewKind::Page {
                    ViewKind::Overlay
                } else {
                    ViewKind::Page
                };
                self.selector.selected = self
                    .diagnostics
                    .registry
                    .views(self.selector.kind)
                    .next()
                    .map_or(String::new(), |v| v.id.clone());
            }
            DebugInput::Next | DebugInput::Previous => {
                let entries = self
                    .diagnostics
                    .registry
                    .views(self.selector.kind)
                    .collect::<Vec<_>>();
                if !entries.is_empty() {
                    let i = entries
                        .iter()
                        .position(|v| v.id == self.selector.selected)
                        .unwrap_or(0);
                    let i = if input == DebugInput::Next {
                        (i + 1) % entries.len()
                    } else {
                        (i + entries.len() - 1) % entries.len()
                    };
                    self.selector.selected = entries[i].id.clone();
                }
            }
            DebugInput::Activate => {
                let v = self
                    .diagnostics
                    .registry
                    .get(&self.selector.selected, self.selector.kind)
                    .ok_or("no selected view")?;
                let a = if v.kind == ViewKind::Page {
                    Action::DebugPage(v.name.clone())
                } else {
                    Action::Overlay {
                        name: v.name.clone(),
                        enabled: !self.overlays.contains(&v.name),
                    }
                };
                self.view_action(&a)?;
            }
            DebugInput::TargetChunk => {
                self.page = "chunk".into();
                self.selected_chunk = None;
                self.diagnostics.invalidate(Domain::Chunk);
                self.diagnostics.invalidate(Domain::Overlays);
            }
            DebugInput::SettingNext
            | DebugInput::SettingPrevious
            | DebugInput::SettingIncrease
            | DebugInput::SettingDecrease
            | DebugInput::SettingReset => unreachable!(),
            DebugInput::NextEntity => {
                let entries = self
                    .domains
                    .entities
                    .as_array()
                    .ok_or("entities unavailable; select Entities and refresh first")?;
                let pos = entries
                    .iter()
                    .position(|v| v["id"].as_str() == self.selected_entity.as_deref())
                    .map_or(0, |p| p + 1);
                let id = entries
                    .get(pos % entries.len().max(1))
                    .and_then(|v| v["id"].as_str())
                    .ok_or("no active entities in bounded list")?
                    .to_owned();
                self.view_action(&Action::InspectEntity(id))?;
            }
        }
        Ok(())
    }
    pub fn diagnostic_demand(&self) -> Vec<Domain> {
        let mut domains = std::collections::BTreeSet::new();
        if let Some(v) = self.diagnostics.registry.get(&self.page, ViewKind::Page) {
            domains.extend(v.requirements.iter().copied());
        }
        for n in &self.overlays {
            if let Some(v) = self.diagnostics.registry.get(n, ViewKind::Overlay) {
                domains.extend(v.requirements.iter().copied());
            }
        }
        domains.into_iter().collect()
    }
    pub fn debug_metadata(&self) -> Value {
        json!({"selector_open":self.selector.open,
            "selected_view":self.selector.selected,
            "active_page":if self.page.is_empty(){"overview"}else{&self.page},
            "active_overlays":self.overlays,
            "selected_chunk":self.selected_chunk,
            "selected_entity":self.selected_entity,
            "views":self.diagnostics.registry.entries.values().map(|v|json!({"metadata":v,
            "availability":availability(v,&self.domains),
            "active":if v.kind==ViewKind::Page{self.page==v.name}else{self.overlays.contains(&v.name)}})).collect::<Vec<_>>(),
            "providers":self.diagnostics.metrics()})
    }
    pub fn diagnostic_text(&self) -> String {
        let mut text = format!(
            "DEVELOPER TOOLS F4 SELECTOR / ` CONSOLE / F3 OVERVIEW\nPAGE {} | OVERLAYS {}\n",
            if self.page.is_empty() {
                "overview"
            } else {
                &self.page
            },
            self.overlays.iter().cloned().collect::<Vec<_>>().join(",")
        );
        if self.page.is_empty() || self.page == "overview" {
            text.push_str(&format!("Tick {} | player {}\nDiagnostics requested on active pages; samples 4 Hz; F3 retains fast access\n",self.domains.tick,self.domains.player["position"]));
        }
        if self.page == "chunk" {
            let c = &self.domains.chunk;
            text.push_str(&format!("Chunk {} section {} | resident {} desired {} retained {}\nSafe {} Visible {} Dirty {} Save pin {}\nWhy: {}\n",c["chunk"],c["section_y"],c["resident"],c["desired"],c["retained_radius"],c["safe"],c["visible"],c["dirty"],c["save_pin"],c["why"]));
        }
        if self.page == "entity" {
            let e = &self.domains.entity;
            text.push_str(&format!(
                "Entity {} | {}\npos {} vel {} owner {}\nstatus {} {}\n",
                e["id"],
                e["type"],
                e["position"],
                e["velocity"],
                e["owner_chunk"],
                e["status"],
                e["unavailable"]
            ));
        }
        if self.selector.open {
            text.push_str("UP/DOWN select | TAB pages/overlays | ENTER toggle/select | H help\nC target chunk | E next entity | ESC/F4 close | ` console\n");
            let views = self
                .diagnostics
                .registry
                .views(self.selector.kind)
                .collect::<Vec<_>>();
            let pos = views
                .iter()
                .position(|v| v.id == self.selector.selected)
                .unwrap_or(0);
            let start = pos.saturating_sub(5);
            for v in views.iter().skip(start).take(8) {
                let active = if v.kind == ViewKind::Page {
                    self.page == v.name
                } else {
                    self.overlays.contains(&v.name)
                };
                text.push_str(&format!(
                    "{} {} {} [{:?}] {}\n",
                    if v.id == self.selector.selected {
                        ">"
                    } else {
                        " "
                    },
                    if active { "ON" } else { "--" },
                    v.title,
                    v.cost,
                    availability(v, &self.domains)
                ));
            }
            if let Some(v) = self
                .diagnostics
                .registry
                .get(&self.selector.selected, self.selector.kind)
            {
                text.push_str(&format!(
                    "{}\nOWNER {} | ID {}\n{}\n",
                    v.title, v.owner, v.id, v.description
                ));
                if self.selector.help {
                    text.push_str(&format!(
                        "Shortcut {} | required {:?}\n",
                        v.shortcut.as_deref().unwrap_or("selector"),
                        v.requirements
                    ));
                }
            }
        }
        if let Some(v) = self.diagnostics.registry.get(&self.page, ViewKind::Page) {
            for d in &v.requirements {
                let m = self.diagnostics.samples.get(d);
                text.push_str(&format!(
                    "{} sampled age={}ms count={} last={}us {}\n",
                    d.key(),
                    m.and_then(|s| s.at)
                        .map_or("N/A".into(), |t| t.elapsed().as_millis().to_string()),
                    m.map_or(0, |s| s.collections),
                    m.map_or(0, |s| s.last_us),
                    if m.and_then(|s| s.at)
                        .is_none_or(|t| t.elapsed() > self.diagnostics.cadence * 2)
                    {
                        "STALE/UNSAMPLED"
                    } else {
                        ""
                    }
                ));
            }
            let d = v.requirements.first().copied();
            if let Some(d) = d {
                let value = d.value(&self.domains);
                if value.is_null() {
                    text.push_str("UNAVAILABLE: provider absent or not yet sampled\n");
                } else if d == Domain::World && self.page == "world" {
                    text.push_str(&format!("time {} | player chunk {}\ngenerator {}\nresident columns {} | dirty sections {}\ntarget {}\nblock_query_radius: {}\n",value["time"],value["player_chunk"],value["generator"],value["resident_columns"],value["dirty_sections"],value["target"],value["block_query_radius"]));
                    let blocks = value["blocks"].as_object();
                    text.push_str(&format!(
                        "Nearby blocks: showing up to 8 of {} (query cap 27)\n",
                        blocks.map_or(0, |b| b.len())
                    ));
                    if let Some(blocks) = blocks {
                        for (p, k) in blocks.iter().take(8) {
                            text.push_str(&format!("{p}: {k}\n"));
                        }
                    }
                } else if d == Domain::Persistence {
                    let io = &value["column_io"];
                    text.push_str(&format!("Dirty {} | oldest {} ms | queued {} inflight {} failures {}\nQueue wait total {} / max {} us\nStale acknowledgements {} | coalesced dirty mutations {}\n",value["dirty"],value["oldest_dirty_ms"],value["queued"],value["inflight"],value["failures"],value["queue_wait_total_us"],value["queue_wait_max_us"],value["stale_save_acks"],value["coalesced_dirty_mutations"]));
                    text.push_str(&format!("Column writes {} | raw {} / application writes {} bytes\nEncode {} | compression {} us\nWrite {} | sync/replace envelope {} us\nSuccessful columns since open; not device writes or isolated fsync\n",io["columns"],io["raw_bytes"],io["application_write_bytes"],io["encode_us"],io["compression_us"],io["write_us"],io["durability_envelope_us"]));
                    text.push_str(&format!("Player revision {} persisted {} | dirty {} inflight {} failures {}\nWorld revision {} persisted {} | dirty {} inflight {} failures {}\n",value["player_revision"],value["player_persisted"],value["player_dirty"],value["player_inflight"],value["player_failures"],value["world_revision"],value["world_persisted"],value["world_dirty"],value["world_inflight"],value["world_failures"]));
                    for entry in value["dirty_subset"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .take(3)
                    {
                        text.push_str(&format!(
                            "Dirty chunk {} generation {} save pin {}\n",
                            entry["chunk"], entry["generation"], entry["saving"]
                        ));
                    }
                    text.push_str("Dirty subset: up to 3 shown; query/capture retains bounded provider subset\n");
                } else if d == Domain::Presentation {
                    if let Some(reason) = value["unavailable"].as_str() {
                        text.push_str(&format!("UNAVAILABLE: {reason}\n"));
                    } else {
                        text.push_str(&format!("{} | {} | {}\nConfigured refresh {} mHz | target {} ms\nPhysical scanout / VRR: unavailable\n",value["backend"],value["adapter"],value["present_mode"],value["monitor"]["reported_refresh_millihertz"],value["target_ms"]));
                        text.push_str(
                            "Application timing: p50 / p95 / p99 / jitter(ms) / cadence misses\n",
                        );
                        for key in [
                            "render_ms",
                            "request_ms",
                            "redraw_ms",
                            "acquire_ms",
                            "prepare_ms",
                            "submit_ms",
                            "present_call_ms",
                            "state_age_ms",
                            "input_event_to_camera_ms",
                        ] {
                            let s = &value[key];
                            let n = |i: usize| {
                                s[i].as_f64()
                                    .map_or_else(|| "N/A".into(), |v| format!("{v:.3}"))
                            };
                            text.push_str(&format!(
                                "{key}: {} / {} / {} / {} / {}\n",
                                n(2),
                                n(3),
                                n(4),
                                n(6),
                                n(7)
                            ));
                        }
                        let tps = value["tick_interval_ms"][1]
                            .as_f64()
                            .filter(|v| *v > 0.)
                            .map_or_else(|| "N/A".into(), |ms| format!("{:.2}", 1000. / ms));
                        text.push_str(&format!("Frames {} ticks {} | target 20 TPS, measured {tps} | alpha {}\nDuplicate authority positions {} / shown positions {} / cameras {}\nCadence threshold >1.5 configured-refresh intervals; not scanout\n",value["frames"],value["ticks"],value["accumulator_alpha"],value["authoritative_position_duplicates"],value["presentation_position_duplicates"],value["camera_duplicates"]));
                    }
                } else if d == Domain::Residency {
                    if let Some(reason) = value["unavailable"].as_str() {
                        text.push_str(&format!("UNAVAILABLE: {reason}\n"));
                        return bounded_text(&text);
                    }
                    let w = &value["world"];
                    let m = &value["meshing"];
                    let g = &value["gpu"];
                    let r = &value["render"];
                    let e = &value["simulation"];
                    text.push_str(&format!("Settled {} | core ready {} | player chunk {}\nDesired {} Retained/live {} Safe {} Visible {}\nWorld columns {} sections {} | load {} generate {}\nDirty {} persistence pins {} lighting pins {} other/transient {}\nRender sections {} snapshots {} bytes {}\nEntities active {} durable {} tombstones {} receipts {}\nMesh metadata {} capacity {}\nMesh pending {} inflight {} completed {} ready {}\nReady bytes {} completed bytes {}\nJob snapshots {} bytes {} | stale {} coalesced {}\nGPU sections {} pages {} vertex/index buffers {}/{}\nGPU logical {} bytes | owned capacity {} bytes\nBuffers created {} reused {} retired {}\nGlobal text {} bytes / {} page\nRSS {} [{}] | process VRAM unavailable\n",value["idle"],value["core_ready"],value["player_chunk"],w["desired"],w["retained_resident"],w["safe"],w["visible"],w["resident_columns"],w["resident_sections"],w["pending_load"],w["pending_generation"],w["dirty"],w["persistence_pinned"],w["lighting_pinned"],w["other_or_transient_eviction_blocked"],r["sections"],r["snapshots"],r["snapshot_bytes"],e["active_entities"],e["durable_entities"],e["tombstones"],e["pickup_receipts"],m["generations"],m["generation_capacity"],m["pending"],m["inflight_submitted_unconsumed"],m["completed_unconsumed"],m["ready"],m["ready_bytes"],m["completed_bytes"],m["live_job_snapshots"],m["live_job_snapshot_bytes"],m["stale"],m["coalesced"],g["sections"],g["mesh_pages"],g["vertex_buffers"],g["index_buffers"],g["logical_bytes"],g["capacity_bytes"],g["created"],g["reused"],g["retired"],g["fixed_text_capacity_bytes"],g["fixed_text_pages"],value["process"]["rss_bytes"],value["process"]["status"]));
                    let light = &value["lighting_lifetime"];
                    text.push_str(&format!("Lighting sources {} retired {} | queued integration {} cleanup {}\nCleanup pressure {} | active {} queue {}\n",light["columns"],light["retired_source_columns"],light["queued_integration"],light["queued_cleanup"],light["cleanup_backpressured"],light["active_column"],light["active_queue"]));
                } else if d == Domain::Entities {
                    text.push_str(&format!(
                        "Active list: returned {} (cap 64); {}\n",
                        value.as_array().map_or(0, Vec::len),
                        self.domains.debug["entity_coverage"]
                    ));
                    for e in value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .skip(self.entity_offset)
                        .take(4)
                    {
                        text.push_str(&format!(
                            "{} {}\npos {} owner {} age {} rev {}\n",
                            e["id"],
                            e["type"],
                            e["position"],
                            e["owner_chunk"],
                            e["age"],
                            e["revision"]
                        ));
                    }
                } else {
                    text.push_str(&format_values(value, 0));
                    if d == Domain::Streaming {
                        text.push_str(&format!(
                            "Lighting {}\nMesh {}\n",
                            self.domains.lighting, self.domains.meshing
                        ));
                    }
                }
            }
        }
        if self.page == "settings" {
            text = self.settings_text(&text);
        }
        bounded_text(&text)
    }
}
/// Bound every native viewport and explicitly disclose row/column/character clipping.
pub fn bounded_text(text: &str) -> String {
    let clipped = text.lines().count() > 28
        || text.lines().any(|l| l.chars().count() > 110)
        || text.chars().count() > 2600;
    let body = text
        .lines()
        .take(if clipped { 27 } else { 28 })
        .map(|l| {
            let mut v = l.chars().take(110).collect::<String>();
            if l.chars().count() > 110 {
                v = l.chars().take(106).collect::<String>() + " ...";
            }
            v
        })
        .collect::<Vec<_>>()
        .join("\n");
    if clipped {
        body.chars().take(2500).collect::<String>()
            + "\nUI viewport truncated; shared queries/capture retain bounded values"
    } else {
        body
    }
}
fn availability(v: &DebugView, s: &Snapshot) -> String {
    if v.requirements.is_empty() {
        return "available".into();
    }
    if v.requirements.iter().any(|d| d.value(s).is_null()) {
        return "not sampled/provider unavailable".into();
    }
    if let Some(reason) = v
        .requirements
        .iter()
        .find_map(|d| d.value(s)["unavailable"].as_str())
    {
        return format!("unavailable: {reason}");
    }
    "available".into()
}
/// Compact bounded field formatting, shared by UI and package views; semantic JSON remains intact.
fn format_values(v: &Value, depth: usize) -> String {
    if depth > 2 {
        return format!("{v}\n");
    }
    match v {
        Value::Object(m) => m
            .iter()
            .take(24)
            .map(|(k, v)| {
                if v.is_object() {
                    format!("{k}: {}", format_values(v, depth + 1))
                } else {
                    format!("{k}: {v}\n")
                }
            })
            .collect(),
        _ => format!("{v}\n"),
    }
}
/// Bounded conservative query preflight. Also recognizes domain references inside REPL-defined
/// functions/commands. Over-demand from comments is safe; no filesystem or subsystem access here.
pub fn query_domains(source: &str) -> Vec<Domain> {
    if source.trim_start().starts_with("/inspect") {
        return Domain::ALL.to_vec();
    }
    let words = source
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .collect::<std::collections::BTreeSet<_>>();
    Domain::ALL
        .into_iter()
        .filter(|d| {
            words.contains(d.key())
                || (*d == Domain::Entities && words.contains("entity"))
                || (*d == Domain::World && words.contains("block_at"))
                || (*d == Domain::Entity && words.contains("entity_inspection"))
                || (*d == Domain::Chunk && words.contains("chunk_inspection"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_registration_identity_and_demand_are_generic() {
        let mut s = ControlState::default();
        s.diagnostics
            .registry
            .register(DebugView {
                id: "other_game:debug/pulse".into(),
                name: "pulse".into(),
                title: "Pulse".into(),
                description: "Independent game world query".into(),
                owner: "other_game:game".into(),
                kind: ViewKind::Page,
                cost: Cost::Medium,
                shortcut: None,
                requirements: vec![Domain::World],
            })
            .unwrap();
        s.view_action(&Action::DebugPage("other_game:debug/pulse".into()))
            .unwrap();
        assert_eq!(s.diagnostic_demand(), [Domain::World]);
        assert!(
            s.diagnostics
                .registry
                .register(
                    s.diagnostics
                        .registry
                        .get("pulse", ViewKind::Page)
                        .unwrap()
                        .clone()
                )
                .is_err()
        );
    }
    #[test]
    fn inactive_collection_and_shared_cadence_are_bounded() {
        let mut s = ControlState::default();
        assert!(s.diagnostic_demand().is_empty());
        let now = Instant::now();
        assert!(s.diagnostics.due(Domain::World, now));
        s.diagnostics.collected(Domain::World, now);
        for _ in 0..100 {
            assert!(!s.diagnostics.due(Domain::World, Instant::now()));
        }
        assert_eq!(s.diagnostics.samples[&Domain::World].collections, 1);
        s.diagnostics.invalidate(Domain::World);
        assert!(s.diagnostics.due(Domain::World, Instant::now()));
    }
    #[test]
    fn repeated_selection_toggles_use_bounded_state() {
        let mut s = ControlState::default();
        let count = s.diagnostics.registry.len();
        for _ in 0..100 {
            for i in [
                DebugInput::Open,
                DebugInput::Next,
                DebugInput::Activate,
                DebugInput::Tab,
                DebugInput::Activate,
                DebugInput::Activate,
                DebugInput::Close,
            ] {
                s.selector_input(i).unwrap();
            }
            for name in [
                "streaming",
                "entities",
                "collision",
                "target",
                "chunks",
                "selected_entity",
            ] {
                for enabled in [true, false, true, false] {
                    s.view_action(&Action::Overlay {
                        name: name.into(),
                        enabled,
                    })
                    .unwrap();
                }
            }
        }
        assert_eq!(s.diagnostics.registry.len(), count);
        assert!(s.overlays.is_empty());
        assert!(!s.selector.open);
        assert!(s.diagnostics.samples.len() <= Domain::ALL.len());
    }
    #[test]
    fn stable_selection_does_not_fall_back_to_other_entity() {
        let mut s = ControlState::default();
        s.view_action(&Action::InspectEntity(
            "00000000000000000000000000000001".into(),
        ))
        .unwrap();
        s.domains.entities = json!([{"id":"00000000000000000000000000000002"}]);
        assert_eq!(
            s.selected_entity.as_deref(),
            Some("00000000000000000000000000000001")
        );
        assert_eq!(s.diagnostic_demand(), [Domain::Entity]);
    }
    #[test]
    fn s1_persistence_page_prioritizes_pressure_without_extra_collection() {
        let mut s = ControlState {
            page: "persistence".into(),
            ..Default::default()
        };
        s.domains.persistence = json!({"dirty":4,"oldest_dirty_ms":100,"queued":2,"inflight":1,
            "column_io":{"columns":7,"raw_bytes":1000,"application_write_bytes":300},
            "dirty_subset":[{"chunk":[-1,2],"generation":7,"saving":true}]});
        let text = s.diagnostic_text();
        assert!(text.contains("queued 2 inflight 1"));
        assert!(text.contains("application writes 300 bytes"));
        assert!(text.contains("not device writes or isolated fsync"));
        assert!(s.diagnostics.samples.is_empty());
    }
}
