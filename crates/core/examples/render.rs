//! Outil d'écoute : fabrique les 20 percussions d'un son en WAV, hors DAW.
//! `cargo run --release -p make-it-drum-core --example render -- son.wav dossier/ [fidélité] [durée] [départ] [graine]`
//! Les trois cadrages vont de 0 à 1 (défaut 0 : la page telle quelle) ; la
//! graine fixe la première graine, les suivantes s'en déduisent (défaut 42).
use std::path::Path;

use make_it_drum_core::kit::Kit;
use make_it_drum_core::render::Shaping;
use make_it_drum_core::wav::{decode_audio, encode_wav};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage : render <son.wav> <dossier> [fidélité] [durée] [départ] [graine]");
        std::process::exit(2);
    }
    let num = |i: usize, d: f64| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(d);
    let shaping = Shaping::new(num(2, 0.0), num(3, 0.0)).with_start(num(4, 0.0));
    let first_seed = num(5, 42.0) as u32;
    let seeds: Vec<u32> = (0..20).map(|i| first_seed.wrapping_add(i * 7919)).collect();

    let source = Path::new(&args[0]);
    let bytes = std::fs::read(source).unwrap_or_else(|e| {
        eprintln!("lecture de {} : {e}", source.display());
        std::process::exit(1);
    });
    let ext = source.extension().map(|e| e.to_string_lossy().into_owned());
    let sample = decode_audio(&bytes, ext.as_deref()).unwrap_or_else(|e| {
        eprintln!("décodage : {e}");
        std::process::exit(1);
    });
    let name = source
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "kit".into());
    let started = std::time::Instant::now();
    let kit =
        Kit::generate_with(&name, &sample, Some(&seeds), shaping, |_, _| {}).unwrap_or_else(|e| {
            eprintln!("génération : {e}");
            std::process::exit(1);
        });
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out).expect("dossier de sortie");
    for pad in &kit.pads {
        let file = out.join(format!("{:02} {}.wav", pad.index + 1, pad.label));
        std::fs::write(&file, encode_wav(&pad.sample).unwrap()).expect("écriture");
        println!(
            "{:>2} {:<8} {:.3} s  graine {}",
            pad.index + 1,
            pad.label,
            pad.sample.frames() as f64 / pad.sample.sample_rate as f64,
            pad.seed.unwrap_or(0)
        );
    }
    std::fs::write(
        out.join(format!("{name}.kit.zip")),
        kit.to_kit_zip().unwrap(),
    )
    .expect("kit.zip");
    println!(
        "20 pads + {name}.kit.zip dans {} en {:.2} s (fidélité {:.2}, durée {:.2}, départ {:.2})",
        out.display(),
        started.elapsed().as_secs_f64(),
        shaping.fidelity,
        shaping.length,
        shaping.start
    );
}
