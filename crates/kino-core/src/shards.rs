//! 3D glass shard burst: hundreds of thick, translucent, faceted shards flying in 3D space, seen through the project camera.
//! Per shard: random convex polygon with thickness (two faces + rim), tumbling rotation, drag/gravity motion, depth of field
//! (soft edges and lower alpha away from the focus plane), chromatic rim dispersion, specular glints, motion blur by time sub-sampling.
use crate::effects::Frame;
use crate::three_d::{rot_matrix, CamView, Camera};
use serde::{Deserialize, Serialize};

fn d_count() -> u32 { 450 } fn d_origin() -> [f64; 3] { [0.0, 40.0, 0.0] } fn d_speed() -> f64 { 520.0 } fn d_smin() -> f64 { 5.0 } fn d_smax() -> f64 { 58.0 } fn d_seed() -> u64 { 7 }
fn d_tint() -> [f64; 3] { [0.78, 0.9, 1.0] } fn d_drag() -> f64 { 1.1 } fn d_spin() -> f64 { 260.0 } fn d_dof() -> f64 { 1.0 } fn d_one() -> f64 { 1.0 } fn d_light() -> [f64; 3] { [-0.5, -0.6, -0.62] }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShardBurst {
    #[serde(default = "d_count")] pub count: u32, #[serde(default = "d_origin")] pub origin: [f64; 3], #[serde(default = "d_speed")] pub speed: f64,
    #[serde(default = "d_smin")] pub size_min: f64, #[serde(default = "d_smax")] pub size_max: f64, #[serde(default = "d_seed")] pub seed: u64,
    #[serde(default = "d_tint")] pub tint: [f64; 3], #[serde(default = "d_drag")] pub drag: f64, #[serde(default)] pub gravity: f64, #[serde(default = "d_spin")] pub spin: f64,
    #[serde(default = "d_dof")] pub dof: f64, #[serde(default)] pub focus: f64, #[serde(default = "d_one")] pub brightness: f64, #[serde(default = "d_light")] pub light: [f64; 3],
    #[serde(default)] pub toward_camera: f64, #[serde(default)] pub spread_time: f64, #[serde(default)] pub fade_out: f64, #[serde(default)] pub dispersion: f64, #[serde(default)] pub origin_spread: f64,
    #[serde(default)] pub physics: bool, #[serde(default = "d_rest")] pub restitution: f64, #[serde(default = "d_fric")] pub friction: f64, #[serde(default)] pub floor: Option<f64>, #[serde(default)] pub ang_drag: f64, #[serde(default = "d_tr")] pub collide: bool,
    #[serde(default)] pub fresnel: f64, #[serde(default)] pub refract: f64, #[serde(default)] pub falloff: f64,
}
fn d_rest() -> f64 { 0.35 } fn d_fric() -> f64 { 0.5 } fn d_tr() -> bool { true }

