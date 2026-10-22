//! Cœur pur du lecteur de kits make it drum : aucune dépendance à un hôte
//! audio, tout est testable en `cargo test`.
pub mod engine;
pub mod kit;
pub mod voice;
pub mod wav;
