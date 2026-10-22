//! Outil de développement : décrit un kit (zip ou dossier) tel que le plugin le verra.
//! `cargo run -p make-it-drum-core --example inspect -- chemin/vers/kit.zip`
use std::path::Path;

use make_it_drum_core::kit::Kit;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage : inspect <kit.zip | dossier>");
    match Kit::from_path(Path::new(&path)) {
        Ok(kit) => {
            println!("kit « {} » — {} pads", kit.name, kit.pads.len());
            for p in &kit.pads {
                let s = &p.sample;
                println!(
                    "  {:>2}  note {:>3}  {:<10} {:>5} Hz  {} canal(aux)  {:.3} s",
                    p.index + 1,
                    p.note,
                    p.label,
                    s.sample_rate,
                    s.channels,
                    s.frames() as f32 / s.sample_rate as f32
                );
            }
        }
        Err(e) => {
            eprintln!("erreur : {e}");
            std::process::exit(1);
        }
    }
}
