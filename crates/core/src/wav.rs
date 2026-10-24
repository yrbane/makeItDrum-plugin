//! Décodage audio vers un échantillon flottant entrelacé : WAV par hound,
//! AIFF / MP3 / FLAC / OGG par symphonia (les formats qu'une DAW glisse
//! depuis son navigateur de fichiers).
use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

use crate::kit::KitError;

/// Extensions acceptées pour un échantillon seul.
pub const AUDIO_EXTENSIONS: [&str; 10] = [
    "wav", "aif", "aiff", "aifc", "mp3", "flac", "ogg", "m4a", "mp4", "aac",
];

/// `true` si le nom de fichier porte une extension audio prise en charge.
pub fn is_audio_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    AUDIO_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{ext}")))
}

/// Échantillon audio en mémoire, entrelacé, en flottants dans [-1, 1].
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    pub data: Vec<f32>,
    pub channels: u16,
    pub sample_rate: u32,
}

impl Sample {
    /// Un échantillon sans aucune trame (pad vide).
    pub fn silent() -> Self {
        Self {
            data: Vec::new(),
            channels: 1,
            sample_rate: 44100,
        }
    }

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

/// Décode un fichier audio quel que soit son format : WAV d'abord (hound),
/// sinon symphonia, guidé par l'extension (`aif`, `mp3`, `flac`, `ogg`…).
/// Un fichier à plus de deux canaux est réduit à ses deux premiers.
pub fn decode_audio(bytes: &[u8], extension: Option<&str>) -> Result<Sample, KitError> {
    if let Ok(sample) = decode_wav(bytes) {
        return Ok(sample);
    }
    decode_with_symphonia(bytes, extension)
}

fn decode_with_symphonia(bytes: &[u8], extension: Option<&str>) -> Result<Sample, KitError> {
    let err = |e: SymphoniaError| KitError::Wav(e.to_string());
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = extension {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &Default::default(), &Default::default())
        .map_err(|e| KitError::Wav(format!("format audio non reconnu ({e})")))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| KitError::Wav("aucune piste audio".into()))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &Default::default())
        .map_err(err)?;

    let mut data = Vec::new();
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut buffer: Option<SampleBuffer<f32>> = None;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(err(e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(err(e)),
        };
        let spec = *decoded.spec();
        let frames = decoded.capacity();
        let src_channels = spec.channels.count();
        if buffer
            .as_ref()
            .map(|b| b.capacity() < frames * src_channels)
            .unwrap_or(true)
        {
            channels = src_channels.min(2) as u16;
            sample_rate = spec.rate;
            buffer = Some(SampleBuffer::new(frames as u64, spec));
        }
        let buffer = buffer.as_mut().expect("buffer alloué ci-dessus");
        buffer.copy_interleaved_ref(decoded);
        let samples = buffer.samples();
        if src_channels <= 2 {
            data.extend_from_slice(samples);
        } else {
            for frame in samples.chunks(src_channels) {
                data.extend_from_slice(&frame[..2]);
            }
        }
    }
    if data.is_empty() || channels == 0 || sample_rate == 0 {
        return Err(KitError::Wav("aucun échantillon décodé".into()));
    }
    Ok(Sample {
        data,
        channels,
        sample_rate,
    })
}

/// Encode un échantillon en WAV float 32 bits.
pub fn encode_wav(sample: &Sample) -> Result<Vec<u8>, KitError> {
    let spec = hound::WavSpec {
        channels: sample.channels,
        sample_rate: sample.sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let wav_err = |e: hound::Error| KitError::Wav(e.to_string());
    let mut wav = std::io::Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut wav, spec).map_err(wav_err)?;
        for v in &sample.data {
            w.write_sample(*v).map_err(wav_err)?;
        }
        w.finalize().map_err(wav_err)?;
    }
    Ok(wav.into_inner())
}
