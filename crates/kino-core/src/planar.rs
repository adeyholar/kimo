//! Planar (homography) tracker + perspective-correct corner-pin compositing.
//! Tracker: Gauss-Newton on the 4 corner positions (8 params) with a zero-mean/unit-variance photometric model (so exposure changes and
//! flicker do not matter), Tukey-robust per-pixel weights (occluders are rejected), 3-level pyramid, sub-pixel by construction.
//! Drift control: every frame is aligned to the previous frame first, then re-aligned to the ANCHOR template from frame 0; the anchor result wins
//! whenever its confidence is good, so error does not accumulate. Occlusion: when both confidences collapse the track coasts on damped velocity and
//! tries to re-acquire the anchor with a coarse translation search.
use crate::model::*;
use crate::effects::Frame;

pub type Quad = [[f64; 2]; 4];
type Img = (Vec<f32>, usize, usize);

/// Homography solving for dst = H(src), 4 point pairs. Returns h[0..8] (h8 = 1).
pub fn solve_h(s: &Quad, d: &Quad) -> Option<[f64; 8]> {
    let mut a = [[0f64; 9]; 8];
    for i in 0..4 { let (x, y) = (s[i][0], s[i][1]); let (u, v) = (d[i][0], d[i][1]);
        a[i * 2] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u]; a[i * 2 + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v]; }
    for c in 0..8 { let mut m = c; for r in c + 1..8 { if a[r][c].abs() > a[m][c].abs() { m = r; } } a.swap(c, m); let dv = a[c][c]; if dv.abs() < 1e-12 { return None; }
        for k in c..9 { a[c][k] /= dv; } for r in 0..8 { if r != c { let f = a[r][c]; for k in c..9 { a[r][k] -= f * a[c][k]; } } } }
    let mut h = [0f64; 8]; for i in 0..8 { h[i] = a[i][8]; } Some(h)
}
#[inline] fn ap(h: &[f64; 8], x: f64, y: f64) -> (f64, f64) { let w = h[6] * x + h[7] * y + 1.0; ((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w) }
const UNIT: Quad = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

fn down(s: &Img) -> Img { let (w, h) = (s.1, s.2); let (nw, nh) = ((w / 2).max(1), (h / 2).max(1)); let mut o = vec![0f32; nw * nh];
    for y in 0..nh { for x in 0..nw { let (x0, y0) = ((x * 2).min(w - 1), (y * 2).min(h - 1)); let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1)); o[y * nw + x] = 0.25 * (s.0[y0 * w + x0] + s.0[y0 * w + x1] + s.0[y1 * w + x0] + s.0[y1 * w + x1]); } } (o, nw, nh) }
fn pyramid(g: Vec<f32>, w: usize, h: usize, n: usize) -> Vec<Img> { let mut v = vec![(g, w, h)]; for _ in 1..n { let d = down(v.last().unwrap()); v.push(d); } v }
#[inline] fn bil(im: &Img, x: f64, y: f64) -> Option<f32> { let (w, h) = (im.1, im.2); if x < 0.0 || y < 0.0 || x > w as f64 - 1.001 || y > h as f64 - 1.001 { return None; }
    let (x0, y0) = (x as usize, y as usize); let (fx, fy) = ((x - x0 as f64) as f32, (y - y0 as f64) as f32); let d = &im.0;
    let (a, b, c, e) = (d[y0 * w + x0], d[y0 * w + x0 + 1], d[(y0 + 1) * w + x0], d[(y0 + 1) * w + x0 + 1]); Some(a + (b - a) * fx + (c - a) * fy + (a - b - c + e) * fx * fy) }

