//! `hotkeys.json`: the user's own hotkey choices over the source's, and whether and where hotkeys fire.

use std::collections::{BTreeMap, HashMap};

use model::{Cheat, Chord, Ref};
use serde::{Deserialize, Serialize};

use crate::storage::{Paths, Stored};

/// Slot: VKs, or `null` for unbound; a missing slot is the source's.
type Slots = BTreeMap<usize, Option<Vec<u16>>>;

#[derive(Clone, Copy)]
pub enum Choice {
    Bound(Chord),
    Unbound,
    /// Back to the source's binding.
    Default,
}

impl Choice {
    /// The chord `slot` of `cheat` ends up with.
    pub fn chord(self, cheat: &Cheat, slot: usize) -> Option<Chord> {
        match self {
            Choice::Bound(chord) => Some(chord),
            Choice::Unbound => None,
            Choice::Default => cheat.hotkeys.get(slot).copied().flatten(),
        }
    }
}

/// Why a chord is not simply bound.
#[derive(Clone, Debug, PartialEq)]
pub enum Clash {
    /// Someone else's hotkey: nothing frees it.
    Reserved,
    /// The slots holding it as (cheat index, slot). A duplicate is allowed (`keep`) unless one is the bound cheat's own.
    Slots { held: Vec<(usize, usize)>, keep: bool },
}

/// How a held chord is bound anyway.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resolution {
    /// Both fire: Wand fires every cheat bound to a chord.
    Keep,
    /// Every holder is left unbound.
    Take,
}

/// The slots of `slots`, a sheet's hotkeys as they fire, other than `at` (cheat, slot) that hold `chord`'s keys.
fn holders(slots: &[Vec<Option<Chord>>], at: (usize, usize), chord: Chord) -> impl Iterator<Item = (usize, usize)> {
    let bound = slots.iter().enumerate().flat_map(|(cheat, row)| row.iter().enumerate().map(move |(slot, held)| (cheat, slot, held)));
    bound
        .filter(move |&(cheat, slot, held)| (cheat, slot) != at && held.is_some_and(|held| held.same_keys(chord)))
        .map(|(cheat, slot, _)| (cheat, slot))
}

/// The slots whose chord another slot of the sheet holds too, by cheat then slot: Wand fires them all.
pub fn shared(slots: &[Vec<Option<Chord>>]) -> Vec<Vec<bool>> {
    let shares = |at, chord| holders(slots, at, chord).next().is_some();
    let rows = slots.iter().enumerate();
    rows.map(|(cheat, row)| row.iter().enumerate().map(|(slot, held)| held.is_some_and(|held| shares((cheat, slot), held))).collect())
        .collect()
}

