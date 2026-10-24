//! Cœur pur du lecteur de kits make it drum : aucune dépendance à un hôte
//! audio, tout est testable en `cargo test`.
pub mod analysis;
pub mod curves;
pub mod dsp;
pub mod engine;
pub mod flavors;
pub mod kit;
pub mod prng;
pub mod render;
pub mod voice;
pub mod wav;
