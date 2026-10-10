//! Mask / selection / matte stack. A clip owns an ordered stack of MaskLayers; each layer is evaluated to a coverage buffer
//! (0..1 per pixel), post-processed (expand/choke, feather, invert, opacity) and combined with the stack below it
//! (add, subtract, intersect, xor, lighten, darken, difference). Layer kinds cover shapes (rect, ellipse, bezier path, regular
//! polygon / star, gradients) and selections (luma, color, HSL qualifier, difference matte, magic wand, scribble select, edges,
//! own alpha, procedural noise). All numeric params are keyframable `Prop`s; paths animate with per-point keyframes.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::effects::Frame;
use crate::keyframe::Prop;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct PathPt { pub x: f64, pub y: f64, #[serde(default)] pub ix: f64, #[serde(default)] pub iy: f64, #[serde(default)] pub ox: f64, #[serde(default)] pub oy: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathKey { pub t: f64, pub points: Vec<PathPt>, #[serde(default)] pub hold: bool }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MaskLayer {
    pub id: String,
    #[serde(default)] pub name: String,
    pub kind: String,
    #[serde(default = "d_mode")] pub mode: String,
    #[serde(default)] pub invert: bool,
    #[serde(default = "d_true")] pub enabled: bool,
    #[serde(default)] pub props: BTreeMap<String, Prop>,
    #[serde(default)] pub points: Vec<PathPt>,
    #[serde(default = "d_true")] pub closed: bool,
    #[serde(default)] pub path_keys: Vec<PathKey>,
    #[serde(default)] pub fg: Vec<[f64; 2]>,
    #[serde(default)] pub bg: Vec<[f64; 2]>,
    #[serde(default)] pub strings: BTreeMap<String, String>,
}
fn d_mode() -> String { "add".into() } fn d_true() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Matte { pub source: String, #[serde(default = "d_mm")] pub mode: String, #[serde(default)] pub hide_source: bool }
fn d_mm() -> String { "alpha".into() }

pub const KINDS: &[&str] = &["rect", "ellipse", "path", "linear", "radial", "luma", "color", "hsl", "difference", "wand", "scribble", "edge", "alpha", "noise"];
pub const MODES: &[&str] = &["add", "subtract", "intersect", "xor", "lighten", "darken", "difference", "none"];

pub fn defaults(kind: &str) -> Vec<(&'static str, f64)> {
    match kind {
        "rect" => vec![("cx", 0.5), ("cy", 0.5), ("w", 0.5), ("h", 0.5), ("rot", 0.0), ("round", 0.0)],
        "ellipse" => vec![("cx", 0.5), ("cy", 0.5), ("w", 0.5), ("h", 0.5), ("rot", 0.0)],
        "linear" => vec![("angle", 0.0), ("pos", 0.5), ("soft", 0.1)],
        "radial" => vec![("cx", 0.5), ("cy", 0.5), ("r", 0.3), ("soft", 0.2)],
        "luma" => vec![("lo", 0.5), ("hi", 1.0), ("soft", 0.1)],
        "color" => vec![("r", 0.0), ("g", 1.0), ("b", 0.0), ("tol", 0.2), ("soft", 0.1)],
        "hsl" => vec![("hue", 0.0), ("hue_width", 30.0), ("s_lo", 0.2), ("s_hi", 1.0), ("l_lo", 0.0), ("l_hi", 1.0), ("soft", 0.1)],
        "difference" => vec![("tol", 0.08), ("soft", 0.06)],
        "wand" => vec![("x", 0.5), ("y", 0.5), ("tol", 0.12), ("soft", 0.05), ("contiguous", 1.0)],
        "scribble" => vec![("soft", 0.15), ("smooth", 2.0)],
        "edge" => vec![("thr", 0.15), ("soft", 0.1), ("thickness", 2.0)],
        "alpha" => vec![("thr", 0.5), ("soft", 0.1)],
        "noise" => vec![("scale", 120.0), ("thr", 0.5), ("soft", 0.08), ("seed", 1.0), ("speed", 0.0)],
        _ => vec![],
    }
}

pub fn new_layer(id: String, kind: &str) -> MaskLayer {
    let mut props = BTreeMap::new();
    for (k, v) in defaults(kind) { props.insert(k.to_string(), Prop::new(v)); }
    for (k, v) in [("feather", 0.0), ("expand", 0.0), ("opacity", 1.0)] { props.insert(k.to_string(), Prop::new(v)); }
    MaskLayer { id, name: String::new(), kind: kind.to_string(), mode: "add".into(), invert: false, enabled: true, props, points: vec![], closed: true, path_keys: vec![], fg: vec![], bg: vec![], strings: BTreeMap::new() }
}

/// Regular polygon (sides>=3) or star (inner>0: inner radius ratio) as path points, normalized canvas coords. aspect = w/h of the canvas.
pub fn ngon(cx: f64, cy: f64, r: f64, sides: usize, inner: f64, rot_deg: f64, aspect: f64) -> Vec<PathPt> {
    let n = sides.max(3); let mut v = vec![]; let steps = if inner > 0.0 { n * 2 } else { n };
    for i in 0..steps { let rr = if inner > 0.0 && i % 2 == 1 { r * inner } else { r };
        let a = (rot_deg - 90.0).to_radians() + i as f64 / steps as f64 * std::f64::consts::TAU; v.push(PathPt { x: cx + rr * a.cos() / aspect, y: cy + rr * a.sin(), ..Default::default() }); }
    v
}

fn g(l: &MaskLayer, k: &str, d: f64, t: f64) -> f32 { l.props.get(k).map(|p| p.at(t)).unwrap_or(d) as f32 }
fn sstep(a: f32, b: f32, x: f32) -> f32 { if b <= a { return if x >= a { 1.0 } else { 0.0 }; } let t = ((x - a) / (b - a)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

/// Path at time t: interpolate between path keys (per-point lerp incl. tangents), else the static points.
pub fn path_at(l: &MaskLayer, t: f64) -> Vec<PathPt> {
    let ks = &l.path_keys; if ks.is_empty() { return l.points.clone(); }
    if t <= ks[0].t { return ks[0].points.clone(); } if t >= ks[ks.len() - 1].t { return ks[ks.len() - 1].points.clone(); }
    let i = ks.iter().position(|k| k.t > t).unwrap_or(ks.len() - 1) - 1; let (a, b) = (&ks[i], &ks[i + 1]);
    if a.hold || a.points.len() != b.points.len() { return a.points.clone(); }
    let u = (t - a.t) / (b.t - a.t).max(1e-9); let u = u * u * (3.0 - 2.0 * u);
    a.points.iter().zip(&b.points).map(|(p, q)| PathPt { x: p.x + (q.x - p.x) * u, y: p.y + (q.y - p.y) * u, ix: p.ix + (q.ix - p.ix) * u, iy: p.iy + (q.iy - p.iy) * u, ox: p.ox + (q.ox - p.ox) * u, oy: p.oy + (q.oy - p.oy) * u }).collect()
}

fn raster_path(pts: &[PathPt], closed: bool, w: usize, h: usize) -> Vec<f32> {
    let mut cov = vec![0f32; w * h]; let n = pts.len(); if n < 3 { return cov; }
    let (wf, hf) = (w as f64, h as f64); let mut poly: Vec<(f64, f64)> = vec![];
    let segs = if closed { n } else { n - 1 };
    for i in 0..segs { let a = &pts[i]; let b = &pts[(i + 1) % n];
        let (p0, p1, p2, p3) = ((a.x * wf, a.y * hf), ((a.x + a.ox) * wf, (a.y + a.oy) * hf), ((b.x + b.ix) * wf, (b.y + b.iy) * hf), (b.x * wf, b.y * hf));
        let straight = a.ox == 0.0 && a.oy == 0.0 && b.ix == 0.0 && b.iy == 0.0; let steps = if straight { 1 } else { 24 };
        for s in 0..steps { let u = s as f64 / steps as f64; let v = 1.0 - u;
            poly.push((v*v*v*p0.0 + 3.0*v*v*u*p1.0 + 3.0*v*u*u*p2.0 + u*u*u*p3.0, v*v*v*p0.1 + 3.0*v*v*u*p1.1 + 3.0*v*u*u*p2.1 + u*u*u*p3.1)); } }
    let m = poly.len(); if m < 3 { return cov; }
    let ymin = poly.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor().max(0.0) as usize; let ymax = (poly.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i64).clamp(0, h as i64) as usize;
    const SS: usize = 4; let mut xs: Vec<(f64, i32)> = vec![];
    for y in ymin..ymax { for sy in 0..SS { let yy = y as f64 + (sy as f64 + 0.5) / SS as f64; xs.clear();
        for i in 0..m { let (a, b) = (poly[i], poly[(i + 1) % m]); if (a.1 <= yy && b.1 > yy) || (b.1 <= yy && a.1 > yy) { let t = (yy - a.1) / (b.1 - a.1); xs.push((a.0 + t * (b.0 - a.0), if b.1 > a.1 { 1 } else { -1 })); } }
        xs.sort_by(|p, q| p.0.partial_cmp(&q.0).unwrap()); let mut wind = 0; 
        for k in 0..xs.len().saturating_sub(1) { wind += xs[k].1; if wind != 0 { let (x0, x1) = (xs[k].0.max(0.0), xs[k + 1].0.min(wf)); if x1 <= x0 { continue; }
            let (i0, i1) = (x0.floor() as usize, (x1.ceil() as usize).min(w)); for x in i0..i1 { let cx0 = (x as f64).max(x0); let cx1 = ((x + 1) as f64).min(x1); if cx1 > cx0 { cov[y * w + x] += ((cx1 - cx0) / SS as f64) as f32; } } } }
    } }
    for v in cov.iter_mut() { *v = v.min(1.0); } cov
}

fn box_pass(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize, horiz: bool) {
    let (len, lines) = if horiz { (w, h) } else { (h, w) }; let n = (2 * r + 1) as f32;
    for l in 0..lines { let idx = |i: usize| if horiz { l * w + i } else { i * w + l };
        let mut acc = 0f32; for i in 0..=r.min(len - 1) { acc += src[idx(i)]; } acc += src[idx(0)] * r as f32; // clamp-extend left
        for i in 0..len { dst[idx(i)] = acc / n; let add = src[idx((i + r + 1).min(len - 1))]; let sub = src[idx(i.saturating_sub(r))]; acc += add - sub; } }
}
pub fn blur_cov(c: &mut Vec<f32>, w: usize, h: usize, sigma: f32) {
    if sigma < 0.4 { return; } let r = ((sigma * 1.7).round() as usize).max(1).min(w.min(h) / 2 - 1); let mut t = vec![0f32; w * h];
    for _ in 0..3 { box_pass(c, &mut t, w, h, r, true); box_pass(&t, c, w, h, r, false); }
}
fn morph(c: &mut Vec<f32>, w: usize, h: usize, r: i32) {
    if r == 0 { return; } let grow = r > 0; let r = r.unsigned_abs() as usize; let mut t = vec![0f32; w * h];
    for (horiz, src_is_c) in [(true, true), (false, false)] {
        let (len, lines) = if horiz { (w, h) } else { (h, w) };
        for l in 0..lines { let idx = |i: usize| if horiz { l * w + i } else { i * w + l };
            for i in 0..len { let (lo, hi) = (i.saturating_sub(r), (i + r).min(len - 1)); let mut m = if grow { 0f32 } else { 1f32 };
                for k in lo..=hi { let v = if src_is_c { c[idx(k)] } else { t[idx(k)] }; m = if grow { m.max(v) } else { m.min(v) }; }
                if src_is_c { t[idx(i)] = m } else { c[idx(i)] = m } } } }
}

pub struct Ctx<'a> { pub refs: &'a dyn Fn(&str) -> Option<Frame> }

fn lumf(f: &Frame, i: usize) -> f32 { 0.2126 * f.d[i * 4] + 0.7152 * f.d[i * 4 + 1] + 0.0722 * f.d[i * 4 + 2] }
fn rgb2hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) { let mx = r.max(g).max(b); let mn = r.min(g).min(b); let l = (mx + mn) / 2.0; let d = mx - mn;
    if d < 1e-6 { return (0.0, 0.0, l); } let s = d / (1.0 - (2.0 * l - 1.0).abs()).max(1e-6);
    let hh = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 }; (hh * 60.0, s, l) }