pub struct Tmpl { n: Vec<(usize, usize)>, v: Vec<Vec<f32>>, us: Vec<Vec<(f64, f64)>> }
fn quad_scale(q: &Quad, s: f64) -> Quad { let mut o = *q; for p in o.iter_mut() { p[0] *= s; p[1] *= s; } o }
fn make_tmpl(pyr: &[Img], q: &Quad) -> Tmpl { let mut t = Tmpl { n: vec![], v: vec![], us: vec![] };
    for (l, im) in pyr.iter().enumerate() { let s = 1.0 / (1u32 << l) as f64; let ql = quad_scale(q, s); let h = solve_h(&UNIT, &ql).unwrap_or([1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
        let wd = ((ql[1][0] - ql[0][0]).hypot(ql[1][1] - ql[0][1])).max((ql[2][0] - ql[3][0]).hypot(ql[2][1] - ql[3][1])); let ht = ((ql[3][0] - ql[0][0]).hypot(ql[3][1] - ql[0][1])).max((ql[2][0] - ql[1][0]).hypot(ql[2][1] - ql[1][1]));
        let (nx, ny) = (((wd / 1.5) as usize).clamp(10, 110), ((ht / 1.5) as usize).clamp(10, 110)); let mut vals = vec![]; let mut us = vec![];
        for j in 0..ny { for i in 0..nx { let (u, v) = ((i as f64 + 0.5) / nx as f64, (j as f64 + 0.5) / ny as f64); let (x, y) = ap(&h, u, v); vals.push(bil(im, x, y).unwrap_or(f32::NAN)); us.push((u, v)); } }
        t.n.push((nx, ny)); t.v.push(vals); t.us.push(us); } t }

pub struct Fit { pub q: Quad, pub conf: f64, pub gain: f64 }
fn tukey(r: f32, c: f32) -> f32 { let a = r.abs() / c; if a >= 1.0 { 0.0 } else { let t = 1.0 - a * a; t * t } }

/// Align template level data to image pyramid, coarse to fine, starting from quad `q0` (full-res pixel coords).
pub fn align(pyr: &[Img], t: &Tmpl, q0: &Quad, iters: &[usize]) -> Fit {
    let nl = pyr.len().min(t.n.len()); let mut q = *q0; let mut conf = 0.0; let mut gain = 1.0;
    for l in (0..nl).rev() { let s = 1.0 / (1u32 << l) as f64; let mut ql = quad_scale(&q, s); let im = &pyr[l]; let tv = &t.v[l]; let us = &t.us[l];
        let valid: Vec<usize> = (0..tv.len()).filter(|&k| tv[k].is_finite()).collect(); if valid.len() < 30 { continue; }
        let mt = valid.iter().map(|&k| tv[k]).sum::<f32>() / valid.len() as f32; let sdt = (valid.iter().map(|&k| (tv[k] - mt).powi(2)).sum::<f32>() / valid.len() as f32).sqrt().max(1e-4);
        let mut lam = 1e-3f64;
        for it in 0..iters[l.min(iters.len() - 1)] {
            let Some(h0) = solve_h(&UNIT, &ql) else { break };
            let mut hp: Vec<[f64; 8]> = vec![]; for j in 0..8 { let mut qq = ql; qq[j / 2][j % 2] += 0.5; hp.push(solve_h(&UNIT, &qq).unwrap_or(h0)); }
            let mut ii = vec![0f32; valid.len()]; let mut jac = vec![[0f32; 8]; valid.len()]; let mut ok = vec![false; valid.len()];
            for (n, &k) in valid.iter().enumerate() { let (u, v) = us[k]; let (x, y) = ap(&h0, u, v);
                let (Some(i0), Some(ixp), Some(ixm), Some(iyp), Some(iym)) = (bil(im, x, y), bil(im, x + 1.0, y), bil(im, x - 1.0, y), bil(im, x, y + 1.0), bil(im, x, y - 1.0)) else { continue };
                let (gx, gy) = ((ixp - ixm) * 0.5, (iyp - iym) * 0.5); ii[n] = i0; ok[n] = true;
                for j in 0..8 { let (xj, yj) = ap(&hp[j], u, v); jac[n][j] = (gx * ((xj - x) * 2.0) as f32 + gy * ((yj - y) * 2.0) as f32) as f32; } }
            let idx: Vec<usize> = (0..valid.len()).filter(|&n| ok[n]).collect(); if idx.len() < 30 { break; }
            let m = idx.iter().map(|&n| ii[n]).sum::<f32>() / idx.len() as f32; let sd = (idx.iter().map(|&n| (ii[n] - m).powi(2)).sum::<f32>() / idx.len() as f32).sqrt().max(1e-4);
            let res: Vec<f32> = idx.iter().map(|&n| (ii[n] - m) / sd - (tv[valid[n]] - mt) / sdt).collect();
            let mut ar: Vec<f32> = res.iter().map(|r| r.abs()).collect(); ar.sort_by(|a, b| a.partial_cmp(b).unwrap()); let mad = ar[ar.len() / 2].max(0.02); let cst = (mad * 1.4826 * 4.685).max(0.15);
            let ws: Vec<f32> = res.iter().map(|&r| tukey(r, cst)).collect(); let wsum: f32 = ws.iter().sum::<f32>().max(1e-6);
            conf = (1.0 - 0.5 * res.iter().zip(&ws).map(|(r, w)| w * r * r).sum::<f32>() / wsum) as f64; let inl = ws.iter().filter(|&&w| w > 0.5).count() as f64 / valid.len() as f64; conf = conf.max(0.0) * inl.min(1.0).sqrt();
            gain = (m / mt.max(1e-3)) as f64;
            let mut a = [[0f64; 8]; 8]; let mut b = [0f64; 8];
            for (e, &n) in idx.iter().enumerate() { let w = ws[e] as f64; if w <= 0.0 { continue; } let j = &jac[n].map(|x| (x / sd) as f64); for p in 0..8 { b[p] -= w * j[p] * res[e] as f64; for qn in 0..8 { a[p][qn] += w * j[p] * j[qn]; } } }
            for p in 0..8 { a[p][p] += lam * (a[p][p] + 1e-3); }
            // solve 8x8
            let mut mtx = [[0f64; 9]; 8]; for p in 0..8 { for qn in 0..8 { mtx[p][qn] = a[p][qn]; } mtx[p][8] = b[p]; }
            let mut sing = false; for c in 0..8 { let mut mx = c; for r in c + 1..8 { if mtx[r][c].abs() > mtx[mx][c].abs() { mx = r; } } mtx.swap(c, mx); let d = mtx[c][c]; if d.abs() < 1e-12 { sing = true; break; } for k in c..9 { mtx[c][k] /= d; } for r in 0..8 { if r != c { let f = mtx[r][c]; for k in c..9 { mtx[r][k] -= f * mtx[c][k]; } } } }
            if sing { break; }
            let mut mx = 0f64; for p in 0..8 { let d = mtx[p][8].clamp(-3.0, 3.0); ql[p / 2][p % 2] += d; mx = mx.max(d.abs()); }
            if mx < 0.005 && it > 0 { break; } lam *= 0.7;
        }
        q = quad_scale(&ql, (1u32 << l) as f64);
    }
    Fit { q, conf, gain }
}

/// ZNCC of template (finest level) at quad, for the coarse re-acquisition search.
fn ncc_at(im: &Img, t: &Tmpl, l: usize, ql: &Quad) -> f64 { let Some(h) = solve_h(&UNIT, ql) else { return -1.0 }; let (tv, us) = (&t.v[l], &t.us[l]); let (mut a, mut b) = (vec![], vec![]);
    for k in (0..tv.len()).step_by(2) { if !tv[k].is_finite() { continue; } let (x, y) = ap(&h, us[k].0, us[k].1); if let Some(v) = bil(im, x, y) { a.push(v as f64); b.push(tv[k] as f64); } }
    if a.len() < 30 { return -1.0; } let n = a.len() as f64; let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n); let (mut sab, mut saa, mut sbb) = (0.0, 1e-9, 1e-9);
    for i in 0..a.len() { sab += (a[i] - ma) * (b[i] - mb); saa += (a[i] - ma).powi(2); sbb += (b[i] - mb).powi(2); } sab / (saa * sbb).sqrt() * (a.len() as f64 / (tv.len() / 2) as f64).min(1.0) }

pub struct Track { pub quads: Vec<Quad>, pub conf: Vec<f64>, pub gain: Vec<f64>, pub lost: Vec<bool> }

/// Track the quad `q0` (pixel coords) through frames `[f0, f1]` of a media clip.
pub fn track(p: &Project, clip_id: &str, q0: Quad, f0: usize, f1: usize, levels: usize) -> Result<Track, String> {
    let c = p.find_clip(clip_id).ok_or("no such clip")?.clone(); let mut cache = crate::render::Cache::new(); let (w, h) = (p.width as usize, p.height as usize);
    let levels = levels.clamp(2, 5); let iters = [6usize, 8, 10, 12, 12];
    let get = |cache: &mut crate::render::Cache, i: usize| -> Result<Vec<Img>, String> { let g = crate::tracking::clip_gray_pub(p, cache, &c, i)?; Ok(pyramid(g, w, h, levels)) };
    let p0 = get(&mut cache, f0)?; let anchor = make_tmpl(&p0, &q0);
    let mut out = Track { quads: vec![q0], conf: vec![1.0], gain: vec![1.0], lost: vec![false] };
    let mut prev_pyr = p0; let (mut q, mut vel) = (q0, [[0f64; 2]; 4]); let mut lost = false;
    for i in f0 + 1..=f1 {
        let cur = get(&mut cache, i)?; let mut pred = q; for k in 0..4 { pred[k][0] += vel[k][0]; pred[k][1] += vel[k][1]; }
        let recent = make_tmpl(&prev_pyr, &q);
        let mut best = Fit { q: pred, conf: 0.0, gain: 1.0 };
        if !lost { let fr = align(&cur, &recent, &pred, &iters); best = fr; }
        // anchor refinement (drift-free); from lost state do a coarse search first
        let start = if lost || best.conf < 0.45 { let l = (levels - 1).min(2); let s = 1.0 / (1u32 << l) as f64; let r = (60.0 * s) as i32; let (mut bs, mut bd) = (-2.0, (0.0, 0.0));
            let base = quad_scale(&pred, s); for dy in (-r..=r).step_by(2.max((r / 8) as usize)) { for dx in (-r..=r).step_by(2.max((r / 8) as usize)) { let mut qq = base; for k in 0..4 { qq[k][0] += dx as f64; qq[k][1] += dy as f64; }
                let sc = ncc_at(&cur[l], &anchor, l, &qq); if sc > bs { bs = sc; bd = (dx as f64, dy as f64); } } }
            let mut qq = pred; for k in 0..4 { qq[k][0] += bd.0 / s; qq[k][1] += bd.1 / s; } qq } else { best.q };
        let fa = align(&cur, &anchor, &start, &iters);
        let (chosen, ok) = if fa.conf >= 0.55 { (fa, true) } else if !lost && best.conf >= 0.5 { (best, true) } else { (Fit { q: pred, conf: fa.conf.max(best.conf), gain: 1.0 }, false) };
        if ok { let nv: Quad = { let mut v = [[0f64; 2]; 4]; for k in 0..4 { v[k] = [chosen.q[k][0] - q[k][0], chosen.q[k][1] - q[k][1]]; } v }; for k in 0..4 { vel[k] = [0.6 * nv[k][0] + 0.4 * vel[k][0], 0.6 * nv[k][1] + 0.4 * vel[k][1]]; } q = chosen.q; lost = false; }
        else { for k in 0..4 { vel[k] = [vel[k][0] * 0.85, vel[k][1] * 0.85]; } q = pred; lost = true; }
        out.quads.push(q); out.conf.push(chosen.conf); out.gain.push(if ok { chosen.gain } else { *out.gain.last().unwrap() }); out.lost.push(lost); prev_pyr = cur;
    }
    Ok(out)
}

/// Destination quad of a pinned clip at clip-local time (absolute pixels). None when the clip is not pinned.
pub fn pin_quad(c: &Clip, lt: f64) -> Option<Quad> { if c.props.get("pin_on").map(|p| p.at(lt)).unwrap_or(0.0) < 0.5 { return None; } let mut q = [[0f64; 2]; 4];
    for i in 0..4 { q[i] = [c.props.get(&format!("pin{i}x"))?.at(lt), c.props.get(&format!("pin{i}y"))?.at(lt)]; } Some(q) }

/// Perspective-correct pin of the whole layer onto `to`, 3x3 supersampled: anti-aliased edges, minification-safe, premultiplied interpolation.
pub fn pin_frame(src: &Frame, to: &Quad, gain: f32) -> Frame {
    let (w, h) = (src.w, src.h); let from: Quad = [[0.0, 0.0], [w as f64, 0.0], [w as f64, h as f64], [0.0, h as f64]]; let mut o = Frame::new(w, h);
    let Some(hm) = solve_h(to, &from) else { return o };
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN); for p in to { x0 = x0.min(p[0]); y0 = y0.min(p[1]); x1 = x1.max(p[0]); y1 = y1.max(p[1]); }
    let (bx0, by0, bx1, by1) = ((x0.floor().max(0.0)) as usize, (y0.floor().max(0.0)) as usize, (x1.ceil().min(w as f64 - 1.0)) as usize, (y1.ceil().min(h as f64 - 1.0)) as usize); if x1 < 0.0 || y1 < 0.0 { return o; }
    // minification: scale of dest relative to source -> more samples when shrinking
    let area = |q: &Quad| 0.5 * ((q[0][0] * q[1][1] - q[1][0] * q[0][1]) + (q[1][0] * q[2][1] - q[2][0] * q[1][1]) + (q[2][0] * q[3][1] - q[3][0] * q[2][1]) + (q[3][0] * q[0][1] - q[0][0] * q[3][1])).abs();
    let ratio = (area(&from) / area(to).max(1.0)).sqrt(); let ss_edge = (ratio.ceil() as usize + 1).clamp(3, 6); let ss_in = if ratio > 1.3 { (ratio.ceil() as usize).clamp(2, 5) } else { 1 };
    for y in by0..=by1.min(h - 1) { for x in bx0..=bx1.min(w - 1) { let mut acc = [0f32; 4];
        let (cu, cv) = ap(&hm, x as f64 + 0.5, y as f64 + 0.5); let mg = 2.5 * ratio.max(1.0) + 1.0; let interior = cu > mg && cv > mg && cu < w as f64 - mg && cv < h as f64 - mg; let ss = if interior { ss_in } else { ss_edge }; let inv = 1.0 / (ss * ss) as f32;
        for sy in 0..ss { for sx in 0..ss { let (xf, yf) = (x as f64 + (sx as f64 + 0.5) / ss as f64, y as f64 + (sy as f64 + 0.5) / ss as f64); let (u, v) = ap(&hm, xf, yf);
            if u < 0.0 || v < 0.0 || u > w as f64 || v > h as f64 { continue; } let (uu, vv) = ((u - 0.5).clamp(0.0, w as f64 - 1.001), (v - 0.5).clamp(0.0, h as f64 - 1.001)); let (ix, iy) = (uu as usize, vv as usize); let (fx, fy) = ((uu - ix as f64) as f32, (vv - iy as f64) as f32);
            let g = |xx: usize, yy: usize| -> [f32; 4] { let i = (yy * w + xx) * 4; let a = src.d[i + 3]; [src.d[i] * a, src.d[i + 1] * a, src.d[i + 2] * a, a] };
            let (a, b, c, d) = (g(ix, iy), g((ix + 1).min(w - 1), iy), g(ix, (iy + 1).min(h - 1)), g((ix + 1).min(w - 1), (iy + 1).min(h - 1)));
            for j in 0..4 { acc[j] += a[j] * (1.0 - fx) * (1.0 - fy) + b[j] * fx * (1.0 - fy) + c[j] * (1.0 - fx) * fy + d[j] * fx * fy; } } }
        let a = acc[3] * inv; let i = (y * w + x) * 4; if a > 1e-6 { for j in 0..3 { o.d[i + j] = (acc[j] * inv / a * gain).min(4.0); } o.d[i + 3] = a; } } }
    o
}

