//! fracture_glass: procedural laminated-glass shatter as a native effect.
//! The frame itself is the glass: cracks grow outward from an impact point, facets refract the
//! live source, then cells release in a radial stagger and fall (gravity, spin, push) carrying
//! the source texture at their rest UV. Deterministic from `seed`. Single impact point.
use crate::effects::{p, Frame, Params};

struct Rng(u64);
impl Rng {
    fn n(&mut self) -> u64 { self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15); let mut z = self.0; z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9); z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB); z ^ (z >> 31) }
    fn f(&mut self) -> f32 { (self.n() >> 40) as f32 / 16777216.0 }
    fn s(&mut self) -> f32 { self.f() * 2.0 - 1.0 }
}
type V = (f32, f32);
fn ss(a: f32, b: f32, x: f32) -> f32 { let t = ((x - a) / (b - a).max(1e-6)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }
fn dist(a: V, b: V) -> f32 { ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt() }

struct Cell { poly: Vec<V>, c: V, d: f32, tilt: V, shade: f32, t_rel: f32, falls: bool, vel: V, spin: f32, z: f32 }
struct Seg { a: V, b: V, d0: f32, d1: f32, g: f32, cells: [usize; 2] }

fn spans(poly: &[V], w: usize, h: usize, mut f: impl FnMut(usize, usize, f32)) {
    let (mut y0, mut y1) = (f32::MAX, f32::MIN); for q in poly { y0 = y0.min(q.1); y1 = y1.max(q.1); }
    let ya = (y0.floor().max(0.0)) as usize; let yb = (y1.ceil().min(h as f32)) as usize; if ya >= yb { return; }
    const S: usize = 4; let mut row = vec![0f32; w]; let n = poly.len();
    for y in ya..yb {
        let (mut mn, mut mx) = (w, 0usize); let mut any = false;
        for sidx in 0..S {
            let sy = y as f32 + (sidx as f32 + 0.5) / S as f32; let mut xs: Vec<f32> = Vec::with_capacity(8);
            for i in 0..n { let (a, b) = (poly[i], poly[(i + 1) % n]); if (a.1 <= sy) != (b.1 <= sy) { xs.push(a.0 + (sy - a.1) * (b.0 - a.0) / (b.1 - a.1)); } }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pr in xs.chunks(2) { if pr.len() < 2 { break; } let (x0, x1) = (pr[0].max(0.0), pr[1].min(w as f32)); if x1 <= x0 { continue; }
                let (p0, p1) = (x0 as usize, (x1 as usize).min(w - 1)); any = true; mn = mn.min(p0); mx = mx.max(p1);
                if p0 == p1 { row[p0] += (x1 - x0) / S as f32; } else { row[p0] += ((p0 + 1) as f32 - x0) / S as f32; for x in p0 + 1..p1 { row[x] += 1.0 / S as f32; } row[p1] += (x1 - p1 as f32) / S as f32; } }
        }
        if any { for x in mn..=mx.min(w - 1) { let c = row[x]; if c > 0.0 { f(y, x, c.min(1.0)); } row[x] = 0.0; } }
    }
}
fn bil(src: &[f32], w: usize, h: usize, x: f32, y: f32) -> [f32; 4] {
    let x = x.clamp(0.0, w as f32 - 1.001); let y = y.clamp(0.0, h as f32 - 1.001); let (x0, y0) = (x as usize, y as usize); let (fx, fy) = (x - x0 as f32, y - y0 as f32); let mut o = [0f32; 4];
    for j in 0..4 { let a = src[(y0 * w + x0) * 4 + j]; let b = src[(y0 * w + x0 + 1) * 4 + j]; let c = src[((y0 + 1) * w + x0) * 4 + j]; let d = src[((y0 + 1) * w + x0 + 1) * 4 + j]; o[j] = a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy; } o
}
fn seg_cov(f: &mut [f32], g: &mut [f32], w: usize, h: usize, a: V, b: V, wd: f32, glint: f32) {
    let pad = wd + 1.5; let (x0, x1) = ((a.0.min(b.0) - pad).floor().max(0.0) as usize, ((a.0.max(b.0) + pad).ceil().min(w as f32 - 1.0)).max(0.0) as usize);
    let (y0, y1) = ((a.1.min(b.1) - pad).floor().max(0.0) as usize, ((a.1.max(b.1) + pad).ceil().min(h as f32 - 1.0)).max(0.0) as usize);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1); let l2 = (dx * dx + dy * dy).max(1e-6);
    for y in y0..=y1 { for x in x0..=x1 { let (px, py) = (x as f32 + 0.5, y as f32 + 0.5); let t = (((px - a.0) * dx + (py - a.1) * dy) / l2).clamp(0.0, 1.0); let d = ((px - a.0 - t * dx).powi(2) + (py - a.1 - t * dy).powi(2)).sqrt();
        let c = (wd * 0.5 + 0.5 - d).clamp(0.0, 1.0); if c > f[y * w + x] { f[y * w + x] = c; }
        let gg = (wd * 1.6 + 0.5 - d).clamp(0.0, 1.0) * glint; if gg > g[y * w + x] { g[y * w + x] = gg; } } }
}

