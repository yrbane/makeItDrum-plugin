use make_it_drum::history::{History, DEPTH};
use make_it_drum::loader::KitState;
use make_it_drum_core::kit::Kit;
use std::sync::Arc;

fn kit(name: &str) -> Arc<Kit> {
    Arc::new(Kit::empty(name))
}

#[test]
fn undo_returns_kits_most_recent_first_and_is_bounded() {
    let h = History::new();
    assert!(h.is_empty());
    assert!(h.pop().is_none());
    for i in 0..(DEPTH + 3) {
        h.push(kit(&format!("k{i}")), KitState::default());
    }
    assert_eq!(h.len(), DEPTH);
    let (last, _) = h.pop().unwrap();
    assert_eq!(last.name, format!("k{}", DEPTH + 2));
    let (prev, _) = h.pop().unwrap();
    assert_eq!(prev.name, format!("k{}", DEPTH + 1));
    assert_eq!(h.len(), DEPTH - 2);
    h.clear();
    assert!(h.is_empty());
}
