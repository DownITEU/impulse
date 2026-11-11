//! The shown map and what the user did with it, shared by the stage, the sidebar and the card.
//! They observe it; it notifies on what a person does (clicks, typing, the player's position), never per frame.

use std::{path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use gpui_kit::{Context, EventEmitter, Task};
use model::{Map, Point, Session};

use super::{
    group,
    state::{self, Mark, PALETTE, Saved},
};
use crate::Load;

const EVERY: Duration = Duration::from_secs(1);
// Failed answers in a row before the map stops asking.
const GIVE_UP: usize = 3;

/// A live run of the shown game, as far as the map uses it.
pub struct Run {
    pub session: Arc<dyn Session>,
    pub location: bool,
    pub teleport: bool,
}

pub enum BoardEvent {
    /// A map went up; `first`: another map than the one shown, not a fresher copy of it; `retiled`: its tiles differ.
    Installed { first: bool, retiled: bool },
    /// The player moved; only the stage draws where. A notify tells whether there is a player at all.
    Moved,
    /// What shows changed: filters, layers, the search, found pins.
    Filtered,
    Restyled,
    /// A pin or a mark of the user's was opened or closed.
    Selected,
    /// A mark of the user's was added and opened.
    MarkAdded,
    /// The view should show `to`; `close`: near enough to tell pins apart.
    Bring { to: Point, close: bool },
}

impl EventEmitter<BoardEvent> for Board {}

pub struct Board {
    pub accent: u32,
    state_dir: PathBuf,
    // The shown map's id, which its saved state is kept under.
    id: String,
    pub map: Load<Rc<Map>>,
    // Shared so a frame can hold it while it paints.
    pub saved: Rc<Saved>,
    // Pins per category and group: all of them and the found ones.
    pub counts: Vec<Vec<usize>>,
    pub found_counts: Vec<Vec<usize>>,
    // Each pin's name, group, category and description, lowercased; made on the map's first search.
    haystacks: Vec<String>,
    query: String,
    pub selected: Option<usize>,
    // The user's own pin whose card is open.
    pub mark: Option<usize>,
    run: Option<Run>,
    pub player: Option<[f64; 3]>,
    // What went wrong last, shown on the map.
    pub note: Option<String>,
    // The player's position is asked for only while the map is on screen.
    open: bool,
    _poll: Task<()>,
}

impl Board {
    pub fn new(state_dir: PathBuf, accent: u32) -> Self {
        Self {
            accent,
            state_dir,
            id: String::new(),
            map: Load::Pending,
            saved: Rc::default(),
            counts: Vec::new(),
            found_counts: Vec::new(),
            haystacks: Vec::new(),
            query: String::new(),
            selected: None,
            mark: None,
            run: None,
            player: None,
            note: None,
            open: false,
            _poll: Task::ready(()),
        }
    }

    pub fn set_accent(&mut self, accent: u32, cx: &mut Context<Self>) {
        self.accent = accent;
        cx.emit(BoardEvent::Restyled);
        cx.notify();
    }

    /// Waits for the map `id`; the one shown keeps its state on disk first.
    pub fn pending(&mut self, id: String, cx: &mut Context<Self>) {
        self.save();
        (self.id, self.map, self.selected, self.mark) = (id, Load::Pending, None, None);
        self.watch(cx);
        cx.notify();
    }

    /// A fresh copy of the map on screen keeps the user's state, not what refers to its pins.
    pub fn install(&mut self, map: Map, cx: &mut Context<Self>) {
        let first = matches!(self.map, Load::Pending);
        let retiled = !matches!(&self.map, Load::Ready(shown) if shown.tiles == map.tiles);
        if first {
            self.saved = Rc::new(state::load(&self.state_dir, &self.id).unwrap_or_else(|| Saved::first(&map)));
        }
        self.counts = map.categories.iter().map(|category| vec![0; category.groups.len()]).collect();
        for pin in &map.pins {
            self.counts[pin.group.0][pin.group.1] += 1;
        }
        (self.map, self.selected, self.mark, self.haystacks) = (Load::Ready(Rc::new(map)), None, None, Vec::new());
        self.index();
        self.recount();
        self.watch(cx);
        cx.emit(BoardEvent::Installed { first, retiled });
        cx.notify();
    }

    // Most maps are never searched: the lowercased copy of every description waits for the first query.
    fn index(&mut self) {
        let Load::Ready(map) = &self.map else { return };
        if self.query.is_empty() || !self.haystacks.is_empty() {
            return;
        }
        self.haystacks = map
            .pins
            .iter()
            .map(|pin| format!("{}\n{}\n{}\n{}", pin.name, group(map, pin).name, map.categories[pin.group.0].name, pin.description).to_lowercase())
            .collect();
    }

    /// The map could not be loaded; a copy on screen stays.
    pub fn fail(&mut self, why: String, cx: &mut Context<Self>) {
        if matches!(self.map, Load::Pending) {
            self.map = Load::Failed(why);
            cx.notify();
        }
    }

    /// Keeps the user's state of the map on screen; blocking, so saves land in order (small, on a click).
    pub fn save(&self) {
        if let Load::Ready(_) = self.map {
            state::store(&self.state_dir, &self.id, &self.saved);
        }
    }

    pub fn saved_mut(&mut self) -> &mut Saved {
        Rc::make_mut(&mut self.saved)
    }

    /// Changes what the user set (filters, layers) and keeps it.
    pub fn filter(&mut self, change: impl FnOnce(&mut Saved), cx: &mut Context<Self>) {
        change(self.saved_mut());
        self.filtered(cx);
    }

    fn filtered(&mut self, cx: &mut Context<Self>) {
        self.save();
        cx.emit(BoardEvent::Filtered);
        cx.notify();
    }

    pub fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.index();
        cx.emit(BoardEvent::Filtered);
        cx.notify();
    }

    pub fn searching(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn toggle_found(&mut self, pin: usize, cx: &mut Context<Self>) {
        let Load::Ready(map) = &self.map else { return };
        let id = map.pins[pin].id.clone();
        let found = &mut self.saved_mut().found;
        if !found.remove(&id) {
            found.insert(id);
        }
        self.recount();
        self.filtered(cx);
    }

    fn recount(&mut self) {
        let Load::Ready(map) = &self.map else { return };
        self.found_counts = self.counts.iter().map(|groups| vec![0; groups.len()]).collect();
        for pin in map.pins.iter().filter(|pin| self.saved.found.contains(&pin.id)) {
            self.found_counts[pin.group.0][pin.group.1] += 1;
        }
    }

    /// Whether pin `index` shows; a search reaches hidden groups too.
    pub fn shows(&self, map: &Map, index: usize) -> bool {
        let (pin, saved) = (&map.pins[index], &self.saved);
        if saved.unfound_only && saved.found.contains(&pin.id) || saved.teleport_only && pin.position.is_none() {
            return false;
        }
        if self.query.is_empty() { !saved.hidden.contains(&group(map, pin).id) } else { self.haystacks[index].contains(&self.query) }
    }

    /// Whether pin `index` is found by its name (its haystack's first line), not its group or description.
    pub fn named(&self, index: usize) -> bool {
        self.haystacks[index].lines().next().is_some_and(|name| name.contains(&self.query))
    }

    /// Opens a pin's card; `bring` also shows the pin up close.
    pub fn select(&mut self, pin: usize, bring: bool, cx: &mut Context<Self>) {
        let Load::Ready(map) = &self.map else { return };
        if bring {
            cx.emit(BoardEvent::Bring { to: map.pins[pin].at, close: true });
        }
        (self.selected, self.mark) = (Some(pin), None);
        cx.emit(BoardEvent::Selected);
        cx.notify();
    }

    /// A pin a description links to.
    pub fn select_id(&mut self, id: &str, cx: &mut Context<Self>) {
        let Load::Ready(map) = &self.map else { return };
        if let Some(index) = map.pins.iter().position(|pin| pin.id == id) {
            self.select(index, true, cx);
        }
    }

    /// Opens a mark's card; `bring` also centers the view on it.
    pub fn select_mark(&mut self, index: usize, bring: bool, cx: &mut Context<Self>) {
        let Some(mark) = self.saved.marks.get(index) else { return };
        if bring {
            cx.emit(BoardEvent::Bring { to: plane(mark.at), close: false });
        }
        (self.mark, self.selected) = (Some(index), None);
        cx.emit(BoardEvent::Selected);
        cx.notify();
    }

    /// Closes the open card; false when none was open.
    pub fn deselect(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.selected.is_some() || self.mark.is_some();
        if open {
            (self.selected, self.mark) = (None, None);
            cx.emit(BoardEvent::Selected);
            cx.notify();
        }
        open
    }

    /// A pin of the user's own at `at`, opened.
    pub fn add_mark(&mut self, at: Point, cx: &mut Context<Self>) {
        let saved = self.saved_mut();
        let count = saved.marks.len();
        saved.marks.push(Mark { at: (at.x, at.y), name: String::new(), note: String::new(), color: PALETTE[count % PALETTE.len()] });
        saved.marks_shown = true;
        self.select_mark(count, false, cx);
        self.save();
        cx.emit(BoardEvent::MarkAdded);
    }

    pub fn delete_mark(&mut self, index: usize, cx: &mut Context<Self>) {
        self.saved_mut().marks.remove(index);
        self.mark = None;
        self.save();
        cx.emit(BoardEvent::Selected);
        cx.notify();
    }

    /// Changes the open mark; the caller saves once the edit is done.
    pub fn edit_mark(&mut self, edit: impl FnOnce(&mut Mark), cx: &mut Context<Self>) {
        let Some(index) = self.mark else { return };
        if let Some(mark) = self.saved_mut().marks.get_mut(index) {
            edit(mark);
            cx.notify();
        }
    }

    pub fn set_run(&mut self, run: Option<Run>, cx: &mut Context<Self>) {
        self.run = run;
        if self.run.is_none() {
            self.player = None;
        }
        self.watch(cx);
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Whether showing the map has to load it: it failed, or it was let go of.
    pub fn unloaded(&self) -> bool {
        matches!(self.map, Load::Failed(_)) || self.id.is_empty()
    }

    /// Lets go of the map, closed for a while; the user's state of it was saved when it closed.
    pub fn forget(&mut self) {
        (self.id, self.map, self.selected, self.mark) = (String::new(), Load::Pending, None, None);
        (self.saved, self.counts, self.found_counts, self.haystacks) = (Rc::default(), Vec::new(), Vec::new(), Vec::new());
    }

    pub fn set_open(&mut self, open: bool, cx: &mut Context<Self>) {
        self.open = open;
        self.watch(cx);
    }

    /// Asks for the position while the map is open and both the trainer and the map can place the player.
    fn watch(&mut self, cx: &mut Context<Self>) {
        let placed = matches!(&self.map, Load::Ready(map) if map.frame.is_some());
        let session = self.run.as_ref().filter(|run| run.location && placed && self.open).map(|run| run.session.clone());
        let Some(session) = session else {
            self._poll = Task::ready(());
            return;
        };
        self._poll = cx.spawn(async move |board, cx| {
            let mut failures = 0;
            while failures < GIVE_UP {
                match session.position().await {
                    Ok(position) => {
                        failures = 0;
                        let located = board.update(cx, |board, cx| {
                            if board.player.replace(position).is_none() {
                                cx.notify();
                            }
                            cx.emit(BoardEvent::Moved);
                        });
                        if located.is_err() {
                            return;
                        }
                    }
                    Err(e) => {
                        failures += 1;
                        log::debug!("player position: {e:#}");
                    }
                }
                cx.background_executor().timer(EVERY).await;
            }
            log::warn!("the trainer does not tell where the player is");
        });
    }

    pub fn can_teleport(&self, map: &Map) -> bool {
        map.teleport && self.run.as_ref().is_some_and(|run| run.teleport)
    }

    /// Whether the player can be moved to a plane point: the height comes from where the player is.
    pub fn can_teleport_to_plane(&self, map: &Map) -> bool {
        self.player.is_some() && self.can_teleport(map)
    }

    /// Moves the player to `at` on the plane, at the height the player is at now.
    pub fn teleport_to_plane(&mut self, at: Point, cx: &mut Context<Self>) {
        let (Load::Ready(map), Some(player)) = (&self.map, self.player) else { return };
        let Some(frame) = &map.frame else { return };
        let to = frame.to_game(at, frame.height(player));
        self.teleport(to, cx);
    }

    pub fn teleport(&mut self, to: [f64; 3], cx: &mut Context<Self>) {
        let Some(run) = &self.run else { return };
        let session = run.session.clone();
        self.note = None;
        cx.spawn(async move |board, cx| {
            let moved = session.teleport(to).await;
            if let Err(e) = &moved {
                log::warn!("teleport: {e:#}");
            }
            board
                .update(cx, |board, cx| {
                    board.note = moved.err().map(|e| format!("TELEPORT FAILED: {e:#}").to_uppercase());
                    cx.notify();
                })
                .ok();
        })
        .detach();
        cx.notify();
    }
}

pub fn plane((x, y): (f64, f64)) -> Point {
    Point { x, y }
}
