// Les valeurs de référence sont celles du JS de la page, recopiées telles quelles.
#![allow(clippy::excessive_precision)]
// Le planificateur tire les mêmes valeurs que la page pour la même graine :
// comparaison aux snapshots du renderer Web Audio (seed 42, analyse fixée).
use make_it_drum_core::analysis::Analysis;
use make_it_drum_core::dsp::FilterKind;
use make_it_drum_core::flavors::FLAVORS;
use make_it_drum_core::prng::Random;
use make_it_drum_core::render::{
    plan, plan_with, render, render_pad, render_pad_with, Rate, Shaping, SourceSample,
    MAX_STRETCHED_TOTAL,
};

const ANALYSIS: Analysis = Analysis {
    centroid: 1800.0,
    fundamental: 62.0,
    mid_peak: 1100.0,
    brightness: 0.35,
    crest: 4.0,
};

fn snapshot_source() -> SourceSample {
    let n = 22050;
    let data: Vec<f32> = (0..n)
        .map(|i| ((i as f64 * 0.01).sin() * 0.8) as f32)
        .collect();
    let reversed = data.iter().rev().copied().collect();
    SourceSample {
        data,
        reversed,
        sample_rate: 44100,
        analysis: ANALYSIS,
    }
}

fn flavor(name: &str) -> &'static make_it_drum_core::flavors::Flavor {
    FLAVORS.iter().find(|f| f.name == name).unwrap()
}

fn eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

macro_rules! check {
    ($a:expr, $b:expr) => {
        assert!(eq($a, $b), "{} = {} ≠ {}", stringify!($a), $a, $b)
    };
}

#[test]
fn boom_plan_matches_the_page_snapshot() {
    let src = snapshot_source();
    let p = plan(flavor("BOOM"), &src, &mut Random::from_seed(42));
    assert_eq!(p.timings.length, 33272);
    assert_eq!(p.sources.len(), 4, "transitoire, 2 couches, sub");
    let tr = &p.sources[0];
    check!(
        matches!(tr.rate, Rate::Const(r) if eq(r, 2.1697340414393693)) as u8 as f64,
        1.0
    );
    let f = tr.filter.as_ref().unwrap();
    assert_eq!(f.kind, FilterKind::Highpass);
    check!(f.freq, 1486.3973351370078);
    check!(tr.env_duration, 0.006197557576186955);
    assert_eq!(tr.offset, 0.0);

    let l1 = &p.sources[1];
    assert_eq!(l1.rate, Rate::Const(0.3106998704578728));
    let f = l1.filter.as_ref().unwrap();
    assert_eq!(f.kind, FilterKind::Lowpass);
    check!(f.freq, 244.97923705257475);
    check!(f.q, 0.594967968375422);
    check!(l1.env_duration, 0.5090858847724273);
    check!(l1.offset, 0.2999000033875927);

    let l2 = &p.sources[2];
    assert_eq!(l2.rate, Rate::Const(0.17179540311917663));
    let f = l2.filter.as_ref().unwrap();
    assert_eq!(f.kind, FilterKind::Bandpass);
    check!(f.freq, 113.47000257268547);
    check!(f.q, 0.900187533046119);
    check!(l2.env_duration, 0.38731772483508103);
    check!(l2.offset, 0.20780273741343988);

    let sub = &p.sources[3];
    assert_eq!(sub.rate, Rate::Const(0.1900770489675924));
    let f = sub.filter.as_ref().unwrap();
    assert_eq!(f.kind, FilterKind::Lowpass);
    check!(f.freq, 45.32037879670038);
    assert_eq!(f.q, 1.0, "Q par défaut de Web Audio");
    assert!(sub.shaper.is_some());
    check!(sub.env_duration, 0.557793148747366);
    check!(sub.offset, 0.16006585407070814);

    assert!(p.phaser.is_none() && p.comb.is_none() && p.sat.fold.is_none());
    let b = p.bass.as_ref().unwrap();
    check!(b.freq, 77.76971971616149);
    check!(b.q, 0.9);
    check!(b.gain_db, 4.875691093504429);
    let r = p.reverb.as_ref().unwrap();
    assert_eq!(r.impulse.len(), 6411);
    check!(r.wet, 0.15337080620229246);
    check!(p.master.ratio, 10.708558407705278);
    check!(p.master.release, 0.10462399534927681);
    check!(p.master.out_gain, 1.2773500874778256);
    assert_eq!(p.crush, None);
    assert_eq!(p.peak, 0.97);
}

