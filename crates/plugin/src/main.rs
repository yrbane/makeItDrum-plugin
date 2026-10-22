//! Binaire autonome : `make-it-drum --backend dummy` ouvre l'interface sans DAW.
fn main() {
    nih_plug::nih_export_standalone::<make_it_drum::KitPlayer>();
}
