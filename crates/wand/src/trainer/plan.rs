//! What running a trainer takes besides its cheat list, read from the same response.

use std::{collections::HashSet, sync::Arc, time::Duration};

use model::{Launch, Pick, Plan, Runner, Target};
use serde_json::Value;

use super::Trainer;
use crate::proxy::Proxy;

// A process younger than this is still starting up.
const MINIMUM_AGE: Duration = Duration::from_secs(8);
const EVENT_KEY_CHARS: usize = 22;

/// A trainer DLL on the public CDN, exactly as the response names it: both fields are
/// untrusted until `binary` has checked them.
#[derive(Clone, Debug, PartialEq)]
pub struct Binary {
    pub url: String,
    /// SHA-256 of the file, hex.
    pub hash: String,
}

/// What the runner puts into the game.
#[derive(Debug, PartialEq)]
pub struct Load {
    /// The names the trainer starts with: one per cheat target and per game event.
    pub variables: Vec<String>,
    /// `Err` says why this trainer cannot run here.
    pub binary: Result<Binary, String>,
    /// The stand-in for the game's launcher at a start; `Err` says why the trainer cannot run.
    pub proxy: Result<Option<Proxy>, String>,
    pub activate: Activate,
}

/// `activate.flags`: how the trainer is loaded.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Activate(pub u64);

impl Activate {
    /// The game is held still while the trainer sets up.
    pub fn suspend(self) -> bool {
        self.0 & 1 != 0
    }

    /// A held game is let go once the trainer is ready; without it, it stays held.
    pub fn resume(self) -> bool {
        self.0 & 2 != 0
    }

    /// The external host takes the service in, not the game.
    pub fn external_host(self) -> bool {
        self.0 & 16 != 0
    }

    /// Hidden threads, masked modules: the classic loader only, which is not ours.
    pub fn classic(self) -> bool {
        self.0 & (4 | 8) != 0
    }
}

impl Trainer {
    pub fn load(&self) -> Load {
        let config = &self.blueprint.config;
        Load {
            variables: self.variables(),
            binary: self.binary(),
            proxy: Proxy::parse(&config["launch"]),
            activate: Activate(config["activate"]["flags"].as_u64().unwrap_or(0)),
        }
    }

    /// How a run finds or starts the game; `runner` is the one made from `load`.
    pub fn plan(&self, runner: Arc<dyn Runner>) -> Plan {
        let config = &self.blueprint.config;
        let (launch, target) = (&config["launch"], &config["target"]);
        if !launch["bypass"].is_null() {
            log::debug!("launch.bypass is not supported");
        }
        // A proxy's game is the process to look for, never its launcher.
        let image = target["image"].as_str().map(str::to_owned);
        let image = image.or_else(|| Proxy::parse(launch).ok().flatten().map(|proxy| proxy.game()));
        Plan {
            target: Target {
                image,
                minimum_age: target["minimumAge"].as_u64().map_or(MINIMUM_AGE, Duration::from_millis),
                // 1 oldest, 2 newest, anything else the most memory; JS compares the number, `1.0` too.
                pick: match target["reducer"].as_f64() {
                    Some(1.0) => Pick::Oldest,
                    Some(2.0) => Pick::Newest,
                    _ => Pick::Biggest,
                },
                arguments: target["cliArgs"].as_str().map(str::to_owned),
            },
            launch: match launch {
                Value::Bool(false) => Launch::User,
                _ if launch["required"] == Value::Bool(true) => Launch::Required,
                _ => Launch::Start,
            },
            launch_arguments: launch["cliArgs"].as_str().filter(|args| !args.is_empty()).map(str::to_owned),
            launch_within: target["timeout"].as_u64().map(Duration::from_millis),
            runner,
        }
    }

    fn variables(&self) -> Vec<String> {
        let events = self.game_events.iter().filter_map(|event| {
            let key: String = event["key"].as_str()?.chars().take(EVENT_KEY_CHARS).collect();
            Some(format!("wm_event_{key}"))
        });
        let targets = self.blueprint.cheats.iter().map(|cheat| cheat.target.clone()).filter(|target| !target.is_empty());
        let mut seen = HashSet::new();
        targets.chain(events).filter(|name| seen.insert(name.clone())).collect()
    }

    fn binary(&self) -> Result<Binary, String> {
        match self.loader.as_deref() {
            Some("trainerlib") => {}
            other => return Err(format!("trainer type '{}' is not supported yet", other.unwrap_or("unknown"))),
        }
        let (Some(url), Some(hash)) = (&self.loader_args.binary_url, &self.loader_args.binary_hash) else {
            return Err("the trainer has no binary".into());
        };
        Ok(Binary { url: url.clone(), hash: hash.clone() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "15b7ad4edbd55daada6d4c8669a57a455820b9579165efcd6934202165f82683";

    fn trainer(extra: &str) -> Trainer {
        let json = format!(
            r#"{{
                "blueprint": {{"cheats": [
                    {{"uuid": "a", "name": "Health", "category": "p", "type": "toggle", "target": "infinite_health"}},
                    {{"uuid": "b", "name": "Hurt", "category": "p", "type": "toggle", "target": "infinite_health"}},
                    {{"uuid": "c", "name": "Speed", "category": "p", "type": "slider", "target": "set_game_speed"}},
                    {{"uuid": "d", "name": "Odd", "category": "p", "type": "toggle"}}
                ]{extra}}},
                "loader": "trainerlib",
                "loaderArgs": {{"trainerId": "50920", "binaryUrl": "https://cdn.example/bin/1", "binaryHash": "{HASH}"}},
                "gameEvents": [{{"key": "a_rather_long_event_key_beyond_the_limit"}}, {{"key": "a_rather_long_event_key_beyond_the_limit"}}, {{"no": "key"}}]
            }}"#
        );
        serde_json::from_str(&json).unwrap()
    }

    struct Idle;

    impl Runner for Idle {
        fn prepare(&self) -> futures::future::BoxFuture<'_, Result<Box<dyn model::Attach>, model::Failure>> {
            unreachable!("a plan's test never runs it")
        }
    }

