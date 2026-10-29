//! La logique des contrôles potard, telle que la bibliothèque la définit.
use make_it_drum::potard::logic::*;

#[test]
fn fraction_and_value_are_inverse_on_both_curves() {
    for curve in [Curve::Lin, Curve::Log] {
        for v in [-12.0, -3.0, 0.0, 4.5, 12.0] {
            let f = fraction(v, -12.0, 12.0, curve);
            assert!((0.0..=1.0).contains(&f));
            assert!(
                (value_of(f, -12.0, 12.0, curve) - v).abs() < 1e-4,
                "{curve:?} {v}"
            );
        }
    }
    assert_eq!(fraction(0.5, 0.0, 1.0, Curve::Lin), 0.5);
    assert_eq!(
        fraction(0.25, 0.0, 1.0, Curve::Log),
        0.5,
        "taper audio : valeur = position²"
    );
    assert_eq!(value_of(0.5, 0.0, 1.0, Curve::Log), 0.25);
    assert_eq!(fraction(5.0, 1.0, 1.0, Curve::Lin), 0.0, "plage nulle");
    assert_eq!(fraction(99.0, 0.0, 1.0, Curve::Lin), 1.0, "borné");
}

#[test]
fn drag_moves_the_full_range_over_sensitivity_pixels_and_shift_is_ten_times_finer() {
    assert_eq!(drag(0.0, 200.0, 200.0, false), 1.0);
    assert_eq!(drag(0.5, 50.0, 200.0, false), 0.75);
    assert!((drag(0.5, 50.0, 200.0, true) - 0.525).abs() < 1e-6);
    assert_eq!(drag(0.5, -500.0, 200.0, false), 0.0, "borné en bas");
    assert_eq!(
        drag(0.0, 600.0, 600.0, false),
        1.0,
        "sensibilité personnalisée"
    );
    assert_eq!(
        drag(0.0, 200.0, 0.0, false),
        1.0,
        "sensibilité nulle : défaut 200 px"
    );
}

#[test]
fn default_step_is_one_percent_and_stepping_is_clamped() {
    assert_eq!(default_step(-12.0, 12.0), 0.24);
    assert_eq!(stepped(0.0, 1, 0.24, -12.0, 12.0), 0.24);
    assert_eq!(stepped(11.9, 3, 0.24, -12.0, 12.0), 12.0);
    assert_eq!(stepped(-11.9, -2, 0.24, -12.0, 12.0), -12.0);
}

#[test]
fn bipolar_fill_starts_at_zero_only_when_the_range_crosses_zero() {
    assert!(bipolar(-12.0, 12.0));
    assert!(!bipolar(0.0, 1.0));
    assert!(!bipolar(-24.0, 0.0));
    assert_eq!(zero_fraction(-12.0, 12.0), 0.5);
    assert_eq!(zero_fraction(-24.0, 12.0), 24.0 / 36.0);
    assert_eq!(zero_fraction(0.0, 1.0), 0.0);
    assert_eq!(zero_fraction(-1.0, 0.0), 1.0);
    assert_eq!(zero_fraction(2.0, 2.0), 0.0);
}

#[test]
fn stepper_index_wraps_in_both_directions() {
    assert_eq!(wrap(3, 3), 0);
    assert_eq!(wrap(-1, 3), 2);
    assert_eq!(wrap(-4, 3), 2);
    assert_eq!(wrap(7, 0), 0);
}

#[test]
fn vumeter_lights_segments_in_three_zones() {
    assert_eq!(lit_segments(0.0, 12), 0);
    assert_eq!(lit_segments(0.5, 12), 6);
    assert_eq!(lit_segments(1.0, 12), 12);
    assert_eq!(lit_segments(2.0, 12), 12, "borné");
    let zones: Vec<u8> = (0..12).map(|i| segment_zone(i, 12)).collect();
    assert_eq!(zones, [0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2]);
}
