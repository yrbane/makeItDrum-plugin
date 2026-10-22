//! Décodage WAV vers un échantillon flottant entrelacé.
use std::io::Cursor;

use crate::kit::KitError;

/// Échantillon audio en mémoire, entrelacé, en flottants dans [-1, 1].
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub data: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl Sample {
    /// Nombre de trames (échantillons par canal).
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.data.len() / self.channels as usize
        }
    }

    /// Trame `i` sous forme (gauche, droite) ; un mono est dupliqué.
    #[inline]
    pub fn frame(&self, i: usize) -> (f32, f32) {
        match self.channels {
            1 => {
                let v = self.data[i];
                (v, v)
            }
            _ => {
                let base = i * self.channels as usize;
                (self.data[base], self.data[base + 1])
            }
        }
    }
}

/// Décode un WAV PCM 8/16/24/32 bits ou flottant 32 bits, 1 ou 2 canaux.
pub fn decode_wav(bytes: &[u8]) -> Result<Sample, KitError> {
    let mut reader =
        hound::WavReader::new(Cursor::new(bytes)).map_err(|e| KitError::Wav(e.to_string()))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.channels > 2 {
        return Err(KitError::Wav(format!(
            "{} canaux non pris en charge",
            spec.channels
        )));
    }
    let data: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<_, _>>()
            .map_err(|e| KitError::Wav(e.to_string()))?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / ((1u64 << (spec.bits_per_sample - 1)) as f32);
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()
                .map_err(|e| KitError::Wav(e.to_string()))?
        }
    };
    Ok(Sample {
        data,
        channels: spec.channels,
        sample_rate: spec.sample_rate,
    })
}