fn h(a: u64, i: u64, k: u64) -> f64 { let mut x = a.wrapping_mul(0x9E3779B97F4A7C15) ^ i.wrapping_mul(0xBF58476D1CE4E5B9) ^ k.wrapping_mul(0x94D049BB133111EB); x ^= x >> 30; x = x.wrapping_mul(0xBF58476D1CE4E5B9); x ^= x >> 27; x = x.wrapping_mul(0x94D049BB133111EB); x ^= x >> 31; (x >> 11) as f64 / (1u64 << 53) as f64 }
fn sstep(x: f64) -> f64 { let t = x.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

struct Prim { depth: f64, pts: [(f64, f64); 6], n: usize, nz: f64, spec: f64, rim_c: f64, blur: f64, alpha: f64, face: usize, u: f64, tx: f64, ty: f64 }

pub fn render(e: &ShardBurst, f: &mut Frame, t: f64, cam: &Camera, fps: f64, total: f64) {
    let (w, hh) = (f.w, f.h); let cv = CamView::new(w, hh, cam); let fl = cv.focal(); let k = hh as f64 / 720.0;
    let nsub = 4usize; let shutter = 0.5 / fps.max(1.0); let mut accc = vec![0f32; w * hh * 3]; let mut acca = vec![0f32; w * hh]; let rf = e.refract > 0.0; let mut dacc = vec![0f32; if rf { w * hh * 2 } else { 0 }]; let mut dsum = vec![0f32; if rf { w * hh * 2 } else { 0 }];
    let lg = { let l = e.light; let m = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt().max(1e-9); [l[0] / m, l[1] / m, l[2] / m] };
    let focus = if e.focus > 0.0 { e.focus } else { fl };
    let traj = if e.physics { Some((bake_cached(e, k, total, hh), 0)) } else { None };
    for sub in 0..nsub { let ts = (t + ((sub as f64 + 0.5) / nsub as f64 - 0.5) * shutter).max(0.0);
        let mut prims: Vec<Prim> = Vec::with_capacity(e.count as usize * 2);
        for i in 0..e.count as u64 {
            let r = |kk: u64| h(e.seed, i, kk); let birth = r(40) * e.spread_time; if ts < birth { continue; } let age = ts - birth;
            let size = (e.size_min * (e.size_max / e.size_min.max(0.1)).powf(r(1).powf(1.0))) * k; // heavy toward small, a few large
            let (th, ph) = (r(2) * std::f64::consts::TAU, (r(3) * 2.0 - 1.0).acos()); let dir = [ph.sin() * th.cos(), ph.sin() * th.sin() * 0.8, ph.cos()];
            let sp = e.speed * k * (0.35 + 1.1 * r(4).powf(1.2)) / (0.6 + size / (30.0 * k)).sqrt(); let dr = e.drag.max(1e-3); let travel = (1.0 - (-dr * age).exp()) / dr;
            let mut vz = dir[2] * sp; vz -= e.toward_camera * sp * (0.3 + r(5)); // negative z = toward camera
            let os = e.origin_spread * k; let pos = [e.origin[0] * k + (r(50) - 0.5) * 2.0 * os + dir[0] * sp * travel, e.origin[1] * k + (r(51) - 0.5) * 2.0 * os * 0.6 + dir[1] * sp * travel + 0.5 * e.gravity * k * age * age, e.origin[2] + (r(52) - 0.5) * 2.0 * os + vz * travel];
            let phys = traj.as_ref().map(|(tj, _): &(std::sync::Arc<crate::physics::Traj>, i32)| tj.at(i as usize, ts));
            let ang = [r(6) * 360.0 + (r(7) - 0.5) * 2.0 * e.spin * age, r(8) * 360.0 + (r(9) - 0.5) * 2.0 * e.spin * age, r(10) * 360.0 + (r(11) - 0.5) * 2.0 * e.spin * age];
            let (pos, rm) = match &phys { Some((p, q)) => (*p, crate::physics::quat_matrix(*q)), None => (pos, rot_matrix(ang)) }; let nv = 4 + (r(12) * 2.99) as usize; let aspect = 0.75 + 0.5 * r(13); let thick = (0.45 + 0.6 * r(14)) * size;
            let life_fade = if e.fade_out > 0.0 { 1.0 - sstep((age - (total - e.fade_out - birth).max(0.1)) / e.fade_out) } else { 1.0 }; let born = sstep(age / 0.05);
            let mut loc = [(0.0, 0.0); 6]; for v in 0..nv { let a = v as f64 / nv as f64 * std::f64::consts::TAU + (h(e.seed, i, 20 + v as u64) - 0.5) * 0.7; let rr = size * (0.74 + 0.3 * h(e.seed, i, 30 + v as u64)); loc[v] = (a.cos() * rr, a.sin() * rr * aspect); }
            let mut shard: Vec<([(f64, f64); 6], f64, f64, f64, f64)> = vec![]; // per face
            let nrm = [rm[0][2], rm[1][2], rm[2][2]]; let mut zsum = 0.0; let mut ok = true;
            for face in 0..2 { let zoff = if face == 0 { thick * 0.5 } else { -thick * 0.5 }; let mut pts = [(0.0, 0.0); 6]; let mut dz = 0.0;
                for v in 0..nv { let l = [loc[v].0, loc[v].1, zoff]; let wp = [pos[0] + rm[0][0] * l[0] + rm[0][1] * l[1] + rm[0][2] * l[2], pos[1] + rm[1][0] * l[0] + rm[1][1] * l[1] + rm[1][2] * l[2], pos[2] + rm[2][0] * l[0] + rm[2][1] * l[1] + rm[2][2] * l[2]];
                    match cv.project(wp) { Some((sx, sy, zv)) => { pts[v] = (sx, sy); dz += zv; } None => { ok = false; } } }
                if !ok { break; } dz /= nv as f64; zsum += dz; shard.push((pts, dz, 0.0, 0.0, 0.0)); }
            if !ok || shard.len() < 2 { continue; } let zv = zsum / 2.0; let coc = e.dof * ((zv - focus).abs() / zv) * fl * 0.035 * k; let ndl = nrm[0] * lg[0] + nrm[1] * lg[1] + nrm[2] * lg[2];
            let refl = [lg[0] - 2.0 * ndl * nrm[0], lg[1] - 2.0 * ndl * nrm[1], lg[2] - 2.0 * ndl * nrm[2]]; let spec = refl[2].max(0.0).powi(18) + 0.35 * (-refl[2]).max(0.0).powi(18);
            let dim = 1.0 / (1.0 + coc * 0.12) * (-e.falloff * ((zv - fl).max(0.0) / fl)).exp(); let fres = 0.04 + 0.96 * (1.0 - nrm[2].abs()).powi(5); let al = life_fade * born * dim;
            { let (fp, bp) = (shard[0].0, shard[1].0); for v in 0..nv { let w2 = (v + 1) % nv; let (ex, ey) = (loc[w2].0 - loc[v].0, loc[w2].1 - loc[v].1); let l = (ex * ex + ey * ey).sqrt().max(1e-9); let nl = [ey / l, -ex / l, 0.0];
                let nw = [rm[0][0] * nl[0] + rm[0][1] * nl[1], rm[1][0] * nl[0] + rm[1][1] * nl[1], rm[2][0] * nl[0] + rm[2][1] * nl[1]]; let nd = nw[0] * lg[0] + nw[1] * lg[1] + nw[2] * lg[2]; let rf = [lg[0] - 2.0 * nd * nw[0], lg[1] - 2.0 * nd * nw[1], lg[2] - 2.0 * nd * nw[2]];
                let sp2 = rf[2].max(0.0).powi(14) + 0.4 * (-rf[2]).max(0.0).powi(14); let mut q = [(0.0, 0.0); 6]; q[0] = fp[v]; q[1] = fp[w2]; q[2] = bp[w2]; q[3] = bp[v];
                prims.push(Prim { depth: (shard[0].1 + shard[1].1) / 2.0 - 0.01, pts: q, n: 4, nz: nw[2].abs(), spec: sp2 + 0.12, rim_c: coc, blur: coc, alpha: al * 0.8, face: 2, u: r(15), tx: nw[0] * thick * 1.6, ty: nw[1] * thick * 1.6 }); } }
            for (fi, (pts, dz, _, _, _)) in shard.into_iter().enumerate() { prims.push(Prim { depth: dz, pts, n: nv, nz: nrm[2].abs(), spec, rim_c: fres, blur: coc, alpha: al, face: fi, u: r(15), tx: nrm[0] * thick, ty: nrm[1] * thick }); }
        }
        prims.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap());
        for pr in &prims {
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN); for v in 0..pr.n { x0 = x0.min(pr.pts[v].0); x1 = x1.max(pr.pts[v].0); y0 = y0.min(pr.pts[v].1); y1 = y1.max(pr.pts[v].1); }
            let pad = pr.blur * 1.2 + 2.0; let (xa, xb, ya, yb) = (((x0 - pad).floor().max(0.0)) as usize, ((x1 + pad).ceil().min(w as f64 - 1.0)) as usize, ((y0 - pad).floor().max(0.0)) as usize, ((y1 + pad).ceil().min(hh as f64 - 1.0)) as usize); if x0 > x1 || xa > xb || ya > yb || (x1 - x0) > w as f64 * 3.0 { continue; }
            // orientation sign for signed distance
            let mut area = 0.0; for v in 0..pr.n { let (a, b) = (pr.pts[v], pr.pts[(v + 1) % pr.n]); area += a.0 * b.1 - b.0 * a.1; } let sg = if area >= 0.0 { 1.0 } else { -1.0 }; if area.abs() < 0.5 { continue; }
            let (cxm, cym) = ((x0 + x1) * 0.5, (y0 + y1) * 0.5); let sw = (pr.blur * 0.9 + 0.7).max(0.7); let size_px = (x1 - x0).max(y1 - y0).max(1.0);
            for y in ya..=yb { for x in xa..=xb { let (px, py) = (x as f64 + 0.5, y as f64 + 0.5); let mut d = f64::MAX; for v in 0..pr.n { let (a, b) = (pr.pts[v], pr.pts[(v + 1) % pr.n]); let (ex, ey) = (b.0 - a.0, b.1 - a.1); let l2 = (ex * ex + ey * ey).max(1e-9); let tt = (((px - a.0) * ex + (py - a.1) * ey) / l2).clamp(0.0, 1.0); let dd = ((px - a.0 - tt * ex).hypot(py - a.1 - tt * ey)); let side = (ex * (py - a.1) - ey * (px - a.0)) * sg; let sd = if side >= 0.0 { dd } else { -dd }; if sd.abs() < d.abs() { d = sd; } }
                let cov = sstep((d + sw) / (2.0 * sw)); if cov <= 0.002 { continue; }
                let inner = d.max(0.0); let rimw = 1.1 * k + pr.blur * 0.5; let rim = (-inner / rimw).exp(); let (rr, rg, rb) = if e.dispersion > 0.0 { let o = e.dispersion * (0.7 + pr.blur * 0.1); ((-(inner + o).max(0.0) / rimw).exp(), rim, (-(inner - o).max(0.0) / rimw).exp()) } else { (rim, rim, rim) };
                let grad = 0.55 + 0.45 * (((px - cxm) / size_px + (py - cym) / size_px) * 0.9 + 0.5 + pr.u * 0.3).clamp(0.0, 1.0);
                let face_a = (e.fresnel * 1.3 * pr.rim_c * (0.6 + 0.8 * grad) + 0.17 + 0.2 * (1.0 - pr.nz) + 0.1 * grad + 1.0 * pr.spec * if pr.face == 0 { 1.0 } else { 0.6 }) * if pr.face == 1 { 0.7 } else { 1.0 };
                let face_a = if rf { face_a * 0.45 } else { face_a }; let a = ((face_a + 0.5 * rim * (0.5 + 0.5 * pr.face as f64 * 0.0 + 0.5)) * cov * pr.alpha).min(0.95); if a <= 0.002 { continue; }
                let br = e.brightness; let base = [e.tint[0], e.tint[1], e.tint[2]]; let sp = pr.spec * 0.8; let o = y * w + x;
                let cr = ((base[0] * (0.35 + 0.45 * grad) + sp) * (1.0 - rim) + rr * 1.0) * br; let cg = ((base[1] * (0.35 + 0.45 * grad) + sp) * (1.0 - rim) + rg * 1.0) * br; let cb = ((base[2] * (0.35 + 0.45 * grad) + sp) * (1.0 - rim) + rb * 1.0) * br;
                let (ca, aa) = (a as f32, acca[o]); // over (premultiplied accumulation)
                accc[o * 3] = accc[o * 3] * (1.0 - ca) + cr as f32 * ca; accc[o * 3 + 1] = accc[o * 3 + 1] * (1.0 - ca) + cg as f32 * ca; accc[o * 3 + 2] = accc[o * 3 + 2] * (1.0 - ca) + cb as f32 * ca; acca[o] = ca + aa * (1.0 - ca); if rf { let k2 = e.refract as f32 * k as f32 * (1.0 + 0.5 * rim as f32); dacc[o * 2] = dacc[o * 2] * (1.0 - ca) + (pr.tx as f32 * k2) * ca; dacc[o * 2 + 1] = dacc[o * 2 + 1] * (1.0 - ca) + (pr.ty as f32 * k2) * ca; } } } }
        // fold sub-frame into frame accumulators happens after all subs; keep running average via scaling below
        if sub + 1 < nsub { // store partial: normalise at the end using per-sub snapshots
            // snapshots are summed by re-using buffers: move current to f (additive average) then clear
                for o in 0..w * hh { let i4 = o * 4; f.d[i4] += accc[o * 3] / nsub as f32; f.d[i4 + 1] += accc[o * 3 + 1] / nsub as f32; f.d[i4 + 2] += accc[o * 3 + 2] / nsub as f32; f.d[i4 + 3] += acca[o] / nsub as f32; if rf { dsum[o * 2] += dacc[o * 2] / nsub as f32; dsum[o * 2 + 1] += dacc[o * 2 + 1] / nsub as f32; dacc[o * 2] = 0.0; dacc[o * 2 + 1] = 0.0; } accc[o * 3] = 0.0; accc[o * 3 + 1] = 0.0; accc[o * 3 + 2] = 0.0; acca[o] = 0.0; }
        }
    }
    if rf { for o in 0..w * hh { dsum[o * 2] += dacc[o * 2] / nsub as f32; dsum[o * 2 + 1] += dacc[o * 2 + 1] / nsub as f32; } DISP.with(|d| *d.borrow_mut() = Some((w, hh, dsum))); } else { DISP.with(|d| *d.borrow_mut() = None); }
    for o in 0..w * hh { let i4 = o * 4; f.d[i4] += accc[o * 3] / nsub as f32; f.d[i4 + 1] += accc[o * 3 + 1] / nsub as f32; f.d[i4 + 2] += accc[o * 3 + 2] / nsub as f32; f.d[i4 + 3] += acca[o] / nsub as f32;
        let a = f.d[i4 + 3]; if a > 1e-5 { for j in 0..3 { f.d[i4 + j] = (f.d[i4 + j] / a).min(1.5); } } }
}

