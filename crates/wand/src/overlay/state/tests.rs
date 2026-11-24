use futures::{
    FutureExt, StreamExt,
    channel::mpsc::{self, UnboundedReceiver},
    executor::block_on_stream,
};

use super::*;

const GAME: u32 = 4242;
const RECT: Rect = Rect { x: 10, y: 20, width: 300, height: 200 };
const FRAME: Texture = Texture { handle: 77, width: 300, height: 200, adapter: 10370 };

fn process(created: u64) -> Process {
    Process { pid: GAME, path: "game.exe".into(), created }
}

/// A canvas bound to `GAME`, whose DLL linked and, when `hooked`, reported its id and adapter.
fn linked(hooked: bool) -> (State, UnboundedReceiver<CanvasEvent>) {
    let mut state = State::default();
    state.record(&process(1), Load::Running);
    state.linked(GAME, 1);
    if hooked {
        hook(&mut state);
    }
    let (events, received) = mpsc::unbounded();
    state.bind(GAME, events);
    (state, received)
}

fn hook(state: &mut State) {
    let client = state.clients.get_mut(&GAME).unwrap();
    (client.id, client.adapter) = (Some(5), Some(10370));
}

fn show(state: &mut State, rect: Rect, frame: Texture) {
    state.canvases.get_mut(&GAME).unwrap().want(0, Some((rect, frame)));
}

fn hide(state: &mut State) {
    state.canvases.get_mut(&GAME).unwrap().want(0, None);
}

/// The messages that go, as JSON.
fn bodies(state: &mut State) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    state.pending(&mut out);
    let body = |(pid, window, frame): &Sending| {
        assert_eq!((*pid, *window), (GAME, 1));
        let (id, body) = wire::unpack(frame).unwrap();
        assert_eq!(id, 5, "the DLL's client id is echoed");
        serde_json::from_str(body).unwrap()
    };
    out.iter().map(body).collect()
}

/// The types of the messages that go.
fn sent(state: &mut State) -> Vec<String> {
    bodies(state).iter().map(|body| body["type"].as_str().unwrap().to_owned()).collect()
}

fn ended(events: UnboundedReceiver<CanvasEvent>) -> Vec<CanvasEvent> {
    block_on_stream(events).collect()
}

#[test]
fn a_layer_is_described_once_and_each_new_frame_is_sent_once() {
    let (mut state, events) = linked(true);
    show(&mut state, RECT, FRAME);
    assert_eq!(sent(&mut state), ["global-init-config", "offscreen-window-info", "offscreen-window-frame-update"]);
    assert!(sent(&mut state).is_empty());
    // Two draws before the host's turn: one frame goes.
    show(&mut state, RECT, FRAME);
    show(&mut state, RECT, FRAME);
    assert_eq!(sent(&mut state), ["offscreen-window-frame-update"]);
    show(&mut state, Rect { x: 0, ..RECT }, FRAME);
    assert_eq!(sent(&mut state), ["offscreen-window-info", "offscreen-window-frame-update"]);
    hide(&mut state);
    assert_eq!(sent(&mut state), ["close-offscreen-window"]);
    assert!(sent(&mut state).is_empty());
    drop(state);
    assert_eq!(ended(events), [CanvasEvent::Ready]);
}

#[test]
fn a_new_texture_of_the_same_size_sends_only_its_frame() {
    let (mut state, _events) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    show(&mut state, RECT, Texture { handle: 78, ..FRAME });
    let bodies = bodies(&mut state);
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0]["shared_handle"].as_u64(), Some(78));
}

#[test]
fn after_a_late_send_the_whole_state_goes_again_and_a_hidden_layer_is_closed_again() {
    let (mut state, _events) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    state.clients.get_mut(&GAME).unwrap().unsure = true;
    assert_eq!(sent(&mut state), ["global-init-config", "offscreen-window-info", "offscreen-window-frame-update"]);
    hide(&mut state);
    sent(&mut state);
    state.clients.get_mut(&GAME).unwrap().unsure = true;
    assert_eq!(sent(&mut state), ["global-init-config", "close-offscreen-window"]);
}

#[test]
fn a_resize_sends_each_shown_frame_again() {
    let (mut state, _events) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    state.resend(GAME);
    assert_eq!(sent(&mut state), ["offscreen-window-frame-update"]);
}

