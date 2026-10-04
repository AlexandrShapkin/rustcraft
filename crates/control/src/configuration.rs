//! Shared command and bounded native Settings presentation adapter.
use crate::*;
use config::{Policy, Value as SettingValue};
use diagnostics::DebugInput;
impl ControlState {
    pub fn sync_config(&mut self) {
        if !self.config.is_empty() {
            self.script_poll_ms = self
                .config
                .effective(config::settings::SCRIPT_POLL_MS)
                .integer() as u64;
            self.diagnostics.cadence = Duration::from_millis(
                self.config
                    .effective(config::settings::DIAGNOSTIC_MS)
                    .integer() as u64,
            );
        }
    }
    pub fn configuration_action(&mut self, a: &Action) -> ControlResult<Value> {
        let result: ControlResult<Value> = (|| {
            match a {
                Action::ConfigSelect(k) => {
                    self.config.spec(k)?;
                    self.selected_setting = k.clone();
                    self.page = "settings".into();
                }
                Action::ConfigSet(changes) => {
                    let parsed = changes
                        .iter()
                        .map(|(k, v)| {
                            let spec = self.config.spec(k)?;
                            if spec.policy == Policy::ImmutableAfterOpen {
                                return Err(format!("{k} immutable after open: {}", spec.reason));
                            }
                            Ok((
                                k.clone(),
                                Some(
                                    spec.kind
                                        .parse(v)
                                        .map_err(|e| format!("{k} runtime: {e}"))?,
                                ),
                            ))
                        })
                        .collect::<ControlResult<Vec<_>>>()?;
                    let p = self.config.request(&parsed)?;
                    if p == Policy::Immediate {
                        self.config.apply(p, 0, |_| Ok(()))?;
                    }
                }
                Action::ConfigResetBatch(keys) => {
                    let p = self
                        .config
                        .request(&keys.iter().map(|k| (k.clone(), None)).collect::<Vec<_>>())?;
                    if p == Policy::Immediate {
                        self.config.apply(p, 0, |_| Ok(()))?;
                    }
                }
                Action::ConfigReset(k) => {
                    let p = self.config.request(&[(k.clone(), None)])?;
                    if p == Policy::Immediate {
                        self.config.apply(p, 0, |_| Ok(()))?;
                    }
                }
                Action::ConfigPersist { keys, remove } => {
                    self.config.persist_many(keys, *remove)?
                }
                _ => return Err("not a configuration action".into()),
            }
            self.sync_config();
            Ok(self.config.snapshot())
        })();
        self.config_status = match &result {
            Ok(_) if matches!(a, Action::ConfigPersist { .. }) => {
                "user file edited for next launch; startup source layers remain unchanged".into()
            }
            Ok(_) => "accepted; effective changes only at the declared boundary".into(),
            Err(e) => e.chars().take(256).collect(),
        };
        result
    }
    pub fn setting_input(&mut self, input: DebugInput) -> ControlResult<()> {
        self.page = "settings".into();
        let keys = self.config.keys().map(str::to_owned).collect::<Vec<_>>();
        if keys.is_empty() {
            return Err("configuration unavailable in this host".into());
        }
        let index = keys
            .iter()
            .position(|k| k == &self.selected_setting)
            .unwrap_or(0);
        if matches!(input, DebugInput::SettingNext | DebugInput::SettingPrevious) {
            let n = if input == DebugInput::SettingNext {
                (index + 1) % keys.len()
            } else {
                (index + keys.len() - 1) % keys.len()
            };
            self.selected_setting = keys[n].clone();
            return Ok(());
        }
        let key = keys[index].clone();
        self.selected_setting = key.clone();
        let a = if input == DebugInput::SettingReset {
            Action::ConfigReset(key)
        } else {
            let delta = if input == DebugInput::SettingIncrease {
                1
            } else {
                -1
            };
            let e = self.config.effective(&key);
            let next = match e {
                SettingValue::Bool(v) => (!v).to_string(),
                SettingValue::Integer(v) => (v + delta).to_string(),
                SettingValue::DurationMs(v) => (*v as i64 + delta * 50).to_string(),
                SettingValue::Bytes(v) => (*v as i64 + delta * 262144).to_string(),
                SettingValue::Float(v) => (v + delta as f64 * 0.25).to_string(),
                SettingValue::Text(_) => {
                    return Err("use /config set KEY VALUE for text/enum".into());
                }
            };
            Action::ConfigSet(vec![(key, next)])
        };
        self.configuration_action(&a).map(|_| ())
    }
    pub fn settings_text(&self, header: &str) -> String {
        let snapshot = self.config.snapshot();
        let keys = self.config.keys().collect::<Vec<_>>();
        let index = keys
            .iter()
            .position(|k| *k == self.selected_setting)
            .unwrap_or(0);
        let mut text = header.lines().take(3).collect::<Vec<_>>().join("\n");
        text.push_str(
            "\nSETTINGS Left/Right choose | +/- change | R reset | ` console exact /config\n",
        );
        for key in keys.iter().skip(index.saturating_sub(2)).take(6) {
            let e = &snapshot["settings"][*key];
            text.push_str(&format!(
                "{} {} effective={} requested={} [{}]\n",
                if Some(key) == keys.get(index) {
                    ">"
                } else {
                    " "
                },
                key,
                e["effective"],
                e["requested"],
                e["policy"]
            ));
        }
        if let Some(k) = keys.get(index) {
            let e = &snapshot["settings"][*k];
            for field in [
                "type",
                "source",
                "requested_source",
                "policy",
                "owner",
                "description",
                "reason",
                "availability",
                "persist",
                "pending",
            ] {
                text.push_str(&format!("{field}: {}\n", e[field]));
            }
        }
        text.push_str(&format!(
            "status: {}\nregistered {} (bounded scrolling selection)\n",
            self.config_status,
            keys.len()
        ));
        text
    }
}
pub fn command(args: &[String], s: &Snapshot) -> ControlResult<CommandResult> {
    match args {
        [verb] if verb=="list"=>Ok(CommandResult::output(s.config["settings"].to_string(),s.config.clone())),
        [verb,key] if verb=="get"||verb=="describe"=>{let e=&s.config["settings"][key];if e.is_null(){return Err(format!("unknown setting {key}"));}Ok(CommandResult::output(e.to_string(),e.clone()))},
        [verb,key,value] if verb=="set"=>Ok(CommandResult::action(Action::ConfigSet(vec![(key.clone(),value.clone())]))),
        [verb,key] if verb=="select"=>Ok(CommandResult::action(Action::ConfigSelect(key.clone()))),
        [verb,keys @ ..] if verb=="reset"=>Ok(CommandResult::action(Action::ConfigResetBatch(keys.to_vec()))),
        [verb,keys @ ..] if verb=="persist"||verb=="unpersist"=>Ok(CommandResult::action(Action::ConfigPersist{keys:keys.to_vec(),remove:verb=="unpersist"})),
        [verb,pairs @ ..] if verb=="batch"=>{let changes=pairs.iter().map(|p|p.split_once('=').map(|(k,v)|(k.into(),v.into())).ok_or("batch expects KEY=VALUE".into())).collect::<ControlResult<Vec<_>>>()?;Ok(CommandResult::action(Action::ConfigSet(changes)))},
        _=>Err("usage: /config list|get KEY|describe KEY|set KEY VALUE|batch KEY=VALUE ...|reset KEY|persist KEY|unpersist KEY".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_capabilities_completion_and_ui_share_registry() {
        let mut state = ControlState {
            config: config::settings::engine(true),
            ..Default::default()
        };
        state.config.open();
        state.sync_config();
        let k = config::settings::DIAGNOSTIC_MS;
        state
            .configuration_action(&Action::ConfigSelect(k.into()))
            .unwrap();
        state.setting_input(DebugInput::SettingIncrease).unwrap();
        assert_eq!(state.config.effective(k).integer(), 300);
        assert!(
            state
                .settings_text("Developer tools")
                .contains("effective=300")
        );
        let snapshot = Snapshot {
            config: state.config.snapshot(),
            ..Default::default()
        };
        let registry = engine_registry();
        let readonly = Context::read_only(Source::ServerAdmin);
        let read = registry
            .dispatch(&readonly, &format!("/config get {k}"), &snapshot)
            .unwrap();
        assert_eq!(read.data["effective"], 300);
        for action in [
            Action::ConfigReset(k.into()),
            Action::ConfigSet(vec![(k.into(), "50".into())]),
            Action::ConfigPersist {
                keys: vec![k.into()],
                remove: false,
            },
            Action::DebugUi(DebugInput::SettingIncrease),
        ] {
            assert!(readonly.require(action.capability()).is_err());
            assert!(
                Context::developer(Source::FutureChat)
                    .require(action.capability())
                    .is_err()
            );
        }
        assert!(
            registry
                .complete("/config get rustcraft:diagnostics/")
                .iter()
                .any(|s| s.ends_with(k))
        );
        state
            .configuration_action(&Action::ConfigReset(k.into()))
            .unwrap();
        assert_eq!(state.config.effective(k).integer(), 250);
    }
}