    fn plan(extra: &str) -> Plan {
        trainer(extra).plan(Arc::new(Idle))
    }

    #[test]
    fn reads_what_wand_reads() {
        let config = r#", "config": {"target": {"image": "enshrouded.exe", "minimumAge": 3000, "reducer": 1, "cliArgs": "-server",
            "timeout": 90000}, "launch": {"cliArgs": "-dx11"}}"#;
        let load = trainer(config).load();
        assert_eq!(load.variables, ["infinite_health", "set_game_speed", "wm_event_a_rather_long_event_ke"]);
        assert_eq!(load.binary, Ok(Binary { url: "https://cdn.example/bin/1".into(), hash: HASH.into() }));
        assert_eq!(load.proxy, Ok(None));
        let game = plan(config);
        let target = Target {
            image: Some("enshrouded.exe".into()),
            minimum_age: Duration::from_secs(3),
            pick: Pick::Oldest,
            arguments: Some("-server".into()),
        };
        assert_eq!(game.target, target);
        assert_eq!((game.launch_arguments.as_deref(), game.launch_within), (Some("-dx11"), Some(Duration::from_secs(90))));
        assert_eq!(game.launch, Launch::Start);
        let pick = |reducer: &str| plan(&format!(r#", "config": {{"target": {{"reducer": {reducer}}}}}"#)).target.pick;
        let picks = [pick("2"), pick("1.0"), pick("2e0"), pick("0"), pick(r#""1""#), pick("7")];
        assert_eq!(picks, [Pick::Newest, Pick::Oldest, Pick::Newest, Pick::Biggest, Pick::Biggest, Pick::Biggest]);
    }

    #[test]
    fn defaults_when_the_config_is_empty_or_odd() {
        for extra in ["", r#", "config": {"launch": false, "target": {"image": ["x"]}}"#, r#", "config": 7"#] {
            let game = plan(extra);
            assert_eq!(game.target, Target { image: None, minimum_age: MINIMUM_AGE, pick: Pick::Biggest, arguments: None }, "{extra}");
            let launch = if extra.contains("false") { Launch::User } else { Launch::Start };
            assert_eq!(game.launch, launch, "{extra}");
            assert_eq!((game.launch_arguments, game.launch_within), (None, None), "{extra}");
            assert_eq!(trainer(extra).load().proxy, Ok(None), "{extra}");
        }
    }

    #[test]
    fn an_anti_cheat_proxy_names_the_game_to_look_for_and_who_starts_it() {
        let ace_combat = r#", "config": {"launch": {"proxy": {"gameExe": "Game\\Binaries\\Win64\\AceCombat8.exe",
            "launcher": "start_protected_game.exe"}, "required": true}}"#;
        let (game, load) = (plan(ace_combat), trainer(ace_combat).load());
        assert_eq!(
            (game.target.image.as_deref(), game.launch, load.activate),
            (Some(r"Game\Binaries\Win64\AceCombat8.exe"), Launch::Required, Activate(0))
        );
        assert!(matches!(load.proxy, Ok(Some(_))));

        let elden_ring = r#", "config": {"launch": {"proxy": {"gameExe": "eldenring.exe", "launcher": "Game\\start_protected_game.exe"}},
            "target": {"image": "Game\\eldenring.exe"}, "activate": {"flags": 16}}"#;
        let (game, load) = (plan(elden_ring), trainer(elden_ring).load());
        assert_eq!((game.target.image.as_deref(), game.launch, load.activate), (Some(r"Game\eldenring.exe"), Launch::Start, Activate(16)));

        let escaping = r#", "config": {"launch": {"proxy": {"gameExe": "..\\..\\x.exe", "launcher": "start_protected_game.exe"}}}"#;
        assert_eq!(plan(escaping).target.image, None);
        assert!(trainer(escaping).load().proxy.unwrap_err().contains("gameExe"));
    }

    #[test]
    fn refuses_a_trainer_it_cannot_run() {
        let broken = |loader: &str, args: &str| {
            let json = format!(r#"{{"blueprint": {{"cheats": []}}, "loader": {loader}, "loaderArgs": {args}}}"#);
            serde_json::from_str::<Trainer>(&json).unwrap().load().binary.unwrap_err()
        };
        assert!(broken(r#""artifacts""#, "{}").contains("'artifacts' is not supported"));
        assert!(broken("null", "{}").contains("'unknown'"));
        assert!(broken(r#""trainerlib""#, "{}").contains("no binary"));
        assert!(broken(r#""trainerlib""#, r#"{"binaryUrl": "https://x/y"}"#).contains("no binary"));
    }
}
