//! Bounded local launch grants, independent of game mode and process responsibility.
//! This is not remote authentication (A1/M5).
use rustcraft_control::{Context, Source};

pub(super) struct LocalSession {
    pub role: &'static str,
    pub context: Context,
}
impl LocalSession {
    pub fn principal(&self) -> &str {
        self.context.provenance.principal.as_str()
    }
    pub fn normal() -> Self {
        let mut context = Context::read_only(Source::DeveloperConsole)
            .with_local_principal(rustcraft_control::PrincipalId::parse("local:player").unwrap());
        context.capabilities.insert("debug.configure".into());
        Self {
            role: "local-diagnostics",
            context,
        }
    }
    pub fn from_args(args: &[String]) -> Self {
        let trusted = args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--devtools"
                    | "--scenario"
                    | "--dx-overhead"
                    | "--dux-acceptance"
                    | "--c1-acceptance"
                    | "--ux1-acceptance"
                    | "--rsm1-acceptance"
                    | "--p1-acceptance"
                    | "--f1-acceptance"
            )
        });
        if trusted {
            Self {
                role: "trusted-developer",
                context: Context::developer(Source::DeveloperConsole).with_local_principal(
                    rustcraft_control::PrincipalId::parse("local:player").unwrap(),
                ),
            }
        } else if args.iter().any(|a| a == "--player") {
            Self {
                role: "player",
                context: Context::read_only(Source::DeveloperConsole).with_local_principal(
                    rustcraft_control::PrincipalId::parse("local:player").unwrap(),
                ),
            }
        } else {
            Self::normal()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn game_mode_and_local_grants_are_independent() {
        for flags in [vec![], vec!["--survival"], vec!["--world", "test"]] {
            let session =
                LocalSession::from_args(&flags.into_iter().map(str::to_owned).collect::<Vec<_>>());
            assert!(session.context.require("debug.configure").is_ok());
            assert!(session.context.require("script.load").is_err());
            assert!(session.context.require("world.write").is_err());
        }
        let player = LocalSession::from_args(&["--player".into()]);
        assert!(player.context.require("debug.configure").is_err());
        let developer = LocalSession::from_args(&["--survival".into(), "--devtools".into()]);
        assert!(developer.context.require("script.load").is_ok());
        assert_eq!(player.principal(), developer.principal());
    }
}
