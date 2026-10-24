// Fabrique de fixtures : WAV en mémoire et ZIP « store », comme la page web les produit.
#![allow(dead_code)]
use std::io::{Cursor, Write};

pub fn sine_wav(freq: f32, frames: usize, sample_rate: u32, channels: u16, bits: u16) -> Vec<u8> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: bits,
        sample_format: if bits == 32 {
            hound::SampleFormat::Float
        } else {
            hound::SampleFormat::Int
        },
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut w = hound::WavWriter::new(&mut cursor, spec).unwrap();
        for i in 0..frames {
            let v = 0.5 * (i as f32 * freq * std::f32::consts::TAU / sample_rate as f32).sin();
            for ch in 0..channels {
                let v = if ch == 1 { -v } else { v };
                match bits {
                    16 => w.write_sample((v * i16::MAX as f32) as i16).unwrap(),
                    24 => w.write_sample((v * 8_388_607.0) as i32).unwrap(),
                    32 => w.write_sample(v).unwrap(),
                    _ => panic!("bits"),
                }
            }
        }
        w.finalize().unwrap();
    }
    cursor.into_inner()
}

pub fn zip_store(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            z.start_file(*name, opts).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }
    cursor.into_inner()
}

/// Le pack exporté par la page web v1.1.0 : 20 × makeitdrum_NN.wav, mono 16 bits, sans manifeste.
pub fn web_pack(sample_rate: u32) -> Vec<u8> {
    let wavs: Vec<(String, Vec<u8>)> = (0..20)
        .map(|i| {
            (
                format!("makeitdrum_{:02}.wav", i + 1),
                sine_wav(100.0 * (i + 1) as f32, 441, sample_rate, 1, 16),
            )
        })
        .collect();
    let refs: Vec<(&str, &[u8])> = wavs
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    zip_store(&refs)
}

/// Un AIFF PCM 16 bits big-endian (le format de la bibliothèque d'Ableton Live).
pub fn sine_aiff(freq: f32, frames: usize, sample_rate: u32, channels: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity(frames * channels as usize * 2);
    for i in 0..frames {
        let v = 0.5 * (i as f32 * freq * std::f32::consts::TAU / sample_rate as f32).sin();
        for ch in 0..channels {
            let v = if ch == 1 { -v } else { v };
            data.extend_from_slice(&((v * i16::MAX as f32) as i16).to_be_bytes());
        }
    }
    // fréquence en flottant étendu 80 bits (exposant biaisé 16383, mantisse normalisée)
    let log2 = 31 - sample_rate.leading_zeros();
    let exponent: u16 = 16383 + log2 as u16;
    let mantissa: u64 = (sample_rate as u64) << (63 - log2);
    let mut comm = Vec::new();
    comm.extend_from_slice(&channels.to_be_bytes());
    comm.extend_from_slice(&(frames as u32).to_be_bytes());
    comm.extend_from_slice(&16u16.to_be_bytes());
    comm.extend_from_slice(&exponent.to_be_bytes());
    comm.extend_from_slice(&mantissa.to_be_bytes());
    let mut ssnd = Vec::new();
    ssnd.extend_from_slice(&0u32.to_be_bytes()); // offset
    ssnd.extend_from_slice(&0u32.to_be_bytes()); // block size
    ssnd.extend_from_slice(&data);
    let mut body = Vec::new();
    body.extend_from_slice(b"AIFF");
    body.extend_from_slice(b"COMM");
    body.extend_from_slice(&(comm.len() as u32).to_be_bytes());
    body.extend_from_slice(&comm);
    body.extend_from_slice(b"SSND");
    body.extend_from_slice(&(ssnd.len() as u32).to_be_bytes());
    body.extend_from_slice(&ssnd);
    let mut out = Vec::new();
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(&body);
    out
}
