//! Placeholder data from the design mockup; real catalog and trainer data replace it.

#[derive(Clone, Copy, PartialEq)]
pub enum Trainer {
    Ready,
    UpdatePending,
    None,
}

pub struct Game {
    pub name: &'static str,
    pub steam_app: u32,
    pub trainer: Trainer,
    pub mods: u32,
    pub build: &'static str,
    pub played: &'static str,
    pub sessions: u32,
    pub map: Option<&'static str>,
    pub last_session: &'static str,
}

macro_rules! game {
    ($name:literal, $app:literal, $trainer:expr, $mods:literal, $build:literal, $played:literal, $sessions:literal, $map:expr, $last:literal) => {
        Game {
            name: $name,
            steam_app: $app,
            trainer: $trainer,
            mods: $mods,
            build: $build,
            played: $played,
            sessions: $sessions,
            map: $map,
            last_session: $last,
        }
    };
}

use Trainer::{None as NoTrainer, Ready, UpdatePending};

#[rustfmt::skip]
pub const GAMES: [Game; 12] = [
    game!("Enshrouded", 1203620, Ready, 28, "1793791052", "1.2 H", 11, Some("EMBERVALE"), "00:42:17"),
    game!("Baldur's Gate 3", 1086940, Ready, 41, "1790112840", "86 H", 54, None, "02:14:09"),
    game!("Elden Ring", 1245620, Ready, 35, "1785530211", "212 H", 140, Some("THE LANDS BETWEEN"), "01:37:52"),
    game!("Cyberpunk 2077", 1091500, Ready, 52, "1788904417", "64 H", 38, None, "00:58:31"),
    game!("Palworld", 1623730, UpdatePending, 33, "1794310022", "40 H", 25, Some("PALPAGOS ISLANDS"), "03:02:40"),
    game!("Hades II", 1145350, Ready, 19, "1786620935", "22 H", 30, None, "00:44:15"),
    game!("Stardew Valley", 413150, Ready, 24, "1781442710", "130 H", 90, None, "01:20:05"),
    game!("Hogwarts Legacy", 990080, Ready, 30, "1779928403", "31 H", 19, Some("HOGWARTS"), "01:05:44"),
    game!("Starfield", 1716740, Ready, 46, "1790845566", "12 H", 8, None, "00:31:20"),
    game!("Lies of P", 1627720, Ready, 22, "1784207719", "18 H", 15, None, "01:11:38"),
    game!("Monster Hunter Wilds", 2246340, UpdatePending, 38, "1794455108", "55 H", 33, None, "02:26:57"),
    game!("Balatro", 2379780, NoTrainer, 0, "1782210984", "9 H", 6, None, "00:52:10"),
];

#[derive(Clone, Copy)]
pub enum Control {
    Toggle,
    Incremental,
    /// Initial level, 0..=9.
    Slider(u8),
    Number,
    Button,
    /// Options, the first is the default.
    Selection(&'static [&'static str]),
}

pub struct Mod {
    pub id: &'static str,
    pub category: &'static str,
    pub name: &'static str,
    pub control: Control,
    pub keys: &'static [&'static str],
    pub instructions: Option<&'static str>,
}

const fn cheat(
    id: &'static str,
    category: &'static str,
    name: &'static str,
    control: Control,
    keys: &'static [&'static str],
    instructions: Option<&'static str>,
) -> Mod {
    Mod {
        id,
        category,
        name,
        control,
        keys,
        instructions,
    }
}

pub const CATEGORIES: [&str; 5] = ["PLAYER", "INVENTORY", "WORLD", "COMBAT", "TELEPORT"];

use Control::{Button, Incremental, Number, Selection, Slider, Toggle};

#[rustfmt::skip]
pub const MODS: [Mod; 20] = [
    cheat("hp", "PLAYER", "Unlimited Health", Toggle, &["SHIFT+F1"], None),
    cheat("st", "PLAYER", "Unlimited Stamina", Toggle, &["SHIFT+F2"], None),
    cheat("mp", "PLAYER", "Unlimited Mana", Toggle, &["SHIFT+F3"], None),
    cheat("fall", "PLAYER", "No Fall Damage", Toggle, &["SHIFT+F4"], None),
    cheat("crit", "PLAYER", "Edit Critical Chance", Incremental, &["↑ SHIFT+F6", "↓ ALT+F6"], Some("Each step changes the chance by 5 %.")),
    cheat("spd", "PLAYER", "Movement Speed", Slider(4), &["SHIFT+F7"], None),
    cheat("jump", "PLAYER", "Jump Height", Slider(0), &["SHIFT+F8"], None),
    cheat("dur", "INVENTORY", "Unlimited Durability", Toggle, &["SHIFT+F9"], None),
    cheat("craft", "INVENTORY", "Free Crafting", Toggle, &["SHIFT+F10"], Some("Open the crafting menu once after enabling.")),
    cheat("amt", "INVENTORY", "Set Selected Item Amount", Number, &["SHIFT+F11"], None),
    cheat("refill", "INVENTORY", "Refill Consumables", Button, &["CTRL+F1"], None),
    cheat("freeze", "WORLD", "Freeze Time of Day", Toggle, &["CTRL+F2"], None),
    cheat("tod", "WORLD", "Time of Day", Selection(&["NOON", "DUSK", "NIGHT", "DAWN"]), &["CTRL+F3"], None),
    cheat("gs", "WORLD", "Game Speed", Slider(0), &["CTRL+F4"], None),
    cheat("ohk", "COMBAT", "One-Hit Kills", Toggle, &["CTRL+F5"], None),
    cheat("dmg", "COMBAT", "Damage Multiplier", Slider(0), &["CTRL+F6"], None),
    cheat("ammo", "COMBAT", "Unlimited Ammo", Toggle, &["CTRL+F7"], None),
    cheat("tpw", "TELEPORT", "Teleport to Waypoint", Button, &["CTRL+F8"], None),
    cheat("tps", "TELEPORT", "Save Position", Button, &["CTRL+F9"], None),
    cheat("tpr", "TELEPORT", "Restore Position", Button, &["CTRL+F10"], None),
];
