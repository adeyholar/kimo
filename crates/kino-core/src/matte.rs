//! Video matte tracker: propagates a reviewed seed outline through a clip into a dense, temporally stable alpha matte.
//! Per frame: dense optical flow warps the previous alpha, a band around the warped edge is re-decided from colour models
//! (anchored to the seed frame) and snapped to image edges with a guided filter, so partial alpha on blurred edges survives.
//! Seeds in the middle of a clip are tracked both ways and blended by distance. A final flow-compensated forward/backward
//! pass removes frame-to-frame flicker without hard thresholds. Output is a raw u8 sequence readable by the `matte` mask kind.
use crate::effects::Frame;
use crate::flow::flow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct Store { pub w: usize, pub h: usize, pub n: usize, pub d: Vec<u8> }
static STORE: Mutex<Option<HashMap<String, Arc<Store>>>> = Mutex::new(None);
pub fn save(dir: &str, s: &Store) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?; let mut b = Vec::with_capacity(16 + s.d.len()); b.extend_from_slice(b"KMT1"); b.extend_from_slice(&(s.w as u32).to_le_bytes()); b.extend_from_slice(&(s.h as u32).to_le_bytes()); b.extend_from_slice(&(s.n as u32).to_le_bytes()); b.extend_from_slice(&s.d);
    std::fs::write(format!("{dir}/matte.bin"), b).map_err(|e| e.to_string())?; STORE.lock().unwrap().get_or_insert_with(Default::default).insert(dir.to_string(), Arc::new(Store { w: s.w, h: s.h, n: s.n, d: s.d.clone() })); Ok(())
}
pub fn load(dir: &str) -> Option<Arc<Store>> {
    if let Some(s) = STORE.lock().unwrap().as_ref().and_then(|m| m.get(dir)) { return Some(s.clone()); }
    let b = std::fs::read(format!("{dir}/matte.bin")).ok()?; if b.len() < 16 || &b[0..4] != b"KMT1" { return None; }
    let rd = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize; let (w, h, n) = (rd(4), rd(8), rd(12)); if b.len() != 16 + w * h * n { return None; }
    let s = Arc::new(Store { w, h, n, d: b[16..].to_vec() }); STORE.lock().unwrap().get_or_insert_with(Default::default).insert(dir.to_string(), s.clone()); Some(s)
}
/// Alpha of frame `i` (clamped) resampled to w x h.
pub fn frame_alpha(s: &Store, i: f64, w: usize, h: usize) -> Vec<f32> {
    let a = i.floor().clamp(0.0, (s.n - 1) as f64) as usize; let b = (a + 1).min(s.n - 1); let t = (i - a as f64).clamp(0.0, 1.0) as f32; let fa = &s.d[a * s.w * s.h..(a + 1) * s.w * s.h]; let fb = &s.d[b * s.w * s.h..(b + 1) * s.w * s.h];
    let mut o = vec![0f32; w * h]; for y in 0..h { let sy = ((y * s.h) / h).min(s.h - 1); for x in 0..w { let sx = ((x * s.w) / w).min(s.w - 1); let k = sy * s.w + sx; o[y * w + x] = (fa[k] as f32 * (1.0 - t) + fb[k] as f32 * t) / 255.0; } } o
}

/// Anti-aliased polygon coverage (4 vertical sub-samples, exact horizontal coverage), even-odd.
pub fn raster_poly(pts: &[[f64; 2]], w: usize, h: usize) -> Vec<f32> {
    let mut c = vec![0f32; w * h]; if pts.len() < 3 { return c; } const SS: usize = 4;
    for y in 0..h { for s in 0..SS { let yy = y as f64 + (s as f64 + 0.5) / SS as f64; let mut xs: Vec<f64> = vec![];
        for i in 0..pts.len() { let (a, b) = (pts[i], pts[(i + 1) % pts.len()]); if (a[1] > yy) != (b[1] > yy) { xs.push(a[0] + (yy - a[1]) / (b[1] - a[1]) * (b[0] - a[0])); } }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pr in xs.chunks(2) { if pr.len() < 2 { continue; } let (x0, x1) = (pr[0].max(0.0), pr[1].min(w as f64)); if x1 <= x0 { continue; } let (i0, i1) = (x0.floor() as usize, (x1.ceil() as usize).min(w));
            for x in i0..i1 { let cov = (x1.min(x as f64 + 1.0) - x0.max(x as f64)).max(0.0); c[y * w + x] += (cov / SS as f64) as f32; } } } }
    for v in c.iter_mut() { *v = v.clamp(0.0, 1.0); } c
}