/// Render a pinned layer with optional motion blur (shutter in degrees) by averaging pinned samples across the shutter interval.
pub fn pin_render(c: &Clip, f: &Frame, lt: f64, fps: f64) -> Option<Frame> {
    pin_quad(c, lt)?; let light = c.props.get("pin_light").map(|p| p.at(lt)).unwrap_or(0.0) as f32; let n = if c.motion_blur > 0.0 { ((c.motion_blur / 22.5).ceil() as usize).clamp(2, 16) } else { 1 }; let sh = c.motion_blur / 360.0 / fps;
    let gain_at = |t: f64| -> f32 { 1.0 + (c.props.get("pin_gain").map(|p| p.at(t)).unwrap_or(1.0) as f32 - 1.0) * light };
    if n == 1 { return Some(pin_frame(f, &pin_quad(c, lt)?, gain_at(lt))); }
    let mut acc = Frame::new(f.w, f.h); for k in 0..n { let t2 = lt + ((k as f64 + 0.5) / n as f64 - 0.5) * sh; let Some(q) = pin_quad(c, t2) else { continue }; let g = pin_frame(f, &q, gain_at(t2));
        for i in 0..f.w * f.h { let a = g.d[i * 4 + 3]; if a > 0.0 { for j in 0..3 { acc.d[i * 4 + j] += g.d[i * 4 + j] * a; } acc.d[i * 4 + 3] += a; } } }
    for i in 0..f.w * f.h { let a = acc.d[i * 4 + 3]; if a > 1e-6 { for j in 0..3 { acc.d[i * 4 + j] /= a; } acc.d[i * 4 + 3] = a / n as f32; } } Some(acc)
}