fn vhash(x: i64, y: i64, s: i64) -> f32 { let mut n = (x.wrapping_mul(374761393) ^ y.wrapping_mul(668265263) ^ s.wrapping_mul(2147483647)) as u64; n = (n ^ (n >> 13)).wrapping_mul(1274126177); ((n ^ (n >> 16)) & 0xffff) as f32 / 65535.0 }
fn vnoise(x: f32, y: f32, s: i64) -> f32 { let (xi, yi) = (x.floor(), y.floor()); let (fx, fy) = (x - xi, y - yi); let (u, v) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy)); let (xi, yi) = (xi as i64, yi as i64);
    let a = vhash(xi, yi, s); let b = vhash(xi + 1, yi, s); let c = vhash(xi, yi + 1, s); let d = vhash(xi + 1, yi + 1, s); a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v }

/// Raw coverage of one layer (before expand/feather/invert/opacity).
fn raw(l: &MaskLayer, f: &Frame, t: f64, ctx: &Ctx) -> Vec<f32> {
    let (w, h) = (f.w, f.h); let (wf, hf) = (w as f32, h as f32); let mut c = vec![0f32; w * h];
    match l.kind.as_str() {
        "rect" | "ellipse" => { let (cx, cy, mw, mh, rot) = (g(l, "cx", 0.5, t) * wf, g(l, "cy", 0.5, t) * hf, g(l, "w", 0.5, t) * wf / 2.0, g(l, "h", 0.5, t) * hf / 2.0, g(l, "rot", 0.0, t).to_radians());
            let rr = g(l, "round", 0.0, t).clamp(0.0, 1.0) * mw.min(mh); let (s, co) = (rot.sin(), rot.cos()); let ell = l.kind == "ellipse";
            for y in 0..h { for x in 0..w { let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy); let (ux, uy) = (px * co + py * s, -px * s + py * co);
                let d = if ell { let k = ((ux / mw.max(0.5)).powi(2) + (uy / mh.max(0.5)).powi(2)).sqrt(); (k - 1.0) * mw.min(mh) } else { let (qx, qy) = (ux.abs() - (mw - rr), uy.abs() - (mh - rr)); (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rr };
                c[y * w + x] = (0.5 - d).clamp(0.0, 1.0); } } }
        "path" => { c = raster_path(&path_at(l, t), l.closed, w, h); }
        "linear" => { let (a, pos, soft) = (g(l, "angle", 0.0, t).to_radians(), g(l, "pos", 0.5, t), g(l, "soft", 0.1, t).max(1e-4)); let (dx, dy) = (a.cos(), a.sin()); let ext = (wf * dx.abs() + hf * dy.abs()).max(1.0);
            for y in 0..h { for x in 0..w { let u = (((x as f32 + 0.5 - wf / 2.0) * dx + (y as f32 + 0.5 - hf / 2.0) * dy) / ext) + 0.5; c[y * w + x] = 1.0 - sstep(pos - soft / 2.0, pos + soft / 2.0, u); } } }
        "radial" => { let (cx, cy, r, soft) = (g(l, "cx", 0.5, t) * wf, g(l, "cy", 0.5, t) * hf, g(l, "r", 0.3, t) * hf, g(l, "soft", 0.2, t).max(1e-4) * hf);
            for y in 0..h { for x in 0..w { let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt(); c[y * w + x] = 1.0 - sstep(r - soft / 2.0, r + soft / 2.0, d); } } }
        "luma" => { let (lo, hi, so) = (g(l, "lo", 0.5, t), g(l, "hi", 1.0, t), g(l, "soft", 0.1, t) / 2.0 + 1e-4);
            for i in 0..w * h { let v = lumf(f, i); c[i] = sstep(lo - so, lo + so, v) * (1.0 - sstep(hi - so, hi + so, v)); } }
        "color" => { let (kr, kg, kb, tol, so) = (g(l, "r", 0.0, t), g(l, "g", 1.0, t), g(l, "b", 0.0, t), g(l, "tol", 0.2, t), g(l, "soft", 0.1, t));
            for i in 0..w * h { let d = ((f.d[i*4] - kr).powi(2) + (f.d[i*4+1] - kg).powi(2) + (f.d[i*4+2] - kb).powi(2)).sqrt() / 1.732; c[i] = 1.0 - sstep(tol, tol + so + 1e-4, d); } }
        "hsl" => { let (hu, hw, s0, s1, l0, l1, so) = (g(l, "hue", 0.0, t), g(l, "hue_width", 30.0, t), g(l, "s_lo", 0.2, t), g(l, "s_hi", 1.0, t), g(l, "l_lo", 0.0, t), g(l, "l_hi", 1.0, t), g(l, "soft", 0.1, t) + 1e-4);
            for i in 0..w * h { let (hh, ss, ll) = rgb2hsl(f.d[i*4], f.d[i*4+1], f.d[i*4+2]); let mut dh = (hh - hu).abs() % 360.0; if dh > 180.0 { dh = 360.0 - dh; }
                c[i] = (1.0 - sstep(hw / 2.0, hw / 2.0 + so * 90.0, dh)) * sstep(s0 - so, s0, ss) * (1.0 - sstep(s1, s1 + so, ss)) * sstep(l0 - so, l0, ll) * (1.0 - sstep(l1, l1 + so, ll)); } }
        "difference" => { if let Some(rf) = l.strings.get("ref_clip").and_then(|id| (ctx.refs)(id)) { let (tol, so) = (g(l, "tol", 0.08, t), g(l, "soft", 0.06, t) + 1e-4);
            for i in 0..w * h { let d = ((f.d[i*4] - rf.d[i*4]).abs() + (f.d[i*4+1] - rf.d[i*4+1]).abs() + (f.d[i*4+2] - rf.d[i*4+2]).abs()) / 3.0; c[i] = sstep(tol, tol + so, d); } } }
        "wand" => { let (sx, sy) = (((g(l, "x", 0.5, t) * wf) as usize).min(w - 1), ((g(l, "y", 0.5, t) * hf) as usize).min(h - 1)); let (tol, so, contig) = (g(l, "tol", 0.12, t), g(l, "soft", 0.05, t), g(l, "contiguous", 1.0, t) > 0.5);
            let mut sc = [0f32; 3]; let mut n = 0.0; for dy in -1i32..=1 { for dx in -1i32..=1 { let (x, y) = ((sx as i32 + dx).clamp(0, w as i32 - 1) as usize, (sy as i32 + dy).clamp(0, h as i32 - 1) as usize); for j in 0..3 { sc[j] += f.d[(y * w + x) * 4 + j]; } n += 1.0; } } for j in 0..3 { sc[j] /= n; }
            let dist = |i: usize| ((f.d[i*4] - sc[0]).powi(2) + (f.d[i*4+1] - sc[1]).powi(2) + (f.d[i*4+2] - sc[2]).powi(2)).sqrt() / 1.732; let val = |i: usize| 1.0 - sstep(tol, tol + so + 1e-4, dist(i));
            if !contig { for i in 0..w * h { c[i] = val(i); } } else { let mut st = vec![sy * w + sx]; let mut seen = vec![false; w * h]; seen[sy * w + sx] = true;
                while let Some(i) = st.pop() { let v = val(i); if v <= 0.0 { continue; } c[i] = v; let (x, y) = (i % w, i / w);
                    for (nx, ny) in [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)] { if nx < w && ny < h { let j = ny * w + nx; if !seen[j] { seen[j] = true; st.push(j); } } } } } }
        "scribble" => { let samp = |pts: &Vec<[f64; 2]>| -> Vec<[f32; 3]> { let mut v = vec![]; for p in pts { let (cx, cy) = ((p[0] * wf as f64) as i32, (p[1] * hf as f64) as i32); for dy in (-6..=6).step_by(3) { for dx in (-6..=6).step_by(3) { let (x, y) = ((cx + dx).clamp(0, w as i32 - 1) as usize, (cy + dy).clamp(0, h as i32 - 1) as usize); let i = (y * w + x) * 4; v.push([f.d[i], f.d[i+1], f.d[i+2]]); } } } v };
            let (fg, bg) = (samp(&l.fg), samp(&l.bg)); if !fg.is_empty() && !bg.is_empty() { let (fgp, bgp): (Vec<(f32, f32)>, Vec<(f32, f32)>) = (l.fg.iter().map(|p| (p[0] as f32 * wf, p[1] as f32 * hf)).collect(), l.bg.iter().map(|p| (p[0] as f32 * wf, p[1] as f32 * hf)).collect());
                let so = g(l, "soft", 0.15, t).max(0.02);
                for y in 0..h { for x in 0..w { let i = (y * w + x) * 4; let px = [f.d[i], f.d[i+1], f.d[i+2]];
                    let dm = |s: &Vec<[f32; 3]>| s.iter().map(|q| (px[0]-q[0]).powi(2) + (px[1]-q[1]).powi(2) + (px[2]-q[2]).powi(2)).fold(f32::MAX, f32::min).sqrt();
                    let sp = |s: &Vec<(f32, f32)>| s.iter().map(|q| ((x as f32 - q.0).powi(2) + (y as f32 - q.1).powi(2)).sqrt()).fold(f32::MAX, f32::min);
                    let (df, db) = (dm(&fg) + sp(&fgp) / hf * 0.15, dm(&bg) + sp(&bgp) / hf * 0.15); c[y * w + x] = sstep(0.5 - so, 0.5 + so, db / (df + db + 1e-6)); } }
                let sm = g(l, "smooth", 2.0, t); blur_cov(&mut c, w, h, sm); for v in c.iter_mut() { *v = sstep(0.35, 0.65, *v); } } }
        "edge" => { let (thr, so, th) = (g(l, "thr", 0.15, t), g(l, "soft", 0.1, t) + 1e-4, g(l, "thickness", 2.0, t));
            for y in 1..h.saturating_sub(1) { for x in 1..w.saturating_sub(1) { let p = |xx: usize, yy: usize| lumf(f, yy * w + xx); let gx = -p(x-1,y-1) - 2.0*p(x-1,y) - p(x-1,y+1) + p(x+1,y-1) + 2.0*p(x+1,y) + p(x+1,y+1); let gy = -p(x-1,y-1) - 2.0*p(x,y-1) - p(x+1,y-1) + p(x-1,y+1) + 2.0*p(x,y+1) + p(x+1,y+1);
                c[y * w + x] = sstep(thr, thr + so, (gx * gx + gy * gy).sqrt() / 4.0); } } morph(&mut c, w, h, (th * hf / 720.0 / 2.0).round() as i32); }
        "alpha" => { let (thr, so) = (g(l, "thr", 0.5, t), g(l, "soft", 0.1, t) / 2.0 + 1e-4); for i in 0..w * h { c[i] = sstep(thr - so, thr + so, f.d[i * 4 + 3]); } }
        "noise" => { let (sc, thr, so, seed, sp) = (g(l, "scale", 120.0, t).max(2.0) * hf / 720.0, g(l, "thr", 0.5, t), g(l, "soft", 0.08, t) + 1e-4, g(l, "seed", 1.0, t) as i64, g(l, "speed", 0.0, t));
            let tt = t as f32 * sp; for y in 0..h { for x in 0..w { let (u, v) = ((x as f32 + tt * sc) / sc, (y as f32 - tt * sc * 0.6) / sc); let n = 0.55 * vnoise(u, v, seed) + 0.3 * vnoise(u * 2.1, v * 2.1, seed + 7) + 0.15 * vnoise(u * 4.3, v * 4.3, seed + 13); c[y * w + x] = sstep(thr - so, thr + so, n); } } }
        _ => {}
    }
    c
}