#[test]
fn chaos_plan_matches_the_page_snapshot() {
    let src = snapshot_source();
    let p = plan(flavor("CHAOS"), &src, &mut Random::from_seed(42));
    assert_eq!(p.timings.length, 23667);
    assert_eq!(p.sources.len(), 3);
    let l1 = &p.sources[1];
    assert_eq!(l1.rate, Rate::Const(1.5534993522893639));
    let f = l1.filter.as_ref().unwrap();
    check!(f.freq, 1086.6014546686783);
    check!(f.q, 1.189935936750844);
    check!(l1.env_duration, 0.1312815353394486);
    check!(l1.offset, 0.2999000033875927);
    let l2 = &p.sources[2];
    assert_eq!(l2.rate, Rate::Const(3.006419554585591));
    let f = l2.filter.as_ref().unwrap();
    assert_eq!(f.kind, FilterKind::Highpass);
    check!(f.freq, 4049.2319466462354);
    check!(f.q, 0.7365170724922792);
    check!(l2.env_duration, 0.0679620921720285);
    check!(l2.offset, 0.09513860267121345);

    let ph = p.phaser.as_ref().unwrap();
    let (rate, depth) = ph.lfo.unwrap();
    check!(rate, 3.558295074403286);
    check!(depth, 373.2303886697628);
    let expected = [
        (744.4022612215485, 0.6877284079696983),
        (1793.3221400293987, 0.5115078918449582),
        (2488.270151698287, 0.503245941363275),
    ];
    assert_eq!(ph.stages.len(), 3);
    for (got, want) in ph.stages.iter().zip(expected) {
        check!(got.0, want.0);
        check!(got.1, want.1);
    }
    let c = p.comb.as_ref().unwrap();
    check!(c.delay, 0.003870781164499931);
    check!(c.feedback, 0.2948275341838598);
    check!(c.mix, -0.2839946915046312);
    assert!(p.sat.fold.is_some());
    let b = p.bass.as_ref().unwrap();
    check!(b.freq, 82.57419890910387);
    check!(b.gain_db, 5.685376250650734);
    let r = p.reverb.as_ref().unwrap();
    assert_eq!(r.impulse.len(), 6411);
    check!(r.wet, 0.18957162377703932);
    check!(p.master.ratio, 9.84417697833851);
    check!(p.master.release, 0.07650311325257644);
    check!(p.master.out_gain, 1.3306306737381965);
}

#[test]
fn tick_plan_matches_the_page_snapshot_and_has_no_reverb() {
    let src = snapshot_source();
    let p = plan(flavor("TICK"), &src, &mut Random::from_seed(42));
    assert_eq!(p.timings.length, 5322);
    assert_eq!(p.sources.len(), 2);
    assert_eq!(p.sources[0].rate, Rate::Const(2.35246579349041));
    check!(
        p.sources[0].filter.as_ref().unwrap().freq,
        1925.6389617774403
    );
    check!(p.sources[0].env_duration, 0.00726168867717497);
    let l = &p.sources[1];
    assert_eq!(l.rate, Rate::Const(3.734724226249382));
    check!(l.filter.as_ref().unwrap().freq, 8212.722580182832);
    check!(l.filter.as_ref().unwrap().q, 0.8850512097543105);
    check!(l.env_duration, 0.020663567841127516);
    check!(l.offset, 0.06622978957020678);
    assert!(p.reverb.is_none() && p.bass.is_none() && p.phaser.is_none() && p.comb.is_none());
    check!(p.master.ratio, 7.910113816615194);
    check!(p.master.release, 0.09003939233813435);
    check!(p.master.out_gain, 1.0363104994315655);
}