#[test]
fn a_closing_canvas_closes_its_layers_and_its_events_end_while_the_link_stays() {
    let (mut state, events) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    state.canvases.get_mut(&GAME).unwrap().closing = true;
    assert_eq!(sent(&mut state), ["close-offscreen-window"]);
    assert!(state.canvases.is_empty());
    assert_eq!(state.clients.len(), 1);
    assert_eq!(ended(events), [CanvasEvent::Ready]);
}

#[test]
fn a_canvas_closing_before_the_dll_said_anything_ends_without_a_word() {
    let (mut state, events) = linked(false);
    state.canvases.get_mut(&GAME).unwrap().closing = true;
    assert!(sent(&mut state).is_empty());
    assert!(state.canvases.is_empty());
    assert!(ended(events).is_empty());
}

#[test]
fn the_host_rests_only_without_a_canvas_and_a_link_of_ours_and_lets_the_others_go() {
    let (mut state, _events) = linked(true);
    state.window = 1;
    state.clients.insert(7, Client::new(70));
    assert_eq!(state.idle(), None, "a canvas draws");
    state.canvases.get_mut(&GAME).unwrap().closing = true;
    sent(&mut state);
    assert_eq!(state.idle(), None, "the game we loaded the DLL into stays linked");
    state.clients.remove(&GAME);
    assert_eq!(state.idle(), Some(vec![70]), "a stranger's link goes");
    assert!(state.dormant && state.clients.is_empty());
    assert_eq!(state.idle(), None, "it rests once");
}

#[test]
fn an_interactive_layer_is_described_again_and_takes_input() {
    let (mut state, _events) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    state.canvases.get_mut(&GAME).unwrap().interact(0, true);
    let bodies = bodies(&mut state);
    let types: Vec<_> = bodies.iter().map(|body| body["type"].as_str().unwrap()).collect();
    assert_eq!(types, ["offscreen-window-info", "offscreen-window-frame-update"]);
    assert_eq!(bodies[0]["accept_input"].as_bool(), Some(true));
}

/// Layer 0 shown with the input and the global block asked for, all of it sent.
fn blocked(state: &mut State) {
    let bound = state.canvases.get_mut(&GAME).unwrap();
    bound.interact(0, true);
    bound.block = true;
    show(state, RECT, FRAME);
    sent(state);
}

fn unblocked(bodies: &[serde_json::Value]) -> bool {
    bodies.last().is_some_and(|body| body["type"] == "global-block" && body["block"] == false)
}

#[test]
fn a_cursor_goes_once_per_change_and_again_with_a_window_described_anew() {
    let (mut state, _events) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    let cursor = |state: &mut State, cursor| state.canvases.get_mut(&GAME).unwrap().cursor(0, cursor);
    cursor(&mut state, Cursor::Hand);
    let bodies = bodies(&mut state);
    assert_eq!((bodies.len(), bodies[0]["cursor"].as_str()), (1, Some("IDC_HAND")));
    cursor(&mut state, Cursor::Hand);
    assert!(sent(&mut state).is_empty());
    show(&mut state, Rect { x: 0, ..RECT }, FRAME);
    assert_eq!(sent(&mut state), ["offscreen-window-info", "offscreen-window-frame-update", "offscreen-window-cursor-update"]);
}

#[test]
fn the_global_block_goes_only_over_an_interactive_layer_again_when_unsure_and_ends_with_the_canvas() {
    let (mut state, events) = linked(true);
    sent(&mut state);
    state.canvases.get_mut(&GAME).unwrap().block = true;
    assert!(sent(&mut state).is_empty(), "nothing shown takes the input");
    state.canvases.get_mut(&GAME).unwrap().interact(0, true);
    show(&mut state, RECT, FRAME);
    assert_eq!(sent(&mut state), ["offscreen-window-info", "offscreen-window-frame-update", "global-block"]);
    assert!(sent(&mut state).is_empty());
    state.clients.get_mut(&GAME).unwrap().unsure = true;
    assert_eq!(sent(&mut state), ["global-init-config", "offscreen-window-info", "offscreen-window-frame-update", "global-block"]);
    state.canvases.get_mut(&GAME).unwrap().closing = true;
    assert!(unblocked(&bodies(&mut state)));
    assert_eq!(ended(events), [CanvasEvent::Ready]);
}

