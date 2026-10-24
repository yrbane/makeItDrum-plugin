use make_it_drum::slot::KitSlot;
use make_it_drum_core::kit::Kit;
use std::sync::Arc;

fn kit(name: &str) -> Arc<Kit> {
    Arc::new(Kit {
        name: name.into(),
        pads: vec![],
        source: None,
    })
}

#[test]
fn starts_empty_at_generation_zero() {
    let s = KitSlot::new();
    assert_eq!(s.generation(), 0);
    assert_eq!(s.try_current().unwrap(), None);
}

#[test]
fn publish_bumps_generation_and_exposes_the_kit() {
    let s = KitSlot::new();
    s.publish(kit("a"));
    assert_eq!(s.generation(), 1);
    assert_eq!(s.try_current().unwrap().unwrap().name, "a");
    s.clear();
    assert_eq!(s.generation(), 2);
    assert_eq!(s.try_current().unwrap(), None);
}

#[test]
fn previous_kits_stay_alive_so_the_audio_thread_never_frees_them() {
    let s = KitSlot::new();
    let first = kit("a");
    let weak = Arc::downgrade(&first);
    s.publish(first);
    s.publish(kit("b"));
    s.publish(kit("c"));
    assert!(
        weak.upgrade().is_some(),
        "gardé vivant après deux remplacements"
    );
    for i in 0..6 {
        s.publish(kit(&format!("k{i}")));
    }
    assert!(
        weak.upgrade().is_none(),
        "fini par être libéré par le thread de fond"
    );
}

#[test]
fn triggers_accumulate_and_are_taken_at_once() {
    let s = KitSlot::new();
    assert_eq!(s.take_triggers(), 0);
    s.request_trigger(0);
    s.request_trigger(19);
    s.request_trigger(25);
    assert_eq!(s.take_triggers(), 1 | (1 << 19));
    assert_eq!(s.take_triggers(), 0);
}

#[test]
fn playing_mask_and_status_round_trip() {
    let s = KitSlot::new();
    s.set_playing(0b101);
    assert_eq!(s.playing(), 0b101);
    s.set_status("Kit chargé");
    assert_eq!(s.status(), "Kit chargé");
}