pub fn layer_coverage(l: &MaskLayer, f: &Frame, t: f64, ctx: &Ctx) -> Vec<f32> {
    let (w, h) = (f.w, f.h); let s = h as f32 / 720.0; let mut c = raw(l, f, t, ctx);
    let ex = (g(l, "expand", 0.0, t) * s).round() as i32; if ex != 0 { morph(&mut c, w, h, ex); }
    let fe = g(l, "feather", 0.0, t) * s; if fe > 0.0 { blur_cov(&mut c, w, h, fe / 2.0); }
    let op = g(l, "opacity", 1.0, t).clamp(0.0, 1.0);
    for v in c.iter_mut() { if l.invert { *v = 1.0 - *v; } *v *= op; } c
}

pub fn stack_coverage(layers: &[MaskLayer], f: &Frame, t: f64, ctx: &Ctx) -> Option<Vec<f32>> {
    let act: Vec<&MaskLayer> = layers.iter().filter(|l| l.enabled && l.mode != "none").collect(); if act.is_empty() { return None; }
    let n = f.w * f.h; let first = act[0].mode.as_str(); let mut acc = vec![if first == "subtract" || first == "intersect" { 1.0f32 } else { 0.0 }; n];
    for l in act { let c = layer_coverage(l, f, t, ctx);
        for i in 0..n { let (a, b) = (acc[i], c[i]); acc[i] = match l.mode.as_str() { "subtract" => a * (1.0 - b), "intersect" => a * b, "xor" | "difference" => (a - b).abs(), "lighten" => a.max(b), "darken" => a.min(b), _ => a + b - a * b }; } }
    Some(acc)
}

pub fn apply_stack(f: &mut Frame, layers: &[MaskLayer], t: f64, ctx: &Ctx) { if let Some(c) = stack_coverage(layers, f, t, ctx) { for i in 0..f.w * f.h { f.d[i * 4 + 3] *= c[i]; } } }

/// Track matte coverage from a rendered matte source frame.
pub fn matte_coverage(m: &Matte, src: &Frame) -> Vec<f32> {
    (0..src.w * src.h).map(|i| { let a = src.d[i * 4 + 3]; let l = 0.2126 * src.d[i * 4] + 0.7152 * src.d[i * 4 + 1] + 0.0722 * src.d[i * 4 + 2];
        match m.mode.as_str() { "luma" => l * a, "inv_luma" => 1.0 - l * a, "inv_alpha" => 1.0 - a, _ => a } }).collect()
}