// ---- occluder auto-matte -------------------------------------------------------------------------------------------------------------
/// Session-held anchor for a pinned layer: the source frame the track started on, used at render time to see what covers the surface.
pub struct Anchor { pub src: String, pub src_start: f64, pub frame: Vec<u8>, pub q0: Quad, pub w: usize, pub h: usize, pub soft: f32 }
static ANCHORS: std::sync::Mutex<Option<std::collections::HashMap<String, Anchor>>> = std::sync::Mutex::new(None);
pub fn set_anchor(target: &str, a: Anchor) { let mut g = ANCHORS.lock().unwrap(); g.get_or_insert_with(Default::default).insert(target.to_string(), a); }
pub fn anchor_src(target: &str) -> Option<(String, f64)> { ANCHORS.lock().unwrap().as_ref()?.get(target).map(|a| (a.src.clone(), a.src_start)) }

/// Coverage matte (1 = surface visible, 0 = occluder) for the tracked quad `q` in the current source frame `cur` (rgba u8, same size as the anchor).
pub fn occluder_matte(target: &str, cur: &[u8], q: &Quad, gain: f32) -> Option<Vec<f32>> {
    let g = ANCHORS.lock().unwrap(); let a = g.as_ref()?.get(target)?; let (w, h) = (a.w, a.h); if cur.len() < w * h * 4 { return None; }
    let hm = solve_h(q, &a.q0)?; let mut diff = vec![0f32; w * h]; let mut inq = vec![false; w * h];
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN); for p in q { x0 = x0.min(p[0]); y0 = y0.min(p[1]); x1 = x1.max(p[0]); y1 = y1.max(p[1]); }
    let (bx0, by0, bx1, by1) = (x0.max(0.0) as usize, y0.max(0.0) as usize, (x1.min(w as f64 - 1.0)).max(0.0) as usize, (y1.min(h as f64 - 1.0)).max(0.0) as usize);
    for y in by0..=by1 { for x in bx0..=bx1 { let (u, v) = ap(&hm, x as f64 + 0.5, y as f64 + 0.5); if u < 1.0 || v < 1.0 || u > w as f64 - 2.0 || v > h as f64 - 2.0 { continue; }
        let (ix, iy) = (u as usize, v as usize); let (fx, fy) = ((u - ix as f64) as f32, (v - iy as f64) as f32); let mut m = 0f32;
        for j in 0..3 { let s = |xx: usize, yy: usize| a.frame[(yy * w + xx) * 4 + j] as f32; let av = s(ix, iy) * (1.0 - fx) * (1.0 - fy) + s(ix + 1, iy) * fx * (1.0 - fy) + s(ix, iy + 1) * (1.0 - fx) * fy + s(ix + 1, iy + 1) * fx * fy;
            m = m.max((cur[(y * w + x) * 4 + j] as f32 - av * gain).abs() / 255.0); } diff[y * w + x] = m; inq[y * w + x] = true; } }
    let mut t1 = vec![0f32; w * h]; let mut t2 = vec![0f32; w * h]; crate::masks::box_pass(&diff, &mut t1, w, h, 3, true); crate::masks::box_pass(&t1, &mut t2, w, h, 3, false);
    let (lo, hi) = (0.07 / a.soft.max(0.2), 0.16 / a.soft.max(0.2)); let mut m: Vec<f32> = t2.iter().map(|&d| { let x = ((d - lo) / (hi - lo)).clamp(0.0, 1.0); 1.0 - x * x * (3.0 - 2.0 * x) }).collect();
    // grow the occluder by 2px and feather
    let mut mn = m.clone(); for y in 2..h.saturating_sub(2) { for x in 2..w.saturating_sub(2) { if !inq[y * w + x] { continue; } let mut v = 1f32; for dy in -2i32..=2 { for dx in -2i32..=2 { v = v.min(m[((y as i32 + dy) as usize) * w + (x as i32 + dx) as usize]); } } mn[y * w + x] = v; } }
    crate::masks::box_pass(&mn, &mut t1, w, h, 1, true); crate::masks::box_pass(&t1, &mut m, w, h, 1, false); for i in 0..w * h { if !inq[i] { m[i] = 1.0; } } Some(m)
}