pub fn apply(pr: &Params, f: &mut Frame, t: f64, seed: u64) -> Result<(), String> {
    let (w, h) = (f.w, f.h); if w < 8 || h < 8 { return Ok(()); }
    let g = |k: &str, d: f64| { let v = p(pr, k, d as f64, t); if v.is_finite() { v } else { d as f32 } };
    let hh = h as f32; let t0 = g("t0", 0.0); let tau = t as f32 - t0; if tau < 0.0 { return Ok(()); }
    let (ix, iy) = (g("impact_x", 0.5).clamp(0.0, 1.0) * w as f32, g("impact_y", 0.5).clamp(0.0, 1.0) * hh); let imp = (ix, iy);
    let nr = g("radials", 18.0).clamp(6.0, 48.0) as usize; let nk = g("rings", 9.0).clamp(3.0, 20.0) as usize;
    let grow = g("ring_growth", 1.5).clamp(1.15, 2.2); let r0 = g("r0", 0.03).clamp(0.005, 0.2) * hh;
    let speed = g("crack_speed", 5.0).clamp(0.2, 60.0) * hh; let front = speed * tau;
    let rel_t0 = g("release_delay", 0.1).max(0.0); let rel_spread = g("release_spread", 0.12).max(0.0); let rel_rad = g("release_radius", 0.6).clamp(0.02, 3.0) * hh;
    let grav = g("gravity", 2.5) * hh; let push = g("push", 0.35) * hh; let spin_m = g("spin", 5.0); let cam = g("toward_camera", 0.25);
    let fade_after = g("fade_after", 0.6); let fade_dur = g("fade_duration", 0.35).max(0.01);
    let refr = g("refraction", 7.0); let disp = g("dispersion", 0.35).clamp(0.0, 1.5); let cw = g("crack_width", 1.5).clamp(0.3, 8.0); let glint = g("glint", 0.8).clamp(0.0, 3.0); let bevel = g("bevel", 3.0).clamp(0.0, 20.0);
    let merge_p = g("ring_merge", 0.25).clamp(0.0, 0.9); 
    let mut rg = Rng(seed ^ 0xF4AC_7E55); let mut rg = { for _ in 0..3 { rg.n(); } rg };
    // --- polar vertex grid: V[k][i], k=0 is the impact point; radial rays random-walk in angle ---
    let dmax = { let mut m = 0f32; for c in [(0.0, 0.0), (w as f32, 0.0), (0.0, hh), (w as f32, hh)] { m = m.max(dist(imp, c)); } m } * 1.15;
    let mut radii = vec![0f32; nk + 1]; let mut r = r0; for k in 1..=nk { radii[k] = r; r *= grow; } let sc = dmax / radii[nk].max(1.0); if sc > 1.0 { for k in 1..=nk { radii[k] *= sc; } } else { radii[nk] = dmax; }
    let mut ang = vec![0f32; nr]; let two = std::f32::consts::TAU; for i in 0..nr { ang[i] = two * (i as f32 + 0.35 * rg.s()) / nr as f32; }
    let mut vs = vec![vec![imp; nr]; nk + 1];
    let mut walk = vec![0f32; nr];
    for k in 1..=nk { for i in 0..nr { walk[i] += 0.18 * rg.s() * (two / nr as f32); let a = ang[i] + walk[i]; let rr = radii[k] * (1.0 + 0.12 * rg.s() * if k == nk { 0.0 } else { 1.0 }); vs[k][i] = (ix + rr * a.cos(), iy + rr * a.sin()); } }
    // --- cells (k,i): k=0 triangles; merge ring-direction pairs for k>=2 ---
    let mut cells: Vec<Cell> = Vec::new(); let mut owner = vec![vec![usize::MAX; nr]; nk]; let mut ring_present = vec![vec![true; nr]; nk + 1];
    for k in 0..nk { for i in 0..nr { if owner[k][i] != usize::MAX { continue; } let i2 = (i + 1) % nr;
        let mut poly: Vec<V> = if k == 0 { vec![imp, vs[1][i], vs[1][i2]] } else { vec![vs[k][i], vs[k][i2], vs[k + 1][i2], vs[k + 1][i]] };
        let mut ks = vec![k];
        if k >= 2 && k + 1 < nk && owner[k + 1][i] == usize::MAX && rg.f() < merge_p { poly = vec![vs[k][i], vs[k][i2], vs[k + 1][i2], vs[k + 2][i2], vs[k + 2][i], vs[k + 1][i]]; ks.push(k + 1); ring_present[k + 1][i] = false; }
        let id = cells.len(); for kk in ks { owner[kk][i] = id; }
        let mut c = (0.0, 0.0); for q in &poly { c.0 += q.0; c.1 += q.1; } c = (c.0 / poly.len() as f32, c.1 / poly.len() as f32);
        let d = dist(c, imp); let rad = (c.0 - ix, c.1 - iy); let rl = (rad.0 * rad.0 + rad.1 * rad.1).sqrt().max(1e-3);
        let falls = d <= rel_rad && rg.f() < 1.0 - ss(0.75, 1.0, d / rel_rad);
        let t_rel = (rel_t0 + rel_spread * (d / rel_rad) + 0.04 * rg.f()).max(d / speed + 0.02);
        let sp = push * (0.4 + 0.8 * rg.f()); let vel = (rad.0 / rl * sp + 0.15 * push * rg.s(), rad.1 / rl * sp - 0.2 * push * rg.f());
        cells.push(Cell { poly, c, d, tilt: (rg.s() * refr, rg.s() * refr), shade: 1.0 + 0.04 * rg.s(), t_rel, falls, vel, spin: spin_m * rg.s() * 1.5, z: rg.f() }); } }
    // --- crack segments ---
    let mut segs: Vec<Seg> = Vec::new();
    for k in 0..nk { for i in 0..nr { let (a, b) = if k == 0 { (imp, vs[1][i]) } else { (vs[k][i], vs[k + 1][i]) };
        let left = owner[k][i]; let right = owner[k][(i + nr - 1) % nr]; segs.push(Seg { a, b, d0: dist(a, imp), d1: dist(b, imp), g: rg.f(), cells: [left, right] }); } }
    for k in 1..=nk { for i in 0..nr { if !ring_present[k][i] { continue; } let (a, b) = (vs[k][i], vs[k][(i + 1) % nr]); let inner = owner[k - 1][i]; let outer = if k < nk { owner[k][i] } else { usize::MAX };
        let lag = 0.02 * hh; segs.push(Seg { a, b, d0: dist(a, imp).min(dist(b, imp)) + lag, d1: dist(a, imp).max(dist(b, imp)) + lag + 1.0, g: rg.f(), cells: [inner, outer] }); } }
    // --- render ---
    let src = f.d.clone(); let n = w * h;
    let released = |c: &Cell| c.falls && tau >= c.t_rel;
    // 1. intact facets: refraction + dispersion, once the crack front has passed
    for (ci, c) in cells.iter().enumerate() { if released(c) || ci >= cells.len() { continue; } let act = ss(c.d - 0.02 * hh, c.d + 0.08 * hh, front); if act <= 0.0 { continue; }
        let (tx, ty) = (c.tilt.0 * act, c.tilt.1 * act); let sh = 1.0 + (c.shade - 1.0) * act;
        spans(&c.poly, w, h, |y, x, cov| { if cov < 0.5 { return; } let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5); let o = (y * w + x) * 4;
            let rch = bil(&src, w, h, fx + tx * (1.0 + disp), fy + ty * (1.0 + disp)); let gch = bil(&src, w, h, fx + tx, fy + ty); let bch = bil(&src, w, h, fx + tx * (1.0 - disp).max(0.0), fy + ty * (1.0 - disp).max(0.0));
            f.d[o] = rch[0] * sh; f.d[o + 1] = gch[1] * sh; f.d[o + 2] = bch[2] * sh; f.d[o + 3] = gch[3]; }); }
    // 2. punch holes where cells have released
    let mut order: Vec<usize> = (0..cells.len()).filter(|&i| released(&cells[i])).collect();
    for &ci in &order { spans(&cells[ci].poly, w, h, |y, x, cov| { let o = (y * w + x) * 4; for j in 0..3 { f.d[o + j] *= (1.0 - cov); } f.d[o + 3] *= 1.0 - cov; }); }
    // 3. cracks (skip edges between two released cells: those became hole boundaries)
    let mut cov = vec![0f32; n]; let mut gl = vec![0f32; n];
    for s in &segs { let rel = |c: usize| c != usize::MAX && released(&cells[c]); if rel(s.cells[0]) && (rel(s.cells[1]) || s.cells[1] == usize::MAX) { continue; }
        let amt = ((front - s.d0) / (s.d1 - s.d0).max(1.0)).clamp(0.0, 1.0); if amt <= 0.0 { continue; } let b = (s.a.0 + (s.b.0 - s.a.0) * amt, s.a.1 + (s.b.1 - s.a.1) * amt);
        let dir = ((s.b.1 - s.a.1).atan2(s.b.0 - s.a.0) * 2.0).sin() * 0.5 + 0.5; seg_cov(&mut cov, &mut gl, w, h, s.a, b, cw, glint * (0.35 + 0.65 * dir) * (0.4 + 0.6 * s.g)); }
    for i in 0..n { let o = i * 4; if f.d[o + 3] <= 0.0 { continue; } let c = cov[i]; let gg = gl[i];
        if c > 0.0 || gg > 0.0 { for j in 0..3 { f.d[o + j] = f.d[o + j] * (1.0 - 0.45 * c) + (gg * 0.55 + c * 0.25) * (0.6 + 0.4 * f.d[o + j].min(1.0)); } } }
    // 4. falling shards sample the live source at rest UV
    order.sort_by(|&a, &b| cells[a].z.partial_cmp(&cells[b].z).unwrap_or(std::cmp::Ordering::Equal));
    for &ci in &order { let c = &cells[ci]; let ts = tau - c.t_rel; let fade = 1.0 - ss(fade_after, fade_after + fade_dur, ts); if fade <= 0.0 { continue; }
        let (cx, cy) = (c.c.0 + c.vel.0 * ts, c.c.1 + c.vel.1 * ts + 0.5 * grav * ts * ts); let rot = c.spin * ts; let scl = 1.0 + cam * ts * (0.5 + c.z); let (cr, sr) = (rot.cos(), rot.sin());
        let tp: Vec<V> = c.poly.iter().map(|q| { let (dx, dy) = ((q.0 - c.c.0) * scl, (q.1 - c.c.1) * scl); (cx + dx * cr - dy * sr, cy + dx * sr + dy * cr) }).collect();
        let m = c.poly.len(); let light = 0.5 + 0.5 * (rot * 1.7 + c.z * 6.0).sin();
        spans(&tp, w, h, |y, x, cv| { let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy); let (ux, uy) = ((px * cr + py * sr) / scl + c.c.0, (-px * sr + py * cr) / scl + c.c.1);
            let mut dm = f32::MAX; for e in 0..m { let (a, b) = (c.poly[e], c.poly[(e + 1) % m]); let (dx, dy) = (b.0 - a.0, b.1 - a.1); let l2 = (dx * dx + dy * dy).max(1e-6); let tt = (((ux - a.0) * dx + (uy - a.1) * dy) / l2).clamp(0.0, 1.0); dm = dm.min(((ux - a.0 - tt * dx).powi(2) + (uy - a.1 - tt * dy).powi(2)).sqrt()); }
            let s = bil(&src, w, h, ux, uy); let bv = if bevel > 0.0 { 1.0 - ss(0.0, bevel, dm) } else { 0.0 }; let lum = 0.35 * glint * bv * (0.3 + light);
            let al = (cv * fade * s[3]).clamp(0.0, 1.0); if al <= 0.0 { return; } let o = (y * w + x) * 4; let oa = f.d[o + 3]; let na = al + oa * (1.0 - al);
            for j in 0..3 { let col = s[j] * c.shade + lum; f.d[o + j] = (col * al + f.d[o + j] * oa * (1.0 - al)) / na.max(1e-6); } f.d[o + 3] = na; }); }
    Ok(())
}