#[test]
fn rendered_pads_are_normalized_finite_and_seeded() {
    let src = snapshot_source();
    for cfg in FLAVORS.iter() {
        let s = render_pad(cfg, &src, 42);
        assert_eq!(s.sample_rate, 44100);
        assert_eq!(s.channels, 1);
        let p = plan(cfg, &src, &mut Random::from_seed(42));
        assert_eq!(s.data.len(), p.timings.length, "{}", cfg.name);
        assert!(s.data.iter().all(|v| v.is_finite()), "{} : NaN", cfg.name);
        let peak = s.data.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            (peak - cfg.peak as f32).abs() < 1e-5,
            "{} : crête {peak} ≠ {}",
            cfg.name,
            cfg.peak
        );
        let again = render_pad(cfg, &src, 42);
        assert_eq!(again, s, "{} : même graine, même son", cfg.name);
        let other = render_pad(cfg, &src, 43);
        assert_ne!(other.data, s.data, "{} : autre graine, autre son", cfg.name);
    }
}

#[test]
fn render_of_the_fixture_sound_works_at_22050_hz_with_a_pitch_bend_and_a_reverse() {
    let wav = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/source.wav"
    ))
    .unwrap();
    let sample = make_it_drum_core::wav::decode_wav(&wav).unwrap();
    let src = SourceSample::prepare(&sample);
    assert_eq!(src.reversed[0], src.data[src.data.len() - 1]);
    for name in ["BEND", "SWELL", "WASH"] {
        let cfg = flavor(name);
        let p = plan(cfg, &src, &mut Random::from_seed(7));
        let out = render(&p, &src);
        assert_eq!(out.len(), p.timings.length);
        assert!(out.iter().all(|v| v.is_finite()));
        let energy: f64 = out.iter().map(|v| (*v as f64).powi(2)).sum();
        assert!(energy > 1.0, "{name} : rendu quasi muet ({energy})");
    }
}

#[test]
fn prepare_mixes_stereo_down_to_mono() {
    let s = make_it_drum_core::wav::Sample {
        data: vec![0.5, -0.5, 1.0, 0.0, 0.2, 0.2],
        channels: 2,
        sample_rate: 48000,
    };
    let src = SourceSample::prepare(&s);
    assert_eq!(src.data, vec![0.0, 0.5, 0.2]);
    assert_eq!(src.to_sample().channels, 1);
    assert!((src.duration() - 3.0 / 48000.0).abs() < 1e-12);
}

#[test]
fn default_shaping_is_the_page_and_fidelity_keeps_the_draws() {
    let src = snapshot_source();
    for cfg in FLAVORS.iter() {
        let page = plan(cfg, &src, &mut Random::from_seed(42));
        let same = plan_with(cfg, &src, &mut Random::from_seed(42), &Shaping::default());
        assert_eq!(same, page, "{} : cadrage nul = page", cfg.name);
        let faithful = plan_with(
            cfg,
            &src,
            &mut Random::from_seed(42),
            &Shaping::new(1.0, 0.0),
        );
        assert_eq!(
            faithful.timings, page.timings,
            "{} : mêmes durées",
            cfg.name
        );
        assert_eq!(
            faithful.reverb.as_ref().map(|r| r.impulse.clone()),
            page.reverb.as_ref().map(|r| r.impulse.clone()),
            "{} : même impulsion, donc mêmes tirages",
            cfg.name
        );
        assert_eq!(
            render_pad_with(cfg, &src, 42, &Shaping::default()),
            render_pad(cfg, &src, 42)
        );
    }
}