#[test]
fn a_closed_canvas_s_last_words_go_again_while_sends_are_late() {
    let (mut state, _events) = linked(true);
    blocked(&mut state);
    state.canvases.get_mut(&GAME).unwrap().closing = true;
    assert_eq!(sent(&mut state), ["close-offscreen-window", "global-block"]);
    for _ in 0..2 {
        state.clients.get_mut(&GAME).unwrap().unsure = true;
        let bodies = bodies(&mut state);
        assert_eq!(bodies.len(), 3, "the init, then the last words");
        assert!(unblocked(&bodies));
    }
    assert!(sent(&mut state).is_empty(), "said until a flush went through");
}

#[test]
fn a_new_canvas_lifts_the_old_one_s_block() {
    let (mut state, _old) = linked(true);
    blocked(&mut state);
    let (events, _new) = mpsc::unbounded();
    state.bind(GAME, events);
    let bodies = bodies(&mut state);
    assert_eq!(bodies.len(), 2);
    assert!(unblocked(&bodies));
}

#[test]
fn the_dll_s_words_reach_a_canvas_only_once_ready() {
    let (mut state, mut events) = linked(false);
    state.heard(GAME, CanvasEvent::Resized { width: 1, height: 1 });
    assert!(events.next().now_or_never().is_none());
    hook(&mut state);
    sent(&mut state);
    let input = CanvasEvent::Input { layer: 0, message: 512, wparam: 0, lparam: 0 };
    state.heard(GAME, input);
    drop(state);
    assert_eq!(ended(events), [CanvasEvent::Ready, input]);
}

#[test]
fn a_texture_on_another_gpu_is_never_sent_and_ends_the_canvas_not_the_link() {
    let (mut state, events) = linked(true);
    assert_eq!(sent(&mut state), ["global-init-config"]);
    show(&mut state, RECT, Texture { adapter: 1, ..FRAME });
    assert!(sent(&mut state).is_empty());
    assert!(state.canvases.is_empty());
    assert_eq!(state.clients.len(), 1);
    assert_eq!(ended(events), [CanvasEvent::Ready]);
}

#[test]
fn nothing_shows_before_the_dll_reported_its_hook() {
    let (mut state, mut events) = linked(false);
    show(&mut state, RECT, FRAME);
    assert!(sent(&mut state).is_empty());
    assert!(events.next().now_or_never().is_none(), "not ready yet");
    hook(&mut state);
    assert_eq!(sent(&mut state), ["global-init-config", "offscreen-window-info", "offscreen-window-frame-update"]);
    assert_eq!(events.next().now_or_never(), Some(Some(CanvasEvent::Ready)));
}

#[test]
fn a_new_canvas_closes_the_old_one_s_layers_unless_it_shows_them_and_the_old_events_end() {
    let (mut state, old) = linked(true);
    show(&mut state, RECT, FRAME);
    sent(&mut state);
    let (events, _new) = mpsc::unbounded();
    state.bind(GAME, events);
    assert_eq!(sent(&mut state), ["close-offscreen-window"]);
    assert_eq!(ended(old), [CanvasEvent::Ready]);
    show(&mut state, RECT, FRAME);
    assert_eq!(sent(&mut state), ["offscreen-window-info", "offscreen-window-frame-update"]);
}

#[test]
fn a_canvas_bound_after_the_dll_reported_its_size_learns_it_once_ready() {
    let mut state = State::default();
    state.record(&process(1), Load::Running);
    state.linked(GAME, 1);
    hook(&mut state);
    state.clients.get_mut(&GAME).unwrap().size = Some((1772, 1041));
    let (events, received) = mpsc::unbounded();
    state.bind(GAME, events);
    sent(&mut state);
    drop(state);
    assert_eq!(ended(received), [CanvasEvent::Ready, CanvasEvent::Resized { width: 1772, height: 1041 }]);
}

#[test]
fn a_new_process_under_an_old_pid_drops_the_old_link_and_canvas() {
    let (mut state, events) = linked(true);
    state.record(&process(2), Load::Running);
    assert!(state.clients.is_empty() && state.canvases.is_empty());
    assert!(ended(events).is_empty());
    assert!(state.mapped.get(&GAME).is_some_and(|(known, _)| *known == process(2)));
}