fn bake_cached(e: &ShardBurst, k: f64, total: f64, hh: usize) -> std::sync::Arc<crate::physics::Traj> {
    use std::sync::{Arc, Mutex}; use std::collections::HashMap; use std::sync::OnceLock;
    static C: OnceLock<Mutex<HashMap<String, Arc<crate::physics::Traj>>>> = OnceLock::new();
    let key = format!("{:?}|{}|{}|{}", serde_json::to_string(e).unwrap_or_default(), k, total, hh); let m = C.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(t) = m.lock().unwrap().get(&key) { return t.clone(); }
    let mut bodies = vec![]; for i in 0..e.count as u64 { let r = |kk: u64| h(e.seed, i, kk);
        let size = (e.size_min * (e.size_max / e.size_min.max(0.1)).powf(r(1))) * k; let (th, ph) = (r(2) * std::f64::consts::TAU, (r(3) * 2.0 - 1.0).acos()); let dir = [ph.sin() * th.cos(), ph.sin() * th.sin() * 0.8, ph.cos()];
        let sp = e.speed * k * (0.35 + 1.1 * r(4).powf(1.2)) / (0.6 + size / (30.0 * k)).sqrt(); let mut vz = dir[2] * sp; vz -= e.toward_camera * sp * (0.3 + r(5)); let os = e.origin_spread * k;
        let p = [e.origin[0] * k + (r(50) - 0.5) * 2.0 * os, e.origin[1] * k + (r(51) - 0.5) * 2.0 * os * 0.6, e.origin[2] + (r(52) - 0.5) * 2.0 * os];
        let sd = e.spin.to_radians(); let w = [(r(60) - 0.5) * 2.0 * sd, (r(61) - 0.5) * 2.0 * sd, (r(62) - 0.5) * 2.0 * sd];
        let thick = (0.45 + 0.6 * r(14)) * size; let m = size * size * thick * 1e-3 + 1e-3;
        bodies.push(crate::physics::Body { p, v: [dir[0] * sp, dir[1] * sp, vz], q: crate::physics::quat_euler([r(6) * 360.0, r(8) * 360.0, r(10) * 360.0]), w, r: size * 0.55, m, birth: r(40) * e.spread_time }); }
    let par = crate::physics::Params { gravity: [0.0, e.gravity * k, 0.0], drag: e.drag, ang_drag: e.ang_drag, restitution: e.restitution, friction: e.friction, floor_y: e.floor.map(|f| f * k), collide: e.collide };
    let tj = Arc::new(crate::physics::bake(bodies, &par, total + 0.1)); m.lock().unwrap().insert(key, tj.clone()); tj
}