#[test]
fn full_fidelity_plays_the_sound_as_is_without_effects() {
    let src = snapshot_source();
    let sr = 44100.0;
    for cfg in FLAVORS.iter() {
        let p = plan_with(
            cfg,
            &src,
            &mut Random::from_seed(42),
            &Shaping::new(1.0, 0.0),
        );
        // sources : le transitoire (index 0) garde sa recette, les couches lisent à vitesse 1 depuis le début
        for s in p.sources.iter().skip(1).take(cfg.layers.len()) {
            match &s.rate {
                Rate::Const(r) => check!(*r, 1.0),
                Rate::Ramp { from, to, .. } => {
                    check!(*from, 1.0);
                    check!(*to, 1.0);
                }
            }
            check!(s.offset, 0.0);
            assert!(!s.reverse, "{} : plus de lecture inversée", cfg.name);
            if let Some(f) = &s.filter {
                match f.kind {
                    FilterKind::Lowpass => check!(f.freq, sr * 0.45),
                    FilterKind::Highpass => check!(f.freq, 30.0),
                    FilterKind::Bandpass => check!(f.q, 0.25),
                    _ => {}
                }
            }
        }
        if let Some(c) = &p.comb {
            check!(c.mix, 0.0);
        }
        if let Some(r) = &p.reverb {
            check!(r.wet, 0.0);
        }
        if let Some(b) = &p.bass {
            check!(b.gain_db, 0.0);
        }
        if let Some(ph) = &p.phaser {
            if let Some((_, depth)) = ph.lfo {
                check!(depth, 0.0);
            }
        }
        if cfg.sub.is_some() {
            let sub = p.sources.last().unwrap();
            assert!(
                sub.env.iter().all(|v| *v == 0.0),
                "{} : renfort grave muet",
                cfg.name
            );
        }
        check!(p.master.ratio, 1.0);
        assert_eq!(p.crush, None, "{} : plus de bitcrush", cfg.name);
        // les tables de saturation sont l'identité
        for (i, v) in p.sat.drive1.iter().enumerate() {
            let x = (i as f64 / 1023.0) * 2.0 - 1.0;
            assert!(
                (*v as f64 - x).abs() < 1e-6,
                "{} : drive1[{i}] = {v} ≠ {x}",
                cfg.name
            );
        }
        assert!(p.sat.fold.as_ref().is_none_or(|f| {
            f.iter()
                .enumerate()
                .all(|(i, v)| (*v as f64 - ((i as f64 / 1023.0) * 2.0 - 1.0)).abs() < 1e-6)
        }));
        let s = render_pad_with(cfg, &src, 42, &Shaping::new(1.0, 0.0));
        assert!(s.data.iter().all(|v| v.is_finite()));
        assert_ne!(
            s.data,
            render_pad(cfg, &src, 42).data,
            "{} : le cadrage change le son",
            cfg.name
        );
    }
}

#[test]
fn half_fidelity_sits_between_the_page_and_the_source() {
    let src = snapshot_source();
    let page = plan(flavor("BOOM"), &src, &mut Random::from_seed(42));
    let half = plan_with(
        flavor("BOOM"),
        &src,
        &mut Random::from_seed(42),
        &Shaping::new(0.5, 0.0),
    );
    fn start(r: &Rate) -> f64 {
        match *r {
            Rate::Const(v) => v,
            Rate::Ramp { from, .. } => from,
        }
    }
    let pf = start(&page.sources[1].rate);
    assert!(pf != 1.0, "la première couche de BOOM est transposée");
    check!(start(&half.sources[1].rate), (pf + 1.0) / 2.0);
    check!(half.sources[1].offset, page.sources[1].offset * 0.5);
    check!(
        half.reverb.as_ref().unwrap().wet,
        page.reverb.as_ref().unwrap().wet * 0.5
    );
}

#[test]
fn length_stretches_hold_and_decay_toward_the_source_duration() {
    let src = snapshot_source(); // 0,5 s
    for cfg in FLAVORS.iter() {
        let page = plan(cfg, &src, &mut Random::from_seed(42));
        let long = plan_with(
            cfg,
            &src,
            &mut Random::from_seed(42),
            &Shaping::new(0.0, 1.0),
        );
        let expected = src
            .duration()
            .max(page.timings.total)
            .min(MAX_STRETCHED_TOTAL);
        check!(long.timings.total, expected);
        check!(long.timings.attack, page.timings.attack);
        check!(long.timings.rev_tail, page.timings.rev_tail);
        assert!(long.timings.length >= page.timings.length);
        assert_eq!(long.sources.len(), page.sources.len());
        let s = render_pad_with(cfg, &src, 42, &Shaping::new(0.0, 1.0));
        assert_eq!(s.data.len(), long.timings.length, "{}", cfg.name);
        let half = plan_with(
            cfg,
            &src,
            &mut Random::from_seed(42),
            &Shaping::new(0.0, 0.5),
        );
        check!(half.timings.total, (page.timings.total + expected) / 2.0);
    }
}

#[test]
fn shaping_is_clamped_to_the_unit_range() {
    let s = Shaping::new(3.0, -1.0);
    assert_eq!(s, Shaping::new(1.0, 0.0));
    assert!(Shaping::default().is_default());
    assert!(!Shaping::new(0.1, 0.0).is_default());
}
