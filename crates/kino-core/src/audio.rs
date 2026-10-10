//! Audio: per-clip decode through ffmpeg filters, keyframed volume, mix to 48k stereo f32, beat detection.
use crate::model::*;
use std::io::Read;
use std::process::{Command, Stdio};

pub const SR: usize = 48000;

pub fn filter_chain(c: &Clip) -> String {
    let a = &c.audio; let mut f: Vec<String> = vec![];
    // speed (atempo supports 0.5..2 per stage)
    let mut sp = c.speed; while sp > 2.0 { f.push("atempo=2.0".into()); sp /= 2.0; } while sp < 0.5 { f.push("atempo=0.5".into()); sp /= 0.5; } if (sp - 1.0).abs() > 1e-6 { f.push(format!("atempo={sp}")); }
    if a.pitch.abs() > 1e-6 { let r = 2f64.powf(a.pitch / 12.0); f.push(format!("asetrate={}", (SR as f64 * r) as i64)); f.push(format!("aresample={SR}")); f.push(format!("atempo={}", 1.0 / r)); }
    if a.highpass > 0.0 { f.push(format!("highpass=f={}", a.highpass)); }
    if a.lowpass > 0.0 { f.push(format!("lowpass=f={}", a.lowpass)); }
    if a.denoise > 0.0 { f.push(format!("afftdn=nr={}", a.denoise)); }
    if a.compress { f.push("acompressor=threshold=-18dB:ratio=3:attack=10:release=200".into()); }
    if a.echo > 0.0 { f.push(format!("aecho=0.8:0.8:{}:0.4", (a.echo * 1000.0) as i64)); }
    if a.reverse { f.push("areverse".into()); }
    f.push(format!("aresample={SR}")); f.push("aformat=sample_fmts=flt:channel_layouts=stereo".into());
    f.join(",")
}

pub fn decode_clip(path: &str, c: &Clip) -> Vec<f32> {
    let mut ch = match Command::new("ffmpeg").args(["-v","error","-ss",&format!("{}", c.in_point),"-t",&format!("{}", c.duration * c.speed),"-i",path,"-vn","-af",&filter_chain(c),"-f","f32le","-ac","2","-ar",&SR.to_string(),"-"])
        .stdout(Stdio::piped()).stderr(Stdio::null()).spawn() { Ok(c) => c, Err(_) => return vec![] };
    let mut b = Vec::new(); ch.stdout.take().unwrap().read_to_end(&mut b).ok(); let _ = ch.wait();
    b.chunks_exact(4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect()
}

/// Mix every media clip's audio (and audio-only assets) into one interleaved stereo buffer.
pub fn mix(p: &Project) -> Vec<f32> {
    let total = ((p.duration() * SR as f64).ceil() as usize) * 2; let mut out = vec![0f32; total];
    for t in &p.tracks { for c in &t.clips {
        let Source::Media { asset } = &c.source else { continue }; if c.audio.mute { continue; }
        let Some(a) = p.assets.get(asset) else { continue };
        let s = decode_clip(&a.path, c); let off = ((c.start * SR as f64) as usize) * 2; let n = ((c.duration * SR as f64) as usize * 2).min(s.len());
        for i in (0..n).step_by(2) {
            let lt = (i / 2) as f64 / SR as f64;
            let mut g = c.audio.volume.at(lt) as f32;
            if c.audio.fade_in > 0.0 && lt < c.audio.fade_in { g *= (lt / c.audio.fade_in) as f32; }
            let rem = c.duration - lt; if c.audio.fade_out > 0.0 && rem < c.audio.fade_out { g *= (rem / c.audio.fade_out) as f32; }
            if off + i + 1 < out.len() { out[off + i] += s[i] * g; out[off + i + 1] += s[i + 1] * g; }
        }
    }}
    for v in out.iter_mut() { *v = v.clamp(-1.0, 1.0); }
    out
}

pub fn write_wav(path: &str, pcm: &[f32]) -> Result<(), String> {
    let mut b = Vec::with_capacity(44 + pcm.len() * 2); let data = (pcm.len() * 2) as u32;
    b.extend(b"RIFF"); b.extend((36 + data).to_le_bytes()); b.extend(b"WAVEfmt "); b.extend(16u32.to_le_bytes()); b.extend(1u16.to_le_bytes()); b.extend(2u16.to_le_bytes());
    b.extend((SR as u32).to_le_bytes()); b.extend((SR as u32 * 4).to_le_bytes()); b.extend(4u16.to_le_bytes()); b.extend(16u16.to_le_bytes()); b.extend(b"data"); b.extend(data.to_le_bytes());
    for v in pcm { b.extend(((v * 32767.0) as i16).to_le_bytes()); }
    std::fs::write(path, b).map_err(|e| e.to_string())
}

/// Spectral-flux style onset detector on a mono envelope; returns beat times in seconds.
pub fn detect_beats(path: &str, min_gap: f64) -> Vec<f64> {
    let Ok(o) = Command::new("ffmpeg").args(["-v","error","-i",path,"-vn","-f","f32le","-ac","1","-ar","22050","-"]).output() else { return vec![] };
    let s: Vec<f32> = o.stdout.chunks_exact(4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).collect();
    let hop = 512; let win = 1024; let mut env = vec![];
    let mut i = 0; while i + win < s.len() { env.push((s[i..i + win].iter().map(|v| v * v).sum::<f32>() / win as f32).sqrt()); i += hop; }
    let flux: Vec<f32> = env.windows(2).map(|w| (w[1] - w[0]).max(0.0)).collect();
    let mut beats = vec![]; let mut last = -10.0;
    for i in 8..flux.len().saturating_sub(8) {
        let local = flux[i - 8..i + 8].iter().sum::<f32>() / 16.0; let t = i as f64 * hop as f64 / 22050.0;
        if flux[i] > local * 1.8 + 0.01 && flux[i] >= flux[i - 1] && flux[i] >= flux[i + 1] && t - last >= min_gap { beats.push(t); last = t; }
    }
    beats
}