fn bil(s: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 { let x = x.clamp(0.0, w as f32 - 1.001); let y = y.clamp(0.0, h as f32 - 1.001); let (x0, y0) = (x as usize, y as usize); let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let (a, b, c, d) = (s[y0 * w + x0], s[y0 * w + x0 + 1], s[(y0 + 1) * w + x0], s[(y0 + 1) * w + x0 + 1]); a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy }
/// Flow field at half resolution (u,v in half-res pixels): a pair for one frame step.
pub struct Fl { pub u: Vec<f32>, pub v: Vec<f32>, pub w2: usize, pub h2: usize }
impl Fl { fn at(&self, x: usize, y: usize) -> (f32, f32) { let fx = (x as f32 + 0.5) / 2.0 - 0.5; let fy = (y as f32 + 0.5) / 2.0 - 0.5; (2.0 * bil(&self.u, self.w2, self.h2, fx, fy), 2.0 * bil(&self.v, self.w2, self.h2, fx, fy)) } }
/// Backward warp: out(x) = src(x + flow(x)), where `fl` maps this frame to the frame `src` belongs to.
pub fn warp(src: &[f32], w: usize, h: usize, fl: &Fl) -> Vec<f32> { let mut o = vec![0f32; w * h]; for y in 0..h { for x in 0..w { let (u, v) = fl.at(x, y); o[y * w + x] = bil(src, w, h, x as f32 + u, y as f32 + v); } } o }

fn to_frame(rgb: &[u8], w: usize, h: usize) -> Frame { let mut f = Frame::new(w, h); for i in 0..w * h { for j in 0..3 { f.d[i * 4 + j] = rgb[i * 3 + j] as f32 / 255.0; } f.d[i * 4 + 3] = 1.0; } f }
fn gray(rgb: &[u8], i: usize) -> f32 { (0.299 * rgb[i * 3] as f32 + 0.587 * rgb[i * 3 + 1] as f32 + 0.114 * rgb[i * 3 + 2] as f32) / 255.0 }
fn bin(rgb: &[u8], i: usize) -> usize { ((rgb[i * 3] as usize >> 4) << 8) | ((rgb[i * 3 + 1] as usize >> 4) << 4) | (rgb[i * 3 + 2] as usize >> 4) }
fn hist(rgb: &[u8], sel: &dyn Fn(usize) -> bool, n: usize) -> Vec<f32> { let mut h = vec![0f32; 4096]; let mut tot = 0.0; for i in 0..n { if sel(i) { h[bin(rgb, i)] += 1.0; tot += 1.0; } }
    // smooth over neighbouring bins so near colours share evidence
    let mut o = vec![0f32; 4096]; for b in 0..4096usize { if h[b] == 0.0 { continue; } let (r, g, bl) = (b >> 8, (b >> 4) & 15, b & 15); for dr in -1i32..=1 { for dg in -1i32..=1 { for db in -1i32..=1 { let (rr, gg, bb) = (r as i32 + dr, g as i32 + dg, bl as i32 + db); if !(0..16).contains(&rr) || !(0..16).contains(&gg) || !(0..16).contains(&bb) { continue; } let wgt = if dr == 0 && dg == 0 && db == 0 { 1.0 } else { 0.25 }; o[((rr << 8) | (gg << 4) | bb) as usize] += h[b] * wgt; } } } }
    let s: f32 = o.iter().sum::<f32>().max(1e-6); let _ = tot; for v in o.iter_mut() { *v = (*v + 0.02) / (s + 0.02 * 4096.0); } o }

#[derive(Clone)]
pub struct Opts { pub band: f32, pub temporal: f32, pub colour: f32, pub guide_r: usize }
impl Default for Opts { fn default() -> Self { Opts { band: 14.0, temporal: 0.6, colour: 0.9, guide_r: 3 } } }
pub struct Out { pub alpha: Vec<Vec<f32>>, pub raw_flicker: f64, pub flicker: f64, pub raw_jitter: f64, pub jitter: f64, pub conf: Vec<f64>, pub warnings: Vec<String> }

/// Re-decide the band around a warped alpha from colour evidence anchored on the seed frame.
fn refine(a: &[f32], rgb: &[u8], anchor: &(Vec<f32>, Vec<f32>), w: usize, h: usize, o: &Opts) -> Vec<f32> {
    let n = w * h; let inside: Vec<bool> = a.iter().map(|&v| v > 0.5).collect(); let outside: Vec<bool> = inside.iter().map(|b| !b).collect();
    let (d_out, d_in) = (crate::masks::edt(&inside, w, h), crate::masks::edt(&outside, w, h)); let sd: Vec<f32> = (0..n).map(|i| if inside[i] { d_in[i] - 0.5 } else { -(d_out[i] - 0.5) }).collect();
    let fgc = hist(rgb, &|i| sd[i] > 4.0, n); let bgc = hist(rgb, &|i| sd[i] < -3.0 && sd[i] > -o.band * 2.5, n);
    let mut out = vec![0f32; n]; let s0 = (o.band * 0.35).max(1.0);
    for i in 0..n { if sd[i] > o.band { out[i] = 1.0; continue; } if sd[i] < -o.band { out[i] = 0.0; continue; }
        let b = bin(rgb, i); let pf = 0.5 * fgc[b] + 0.5 * anchor.0[b]; let pb = 0.5 * bgc[b] + 0.5 * anchor.1[b]; let lr = (pf.ln() - pb.ln()).clamp(-4.0, 4.0);
        // keep the warped soft value as a prior too so motion-blurred edges keep their partial alpha
        let prior = (a[i].clamp(0.02, 0.98) / (1.0 - a[i].clamp(0.02, 0.98))).ln() * 0.5; let z = sd[i] / s0 + o.colour * lr + prior; out[i] = 1.0 / (1.0 + (-z).exp()); }
    // snap the soft edge to image structure, then restore the deep interior / exterior exactly
    let f = to_frame(rgb, w, h); let mut g = out.clone(); crate::masks::guided(&mut g, &f, o.guide_r, 2e-3);
    for i in 0..n { if sd[i].abs() <= o.band { out[i] = 0.35 * out[i] + 0.65 * g[i]; } } out
}
fn anchor_models(rgb: &[u8], alpha: &[f32], w: usize, h: usize, band: f32) -> (Vec<f32>, Vec<f32>) {
    let n = w * h; let inside: Vec<bool> = alpha.iter().map(|&v| v > 0.5).collect(); let outside: Vec<bool> = inside.iter().map(|b| !b).collect(); let (d_out, d_in) = (crate::masks::edt(&inside, w, h), crate::masks::edt(&outside, w, h));
    let sd: Vec<f32> = (0..n).map(|i| if inside[i] { d_in[i] - 0.5 } else { -(d_out[i] - 0.5) }).collect(); (hist(rgb, &|i| sd[i] > 4.0, n), hist(rgb, &|i| sd[i] < -3.0 && sd[i] > -band * 3.0, n)) }

fn edge_flicker(seq: &[Vec<f32>], bf: &[Fl], w: usize, h: usize) -> f64 { let (mut tot, mut cnt) = (0f64, 0f64); for t in 1..seq.len() { let wp = warp(&seq[t - 1], w, h, &bf[t]); for i in 0..w * h { if seq[t][i].max(wp[i]) > 0.02 && seq[t][i].min(wp[i]) < 0.98 { tot += (seq[t][i] - wp[i]).abs() as f64; cnt += 1.0; } } } if cnt > 0.0 { tot / cnt } else { 0.0 } }
fn area_jitter(seq: &[Vec<f32>]) -> f64 { let a: Vec<f64> = seq.iter().map(|f| f.iter().map(|&v| v as f64).sum::<f64>()).collect(); if a.len() < 3 { return 0.0; } let m = a.iter().sum::<f64>() / a.len() as f64; if m < 1.0 { return 0.0; } let mut s = 0.0; for t in 1..a.len() - 1 { s += (a[t] - 0.5 * (a[t - 1] + a[t + 1])).powi(2); } (s / (a.len() - 2) as f64).sqrt() / m }

/// `frames`: RGB u8 per frame (all frames of the range); `seeds`: (frame index within `frames`, outline in pixels).
pub fn track(frames: &[Vec<u8>], w: usize, h: usize, seeds: &[(usize, Vec<[f64; 2]>)], o: &Opts) -> Result<Out, String> {
    let nf = frames.len(); if nf < 2 || seeds.is_empty() { return Err("need at least 2 frames and 1 seed".into()); } let mut seeds = seeds.to_vec(); seeds.sort_by_key(|s| s.0); if seeds.iter().any(|s| s.0 >= nf) { return Err("seed outside the frame range".into()); }
    let fr: Vec<Frame> = vec![]; let _ = fr;
    // flows: bf[t] maps frame t -> t-1 (for forward warping of alpha), ff[t] maps t -> t+1
    let mk = |a: usize, b: usize| -> Fl { let (fa, fb) = (to_frame(&frames[a], w, h), to_frame(&frames[b], w, h)); let (u, v, w2, h2) = flow(&fa, &fb, 5); Fl { u, v, w2, h2 } };
    let mut bf: Vec<Fl> = (0..nf).map(|_| Fl { u: vec![], v: vec![], w2: 0, h2: 0 }).collect(); let mut ff: Vec<Fl> = (0..nf).map(|_| Fl { u: vec![], v: vec![], w2: 0, h2: 0 }).collect();
    for t in 1..nf { bf[t] = mk(t, t - 1); } for t in 0..nf - 1 { ff[t] = mk(t, t + 1); }
    let seed_a: Vec<Vec<f32>> = seeds.iter().map(|s| raster_poly(&s.1, w, h)).collect(); let anchors: Vec<(Vec<f32>, Vec<f32>)> = seeds.iter().enumerate().map(|(k, s)| anchor_models(&frames[s.0], &seed_a[k], w, h, o.band)).collect();
    let none: Vec<Option<Vec<f32>>> = vec![None; nf]; let mut fwd: Vec<Vec<Option<Vec<f32>>>> = vec![]; let mut bwd: Vec<Vec<Option<Vec<f32>>>> = vec![];
    for (k, s) in seeds.iter().enumerate() {
        let end_f = if k + 1 < seeds.len() { seeds[k + 1].0 } else { nf - 1 }; let mut chain = none.clone(); chain[s.0] = Some(seed_a[k].clone());
        for t in s.0 + 1..=end_f { let prev = chain[t - 1].as_ref().unwrap(); let wp = warp(prev, w, h, &bf[t]); chain[t] = Some(refine(&wp, &frames[t], &anchors[k], w, h, o)); } fwd.push(chain);
        let end_b = if k > 0 { seeds[k - 1].0 } else { 0 }; let mut chain = none.clone(); chain[s.0] = Some(seed_a[k].clone());
        for t in (end_b..s.0).rev() { let prev = chain[t + 1].as_ref().unwrap(); let wp = warp(prev, w, h, &ff[t]); chain[t] = Some(refine(&wp, &frames[t], &anchors[k], w, h, o)); } bwd.push(chain); }
    // blend: between seeds k and k+1 use forward chain of k and backward chain of k+1 by distance
    let mut blended: Vec<Vec<f32>> = Vec::with_capacity(nf); let mut conf = vec![1.0f64; nf]; let mut warnings = vec![];
    for t in 0..nf { let prev = seeds.iter().rposition(|s| s.0 <= t); let next = seeds.iter().position(|s| s.0 >= t);
        let a = match (prev, next) { (Some(p), Some(nx)) if p == nx => seed_a[p].clone(),
            (Some(p), Some(nx)) => { let (t0, t1) = (seeds[p].0 as f32, seeds[nx].0 as f32); let wgt = (t as f32 - t0) / (t1 - t0); let (fa, ba) = (fwd[p][t].as_ref().unwrap(), bwd[nx][t].as_ref().unwrap());
                let (mut inter, mut uni) = (0f64, 0f64); for i in 0..w * h { inter += fa[i].min(ba[i]) as f64; uni += fa[i].max(ba[i]) as f64; } conf[t] = if uni > 1.0 { inter / uni } else { 1.0 };
                (0..w * h).map(|i| (1.0 - wgt) * fa[i] + wgt * ba[i]).collect() }
            (Some(p), None) => fwd[p][t].clone().unwrap(), (None, Some(nx)) => bwd[nx][t].clone().unwrap(), _ => unreachable!() }; blended.push(a); }
    // decisiveness confidence outside two-seed segments
    for t in 0..nf { if seeds.iter().any(|s| s.0 == t) { continue; } let inb = seeds.iter().any(|s| s.0 < t) && seeds.iter().any(|s| s.0 > t); if !inb { let (mut e, mut c) = (0f64, 0f64); for &v in blended[t].iter() { if v > 0.02 && v < 0.98 { e += (v - 0.5).abs() as f64 * 2.0; c += 1.0; } } conf[t] = if c > 0.0 { e / c } else { 1.0 }; } }
    let low: Vec<usize> = (0..nf).filter(|&t| conf[t] < 0.8).collect(); if !low.is_empty() { warnings.push(format!("low confidence (forward/backward disagreement or undecided edge) on {} frames, first at frame {}", low.len(), low[0])); }
    let raw_flicker = edge_flicker(&blended, &bf, w, h); let raw_jitter = area_jitter(&blended);
    // temporal stabilisation: flow-compensated, photo-consistency-gated, zero-lag (forward then backward)
    let cons = |t: usize, wp_idx: usize, fl: &Fl| -> Vec<f32> { let g1: Vec<f32> = (0..w * h).map(|i| gray(&frames[wp_idx], i)).collect(); let wg = warp(&g1, w, h, fl); (0..w * h).map(|i| { let d = (gray(&frames[t], i) - wg[i]) / 0.05; (-d * d).exp() }).collect() };
    let k = o.temporal.clamp(0.0, 0.95); let mut sf = blended.clone(); let mut sb = blended.clone();
    if k > 0.0 { for t in 1..nf { if seeds.iter().any(|s| s.0 == t) { continue; } let wp = warp(&sf[t - 1], w, h, &bf[t]); let c = cons(t, t - 1, &bf[t]); for i in 0..w * h { let kk = k * c[i]; sf[t][i] = (1.0 - kk) * blended[t][i] + kk * wp[i]; } }
        for t in (0..nf - 1).rev() { if seeds.iter().any(|s| s.0 == t) { continue; } let wp = warp(&sb[t + 1], w, h, &ff[t]); let c = cons(t, t + 1, &ff[t]); for i in 0..w * h { let kk = k * c[i]; sb[t][i] = (1.0 - kk) * blended[t][i] + kk * wp[i]; } } }
    let alpha: Vec<Vec<f32>> = (0..nf).map(|t| if seeds.iter().any(|s| s.0 == t) { blended[t].clone() } else { (0..w * h).map(|i| 0.5 * (sf[t][i] + sb[t][i])).collect() }).collect();
    let (flicker, jitter) = (edge_flicker(&alpha, &bf, w, h), area_jitter(&alpha));
    Ok(Out { alpha, raw_flicker, flicker, raw_jitter, jitter, conf, warnings })
}