thread_local! { static DISP: std::cell::RefCell<Option<(usize, usize, Vec<f32>)>> = std::cell::RefCell::new(None); }
/// Take the premultiplied screen-space refraction field written by the last shard render (if refract > 0).
pub fn take_disp() -> Option<(usize, usize, Vec<f32>)> { DISP.with(|d| d.borrow_mut().take()) }
/// Bend the canvas behind a layer by a premultiplied displacement field; `layer_a` is the layer alpha (field is divided by it).
pub fn refract_canvas(out: &mut Frame, disp: &(usize, usize, Vec<f32>), layer: &Frame) {
    let (w, hh) = (out.w, out.h); if disp.0 != w || disp.1 != hh { return; } let src = out.d.clone();
    for y in 0..hh { for x in 0..w { let o = y * w + x; let a = layer.d[o * 4 + 3]; if a < 0.003 { continue; } let lim = 28.0 * (hh as f32 / 720.0); let (dx, dy) = ((disp.2[o * 2] / a.max(0.05)).clamp(-lim, lim), (disp.2[o * 2 + 1] / a.max(0.05)).clamp(-lim, lim));
        let (sx, sy) = ((x as f32 + dx).clamp(0.0, w as f32 - 1.001), (y as f32 + dy).clamp(0.0, hh as f32 - 1.001)); let (x0, y0) = (sx as usize, sy as usize); let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
        let wgt = a.min(1.0); for j in 0..3 { let v = |xx: usize, yy: usize| src[(yy * w + xx) * 4 + j]; let s = v(x0, y0) * (1.0 - fx) * (1.0 - fy) + v(x0 + 1, y0) * fx * (1.0 - fy) + v(x0, y0 + 1) * (1.0 - fx) * fy + v(x0 + 1, y0 + 1) * fx * fy; out.d[o * 4 + j] = src[o * 4 + j] * (1.0 - wgt) + s * wgt; } } }
}
