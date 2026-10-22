// L'état persistant du plugin se sérialise en JSON (format que nih-plug stocke dans le preset).
use make_it_drum::loader::KitState;

#[test]
fn kit_state_round_trips_through_json() {
    let s = KitState {
        path: Some("/a/b.zip".into()),
        name: Some("b".into()),
        zip_base64: Some("AAAA".into()),
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: KitState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    let old: KitState = serde_json::from_str("{}").unwrap();
    assert_eq!(old, KitState::default(), "un preset sans kit se lit");
}