/// Why `chord` cannot go to the slot `at`: the overlay's key first, then a slot of the cheat itself.
fn clash(slots: &[Vec<Option<Chord>>], at: (usize, usize), chord: Chord, reserved: impl Fn(Chord) -> bool) -> Option<Clash> {
    if reserved(chord) {
        return Some(Clash::Reserved);
    }
    let held: Vec<_> = holders(slots, at, chord).collect();
    (!held.is_empty()).then(|| Clash::Slots { keep: held.iter().all(|&(cheat, _)| cheat != at.0), held })
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Saved {
    /// Hotkeys fire during a run.
    enabled: bool,
    /// Hotkeys fire only while the game owns the foreground window.
    game_only: bool,
    /// By trainer, then cheat id.
    #[serde(with = "crate::storage::by_ref")]
    trainers: HashMap<Ref, HashMap<String, Slots>>,
}

impl Default for Saved {
    fn default() -> Self {
        Self { enabled: true, game_only: false, trainers: HashMap::new() }
    }
}

pub struct Store {
    saved: Stored<Saved>,
}

impl Store {
    pub fn load(paths: &Paths) -> Self {
        Self { saved: Stored::load(paths.hotkeys()) }
    }

    pub fn enabled(&self) -> bool {
        self.saved.enabled
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.saved.edit(|saved| saved.enabled = on);
    }

    pub fn game_only(&self) -> bool {
        self.saved.game_only
    }

    pub fn set_game_only(&mut self, on: bool) {
        self.saved.edit(|saved| saved.game_only = on);
    }

    /// The cheat's slots: the source's bindings with the user's choices laid over them. A default
    /// holding the keys of a choice is dropped: a newer cheat list may bring it after the choice.
    pub fn slots(&self, trainer: &Ref, cheat: &Cheat) -> Vec<Option<Chord>> {
        let chosen = self.saved.trainers.get(trainer).and_then(|cheats| cheats.get(&cheat.id));
        let choice = |slot: usize| chosen.and_then(|slots| slots.get(&slot)).map(|vks| vks.as_deref().and_then(Chord::from_vks));
        let choices: Vec<_> = (0..cheat.kind.slots()).map(choice).collect();
        let chosen_keys = |chord: Chord| choices.iter().flatten().flatten().any(|other| other.same_keys(chord));
        choices
            .iter()
            .enumerate()
            .map(|(slot, choice)| match choice {
                Some(chord) => *chord,
                None => cheat.hotkeys.get(slot).copied().flatten().filter(|chord| !chosen_keys(*chord)),
            })
            .collect()
    }

    pub fn choose(&mut self, trainer: &Ref, cheat: &str, slot: usize, choice: Choice) {
        self.saved.edit(|saved| {
            let cheats = saved.trainers.entry(trainer.clone()).or_default();
            let slots = cheats.entry(cheat.to_owned()).or_default();
            match choice {
                Choice::Bound(chord) => slots.insert(slot, Some(chord.vks().map(u16::from).collect())),
                Choice::Unbound => slots.insert(slot, None),
                Choice::Default => slots.remove(&slot),
            };
            if slots.is_empty() {
                cheats.remove(cheat);
            }
            if cheats.is_empty() {
                saved.trainers.remove(trainer);
            }
        });
    }

    /// Binds `choice` to the slot `at` (cheat, slot) of `cheats`, or says why not. A held chord is bound only by the
    /// resolution that fits it, checked against the bindings as they are now: `Keep` where a duplicate is allowed, `Take`.
    pub fn rebind(
        &mut self,
        trainer: &Ref,
        cheats: &[Cheat],
        at: (usize, usize),
        choice: Choice,
        resolution: Option<Resolution>,
        reserved: impl Fn(Chord) -> bool,
    ) -> Result<(), Clash> {
        let (index, slot) = at;
        let wanted = choice.chord(&cheats[index], slot);
        let clash_now = |store: &Self| {
            let slots: Vec<_> = cheats.iter().map(|cheat| store.slots(trainer, cheat)).collect();
            wanted.and_then(|chord| clash(&slots, at, chord, &reserved))
        };
        match clash_now(self) {
            None => {}
            Some(Clash::Slots { keep: true, .. }) if resolution == Some(Resolution::Keep) => {}
            Some(Clash::Slots { .. }) if resolution == Some(Resolution::Take) => {
                // Unbinding a holder may show a default its choice hid: look again until none holds the chord.
                while let Some(Clash::Slots { held, .. }) = clash_now(self) {
                    for (cheat, slot) in held {
                        self.choose(trainer, &cheats[cheat].id, slot, Choice::Unbound);
                    }
                }
            }
            Some(clash) => return Err(clash),
        }
        self.choose(trainer, &cheats[index].id, slot, choice);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use model::{Args, Kind};

    use super::*;

    fn cheat() -> Cheat {
        Cheat {
            id: "a".into(),
            key: None,
            name: String::new(),
            category: String::new(),
            category_name: None,
            kind: Kind::Number,
            args: Args::default(),
            hotkeys: vec![Chord::from_vks(&[112])],
            instructions: None,
            description: None,
        }
    }

    #[test]
    fn choices_lay_over_the_source_and_survive_a_reload() {
        let dir = std::env::temp_dir().join(format!("impulse-hotkeys-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = Paths::new(dir.clone());
        let mut store = Store::load(&paths);
        let g = Ref::parse("s:g").unwrap();
        assert!(store.enabled());
        assert_eq!(store.slots(&g, &cheat()), [Chord::from_vks(&[112]), None]);
        store.choose(&g, "a", 0, Choice::Unbound);
        store.choose(&g, "a", 1, Choice::Bound(Chord::from_vks(&[17, 113]).unwrap()));
        store.set_game_only(true);
        store.set_enabled(false);
        let reloaded = Store::load(&paths);
        assert_eq!(reloaded.slots(&g, &cheat()), [None, Chord::from_vks(&[17, 113])]);
        assert!(reloaded.game_only() && !reloaded.enabled());
        store.choose(&g, "a", 0, Choice::Default);
        store.choose(&g, "a", 1, Choice::Default);
        assert!(Store::load(&paths).saved.trainers.is_empty());
        store.choose(&g, "a", 1, Choice::Bound(Chord::from_vks(&[112]).unwrap()));
        assert_eq!(store.slots(&g, &cheat()), [None, Chord::from_vks(&[112])]);
        fs::remove_dir_all(&dir).unwrap();
    }

    fn chord(vks: &[u16]) -> Chord {
        Chord::from_vks(vks).unwrap()
    }

    #[test]
    fn a_chord_clashes_with_other_slots_or_the_reserved() {
        let (f1, f2, f3) = (chord(&[112]), chord(&[113]), chord(&[114]));
        let slots = vec![vec![Some(f1), None], vec![Some(f2)]];
        assert_eq!(clash(&slots, (0, 1), f1, |_| false), Some(Clash::Slots { held: vec![(0, 0)], keep: false }));
        assert_eq!(clash(&slots, (0, 1), f2, |_| false), Some(Clash::Slots { held: vec![(1, 0)], keep: true }));
        assert_eq!(clash(&slots, (0, 0), f1, |_| false), None, "a slot does not hold against itself");
        assert_eq!(clash(&slots, (0, 1), f3, |_| false), None);
        assert_eq!(clash(&slots, (0, 1), f1, |_| true), Some(Clash::Reserved), "the overlay's key comes first");
        let pair = vec![vec![Some(chord(&[17, 112]))]];
        assert_eq!(clash(&pair, (1, 0), chord(&[112, 17]), |_| false), Some(Clash::Slots { held: vec![(0, 0)], keep: true }));
    }

    #[test]
    fn a_slot_is_shared_when_another_holds_its_keys() {
        let (f1, f2) = (chord(&[112]), chord(&[113]));
        let slots = vec![vec![Some(f1), None], vec![Some(f1), Some(f2)], vec![None]];
        assert_eq!(shared(&slots), vec![vec![true, false], vec![true, false], vec![false]]);
    }

    #[test]
    fn taking_a_chord_also_frees_a_default_its_holder_hid() {
        let dir = std::env::temp_dir().join(format!("impulse-take-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut store = Store::load(&Paths::new(dir.clone()));
        let g = Ref::parse("s:g").unwrap();
        let f1 = chord(&[112]);
        let cheats =
            [Cheat { id: "d".into(), hotkeys: vec![Some(f1), None], ..cheat() }, Cheat { id: "e".into(), hotkeys: vec![], ..cheat() }];
        let slots = |store: &Store| cheats.iter().map(|cheat| store.slots(&g, cheat)).collect::<Vec<_>>();
        // The user's own F1 on slot 1 hides the source's F1 on slot 0.
        store.choose(&g, "d", 1, Choice::Bound(f1));
        assert_eq!(slots(&store), [[None, Some(f1)], [None, None]]);
        assert_eq!(store.rebind(&g, &cheats, (1, 0), Choice::Bound(f1), Some(Resolution::Take), |_| false), Ok(()));
        assert_eq!(slots(&store), [[None, None], [Some(f1), None]]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_held_chord_is_bound_only_by_the_resolution_that_fits() {
        let dir = std::env::temp_dir().join(format!("impulse-rebind-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut store = Store::load(&Paths::new(dir.clone()));
        let g = Ref::parse("s:g").unwrap();
        let (f1, f2) = (chord(&[112]), chord(&[113]));
        let cheats = [cheat(), Cheat { id: "b".into(), hotkeys: vec![Some(f2)], ..cheat() }];
        let slots = |store: &Store| cheats.iter().map(|cheat| store.slots(&g, cheat)).collect::<Vec<_>>();
        let free = |_: Chord| false;

        let other = Clash::Slots { held: vec![(0, 0)], keep: true };
        assert_eq!(store.rebind(&g, &cheats, (1, 0), Choice::Bound(f1), None, free), Err(other));
        assert_eq!(store.rebind(&g, &cheats, (1, 0), Choice::Bound(f1), Some(Resolution::Keep), free), Ok(()));
        assert_eq!(slots(&store), [[Some(f1), None], [Some(f1), None]]);

        let (own, both) = (Clash::Slots { held: vec![(0, 0), (1, 0)], keep: false }, Resolution::Keep);
        assert_eq!(store.rebind(&g, &cheats, (0, 1), Choice::Bound(f1), Some(both), free), Err(own));
        assert_eq!(store.rebind(&g, &cheats, (0, 1), Choice::Bound(f1), Some(Resolution::Take), free), Ok(()));
        assert_eq!(slots(&store), [[None, Some(f1)], [None, None]]);

        let reserved = |held: Chord| held == f2;
        assert_eq!(store.rebind(&g, &cheats, (0, 0), Choice::Bound(f2), Some(Resolution::Take), reserved), Err(Clash::Reserved));
        assert_eq!(store.rebind(&g, &cheats, (0, 1), Choice::Unbound, None, |_| true), Ok(()));
        assert_eq!(store.rebind(&g, &cheats, (1, 0), Choice::Default, None, free), Ok(()));
        assert_eq!(slots(&store), [[None, None], [Some(f2), None]]);
        fs::remove_dir_all(&dir).unwrap();
    }
}
