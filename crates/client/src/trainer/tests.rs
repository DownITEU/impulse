use model::{Args, Cheat, Kind};

use super::{Span, Step, TOP_TICK, Values, changes, round_to, step};

fn cheat(id: &str, kind: Kind, args: Args) -> Cheat {
    Cheat {
        id: id.into(),
        key: None,
        name: String::new(),
        category: String::new(),
        kind,
        args,
        hotkeys: Vec::new(),
        instructions: None,
        description: None,
    }
}

#[test]
fn kept_values_change_only_what_they_can() {
    let cheats = [
        cheat("on", Kind::Toggle, Args::default()),
        cheat("same", Kind::Toggle, Args::default()),
        cheat("far", Kind::Slider, Args { min: Some(1.), max: Some(5.), ..Args::default() }),
        cheat("pick", Kind::Selection, Args { options: vec!["a".into(), "b".into()], ..Args::default() }),
        cheat("press", Kind::Button, Args::default()),
    ];
    let wanted: Values =
        [("on", 1.), ("same", 1.), ("far", 9.), ("pick", 2.), ("press", 1.), ("gone", 1.)].map(|(id, value)| (id.to_owned(), value)).into();
    assert_eq!(changes(&cheats, &[0., 1., 1., 0., 0.], &wanted), [(0, 1.), (2, 5.)]);
}

#[test]
fn a_pair_steps_by_its_step_and_stops_at_the_ends() {
    let speed = cheat("speed", Kind::Slider, Args { min: Some(0.5), max: Some(2.5), step: Some(1.), ..Args::default() });
    assert_eq!(step(&speed, 0.5, 1), Some(Step::Set(1.5)));
    assert_eq!(step(&speed, 1.5, 0), Some(Step::Set(0.5)));
    assert_eq!(step(&speed, 2.5, 1), None);
    assert_eq!(step(&speed, 0.5, 0), None);
    let unstepped = cheat("gold", Kind::Number, Args { step: Some(0.), ..Args::default() });
    assert_eq!(step(&unstepped, 3., 1), Some(Step::Set(4.)));
}

#[test]
fn a_selection_wraps_and_an_empty_one_does_nothing() {
    let pick = cheat("pick", Kind::Selection, Args { options: vec!["a".into(), "b".into(), "c".into()], ..Args::default() });
    assert_eq!(step(&pick, 2., 1), Some(Step::Set(0.)));
    assert_eq!(step(&pick, 0., 0), Some(Step::Set(2.)));
    assert_eq!(step(&cheat("none", Kind::Selection, Args::default()), 0., 1), None);
}

#[test]
fn a_toggle_flips_a_button_presses_and_an_unknown_does_nothing() {
    assert_eq!(step(&cheat("god", Kind::Toggle, Args::default()), 0., 0), Some(Step::Set(1.)));
    assert_eq!(step(&cheat("god", Kind::Toggle, Args::default()), 1., 0), Some(Step::Set(0.)));
    assert_eq!(step(&cheat("heal", Kind::Button, Args { pressed: 7., ..Args::default() }), 0., 0), Some(Step::Press(7.)));
    assert_eq!(step(&cheat("odd", Kind::Unknown, Args::default()), 0., 0), None);
}

#[test]
fn hotkey_steps_keep_the_origin_and_do_not_drift() {
    let half = Args { min: Some(0.5), ..Args::default() };
    assert_eq!(round_to(0.5 + 1., &half, 1.), 1.5);
    assert_eq!(round_to(0.2 + 0.1, &Args::default(), 0.1), 0.3);
}

#[test]
fn steps_count_from_min() {
    let span = Span { min: 3., max: 21., step: 2. };
    assert_eq!(span.value(0), 3.);
    assert_eq!(span.value(TOP_TICK), 21.);
    assert!((0..=TOP_TICK).all(|tick| (span.value(tick) - 3.) % 2. == 0.));
}

#[test]
fn wheel_moves_a_coarse_step() {
    let span = Span { min: 0., max: 10., step: 5. };
    assert_eq!(span.nudge(0., true), 5.);
    assert_eq!(span.nudge(5., true), 10.);
    assert_eq!(span.nudge(10., true), 10.);
    assert_eq!(span.nudge(5., false), 0.);
}
