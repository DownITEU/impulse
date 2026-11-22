//! What running a trainer takes besides its cheat list, read from the same response.

use std::{collections::HashSet, time::Duration};

use model::Launch;
use serde_json::Value;

use super::Trainer;
use crate::proxy::Proxy;

// Wand's default: a process younger than this is still starting up.
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

#[derive(Debug, PartialEq)]
pub struct Plan {
    /// The names the trainer starts with: one per cheat target and per game event.
    pub variables: Vec<String>,
    /// `Err` says why this trainer cannot run here.
    pub binary: Result<Binary, String>,
    /// Path suffix of the game's executable, `*` allowed. Without it the install folder decides.
    pub image: Option<String>,
    pub minimum_age: Duration,
    pub launch: Launch,
    /// The stand-in for the game's launcher at a start; `Err` says why the trainer cannot run.
    pub proxy: Result<Option<Proxy>, String>,
    /// `activate.flags`: how Wand injects it (suspended, hidden threads, masked modules, an external host). Not
    /// done here; a failed attach names them.
    pub activate: u64,
}

impl Trainer {
    pub fn plan(&self) -> Plan {
        let config = &self.blueprint.config;
        let launch = &config["launch"];
        let proxy = Proxy::parse(launch);
        for unread in ["bypass", "cliArgs"].into_iter().filter(|field| !launch[field].is_null()) {
            log::debug!("launch.{unread} is not supported");
        }
        // A proxy's game is the process to look for, never its launcher.
        let image = config["target"]["image"].as_str().map(str::to_owned);
        let image = image.or_else(|| proxy.as_ref().ok().and_then(Option::as_ref).map(Proxy::game));
        Plan {
            variables: self.variables(),
            binary: self.binary(),
            image,
            minimum_age: config["target"]["minimumAge"].as_u64().map_or(MINIMUM_AGE, Duration::from_millis),
            launch: match launch {
                Value::Bool(false) => Launch::User,
                _ if launch["required"] == Value::Bool(true) => Launch::Required,
                _ => Launch::Start,
            },
            proxy,
            activate: config["activate"]["flags"].as_u64().unwrap_or(0),
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

    #[test]
    fn reads_what_wand_reads() {
        let plan = trainer(r#", "config": {"target": {"image": "enshrouded.exe", "minimumAge": 3000}}"#).plan();
        assert_eq!(plan.variables, ["infinite_health", "set_game_speed", "wm_event_a_rather_long_event_ke"]);
        assert_eq!(plan.binary, Ok(Binary { url: "https://cdn.example/bin/1".into(), hash: HASH.into() }));
        assert_eq!(plan.image.as_deref(), Some("enshrouded.exe"));
        assert_eq!(plan.minimum_age, Duration::from_secs(3));
        assert_eq!((plan.launch, plan.proxy), (Launch::Start, Ok(None)));
    }

    #[test]
    fn defaults_when_the_config_is_empty_or_odd() {
        for extra in ["", r#", "config": {"launch": false, "target": {"image": ["x"]}}"#, r#", "config": 7"#] {
            let plan = trainer(extra).plan();
            assert_eq!(plan.image, None, "{extra}");
            assert_eq!(plan.minimum_age, MINIMUM_AGE, "{extra}");
            let launch = if extra.contains("false") { Launch::User } else { Launch::Start };
            assert_eq!((plan.launch, plan.proxy), (launch, Ok(None)), "{extra}");
        }
    }

    #[test]
    fn an_anti_cheat_proxy_names_the_game_to_look_for_and_who_starts_it() {
        let ace_combat = r#", "config": {"launch": {"proxy": {"gameExe": "Game\\Binaries\\Win64\\AceCombat8.exe",
            "launcher": "start_protected_game.exe"}, "required": true}}"#;
        let plan = trainer(ace_combat).plan();
        assert_eq!((plan.image.as_deref(), plan.launch, plan.activate), (Some(r"Game\Binaries\Win64\AceCombat8.exe"), Launch::Required, 0));
        assert!(matches!(plan.proxy, Ok(Some(_))));

        let elden_ring = r#", "config": {"launch": {"proxy": {"gameExe": "eldenring.exe", "launcher": "Game\\start_protected_game.exe"}},
            "target": {"image": "Game\\eldenring.exe"}, "activate": {"flags": 16}}"#;
        let plan = trainer(elden_ring).plan();
        assert_eq!((plan.image.as_deref(), plan.launch, plan.activate), (Some(r"Game\eldenring.exe"), Launch::Start, 16));

        let escaping = r#", "config": {"launch": {"proxy": {"gameExe": "..\\..\\x.exe", "launcher": "start_protected_game.exe"}}}"#;
        let plan = trainer(escaping).plan();
        assert_eq!(plan.image, None);
        assert!(plan.proxy.unwrap_err().contains("gameExe"));
    }

    #[test]
    fn refuses_a_trainer_it_cannot_run() {
        let broken = |loader: &str, args: &str| {
            let json = format!(r#"{{"blueprint": {{"cheats": []}}, "loader": {loader}, "loaderArgs": {args}}}"#);
            serde_json::from_str::<Trainer>(&json).unwrap().plan().binary.unwrap_err()
        };
        assert!(broken(r#""artifacts""#, "{}").contains("'artifacts' is not supported"));
        assert!(broken("null", "{}").contains("'unknown'"));
        assert!(broken(r#""trainerlib""#, "{}").contains("no binary"));
        assert!(broken(r#""trainerlib""#, r#"{"binaryUrl": "https://x/y"}"#).contains("no binary"));
    }
}
