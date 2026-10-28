use make_it_drum::variations::{Variations, DEPTH};

fn seeds(n: u32) -> Vec<Option<u32>> {
    (0..20).map(|i| Some(n * 100 + i)).collect()
}

#[test]
fn vide_au_depart_rien_a_parcourir() {
    let v = Variations::new();
    assert_eq!(v.position(), (0, 0));
    assert!(!v.can_back() && !v.can_forward());
    assert_eq!(v.back(), None);
    assert_eq!(v.forward(), None);
}

#[test]
fn record_avance_le_curseur_et_back_retrouve_la_serie_precedente() {
    let v = Variations::new();
    v.reset(seeds(1));
    v.record(seeds(2));
    v.record(seeds(3));
    assert_eq!(v.position(), (3, 3));
    assert_eq!(v.back(), Some(seeds(2)));
    assert_eq!(v.back(), Some(seeds(1)));
    assert_eq!(v.back(), None);
    assert_eq!(v.position(), (1, 3));
    assert_eq!(v.forward(), Some(seeds(2)));
    assert!(v.can_back() && v.can_forward());
}

#[test]
fn record_apres_un_retour_oublie_la_suite() {
    let v = Variations::new();
    v.reset(seeds(1));
    v.record(seeds(2));
    v.record(seeds(3));
    v.back();
    v.back();
    v.record(seeds(9));
    assert_eq!(v.position(), (2, 2));
    assert_eq!(v.forward(), None);
    assert_eq!(v.back(), Some(seeds(1)));
}

#[test]
fn une_serie_identique_a_la_courante_nest_pas_ajoutee() {
    let v = Variations::new();
    v.reset(seeds(1));
    v.record(seeds(1));
    assert_eq!(v.position(), (1, 1));
}

#[test]
fn reset_repart_dune_seule_serie() {
    let v = Variations::new();
    v.reset(seeds(1));
    v.record(seeds(2));
    v.reset(seeds(7));
    assert_eq!(v.position(), (1, 1));
    assert_eq!(v.back(), None);
}

#[test]
fn la_profondeur_est_bornee() {
    let v = Variations::new();
    v.reset(seeds(0));
    for n in 1..(DEPTH as u32 + 10) {
        v.record(seeds(n));
    }
    assert_eq!(v.position(), (DEPTH, DEPTH));
    let mut oldest = None;
    while let Some(s) = v.back() {
        oldest = Some(s);
    }
    assert_eq!(oldest, Some(seeds(10)));
}
