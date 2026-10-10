//! Scene3D: a small z-buffered 3D renderer for real geometry (boxes, rounded boxes, cylinders, capsules, spheres, planes, nulls)
//! with transform parenting, keyframed transforms, PBR-style materials (albedo, roughness, metal, emissive, opacity, Schlick fresnel),
//! directional/point/ambient lights, 2x2 supersampling and depth-of-field from the depth buffer. Seen through the project camera.
use crate::effects::Frame;
use crate::three_d::{rot_matrix, CamView, Camera};
use serde::{Deserialize, Serialize};

fn d3() -> [f64; 3] { [0.0; 3] } fn d1() -> [f64; 3] { [1.0; 3] } fn dsz() -> [f64; 3] { [100.0, 100.0, 100.0] } fn dcol() -> [f64; 3] { [0.8, 0.8, 0.8] } fn dr() -> f64 { 0.5 } fn dop() -> f64 { 1.0 } fn dkind() -> String { "box".into() }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Key { pub t: f64, #[serde(default)] pub pos: Option<[f64; 3]>, #[serde(default)] pub rot: Option<[f64; 3]>, #[serde(default)] pub scale: Option<[f64; 3]> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Obj {
    #[serde(default = "dkind")] pub kind: String, // box | rbox | cylinder | capsule | sphere | plane | null
    #[serde(default = "dsz")] pub size: [f64; 3], #[serde(default)] pub round: f64, // rbox corner radius
    #[serde(default = "d3")] pub pos: [f64; 3], #[serde(default = "d3")] pub rot: [f64; 3], #[serde(default = "d1")] pub scale: [f64; 3],
    #[serde(default)] pub parent: Option<usize>, #[serde(default)] pub keys: Vec<Key>,
    #[serde(default = "dcol")] pub color: [f64; 3], #[serde(default = "dr")] pub rough: f64, #[serde(default)] pub metal: f64, #[serde(default = "d3")] pub emissive: [f64; 3], #[serde(default = "dop")] pub opacity: f64,
    #[serde(default)] pub flat: bool,
}
fn dlk() -> String { "directional".into() } fn dli() -> f64 { 1.0 }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Light { #[serde(default = "dlk")] pub kind: String, #[serde(default = "d3")] pub dir: [f64; 3], #[serde(default = "d3")] pub pos: [f64; 3], #[serde(default = "d1")] pub color: [f64; 3], #[serde(default = "dli")] pub intensity: f64 }
fn dam() -> [f64; 3] { [0.08, 0.09, 0.11] }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene3D { #[serde(default)] pub objects: Vec<Obj>, #[serde(default)] pub lights: Vec<Light>, #[serde(default = "dam")] pub ambient: [f64; 3], #[serde(default)] pub dof: f64, #[serde(default)] pub focus: f64, #[serde(default)] pub ssaa: u32 }

type V3 = [f64; 3];
struct Tri { p: [V3; 3], n: [V3; 3] }
fn sub(a: V3, b: V3) -> V3 { [a[0] - b[0], a[1] - b[1], a[2] - b[2]] } fn dotv(a: V3, b: V3) -> f64 { a[0] * b[0] + a[1] * b[1] + a[2] * b[2] }
fn norm(a: V3) -> V3 { let m = dotv(a, a).sqrt().max(1e-12); [a[0] / m, a[1] / m, a[2] / m] }
fn cross(a: V3, b: V3) -> V3 { [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]] }
fn sm(x: f64) -> f64 { let t = x.clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) }

fn grid(f: &dyn Fn(f64, f64) -> (V3, V3), nu: usize, nv: usize, out: &mut Vec<Tri>) {
    for i in 0..nu { for j in 0..nv { let (u0, u1, v0, v1) = (i as f64 / nu as f64, (i + 1) as f64 / nu as f64, j as f64 / nv as f64, (j + 1) as f64 / nv as f64);
        let (a, b, c, d) = (f(u0, v0), f(u1, v0), f(u1, v1), f(u0, v1)); out.push(Tri { p: [a.0, b.0, c.0], n: [a.1, b.1, c.1] }); out.push(Tri { p: [a.0, c.0, d.0], n: [a.1, c.1, d.1] }); } }
}
fn mesh(o: &Obj) -> Vec<Tri> {
    let mut t = vec![]; let s = o.size; let tau = std::f64::consts::TAU; let pi = std::f64::consts::PI;
    match o.kind.as_str() {
        "plane" => { let (hx, hz) = (s[0] / 2.0, s[2] / 2.0); let n = [0.0, -1.0, 0.0]; grid(&|u, v| ([(u - 0.5) * 2.0 * hx, 0.0, (v - 0.5) * 2.0 * hz], n), 1, 1, &mut t); }
        "sphere" => { let r = s[0]; grid(&|u, v| { let (th, ph) = (u * tau, v * pi); let n = [ph.sin() * th.cos(), ph.cos(), ph.sin() * th.sin()]; ([n[0] * r, n[1] * r, n[2] * r], n) }, 48, 24, &mut t); }
        "cylinder" | "capsule" => { let (r, h) = (s[0], s[1]); let cap = o.kind == "capsule"; let hh = h / 2.0;
            // lathe profile param v in 0..1: top cap, side, bottom cap
            let f = |u: f64, v: f64| { let th = u * tau; let (c, sn) = (th.cos(), th.sin());
                if cap { let body = (hh - r).max(0.0); let tot = body * 2.0 + pi * r; let d = v * tot; if d < pi * r / 2.0 { let a = d / r; let y = -body - a.cos() * r; let rad = a.sin() * r; ([c * rad, y, sn * rad], norm([c * a.sin(), -a.cos(), sn * a.sin()])) }
                    else if d < pi * r / 2.0 + body * 2.0 { let y = -body + (d - pi * r / 2.0); ([c * r, y, sn * r], [c, 0.0, sn]) } else { let a = (d - pi * r / 2.0 - body * 2.0) / r; let y = body + a.sin() * r; let rad = a.cos() * r; ([c * rad, y, sn * rad], norm([c * a.cos(), a.sin(), sn * a.cos()])) } }
                else { if v < 0.1 { let k = v / 0.1; ([c * r * k, -hh, sn * r * k], [0.0, -1.0, 0.0]) } else if v < 0.9 { let y = -hh + (v - 0.1) / 0.8 * h; ([c * r, y, sn * r], [c, 0.0, sn]) } else { let k = (1.0 - v) / 0.1; ([c * r * k, hh, sn * r * k], [0.0, 1.0, 0.0]) } } };
            grid(&f, 40, 40, &mut t); }
        _ => { // box / rbox: subdivided cube pushed onto a rounded box
            let hx = [s[0] / 2.0, s[1] / 2.0, s[2] / 2.0]; let r = if o.kind == "rbox" { o.round.min(hx[0].min(hx[1]).min(hx[2]) - 1e-3).max(0.0) } else { 0.0 }; let nsub = if r > 0.0 { 10 } else { 1 };
            for face in 0..6 { let ax = face / 2; let sg = if face % 2 == 0 { 1.0 } else { -1.0 }; let (a1, a2) = ((ax + 1) % 3, (ax + 2) % 3);
                let f = |u: f64, v: f64| { let mut c = [0.0; 3]; c[ax] = sg; c[a1] = u * 2.0 - 1.0; c[a2] = v * 2.0 - 1.0; let mut p = [c[0] * hx[0], c[1] * hx[1], c[2] * hx[2]];
                    if r > 0.0 { let inner = [p[0].clamp(-(hx[0] - r), hx[0] - r), p[1].clamp(-(hx[1] - r), hx[1] - r), p[2].clamp(-(hx[2] - r), hx[2] - r)]; let d = norm(sub(p, inner)); let dd = sub(p, inner); let n = if dotv(dd, dd) < 1e-12 { let mut q = [0.0; 3]; q[ax] = sg; q } else { d }; p = [inner[0] + n[0] * r, inner[1] + n[1] * r, inner[2] + n[2] * r]; (p, n) }
                    else { let mut n = [0.0; 3]; n[ax] = sg; (p, n) } };
                grid(&f, nsub, nsub, &mut t); } }
    }
    t
}
fn mat_mul(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] { let mut r = [[0.0; 3]; 3]; for i in 0..3 { for j in 0..3 { for k in 0..3 { r[i][j] += a[i][k] * b[k][j]; } } } r }
fn mv(m: [[f64; 3]; 3], v: V3) -> V3 { [m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2], m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2], m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2]] }
/// Local (pos, rot, scale) at time t from keys (smoothstep between keys).
fn local(o: &Obj, t: f64) -> (V3, V3, V3) {
    let (mut p, mut r, mut s) = (o.pos, o.rot, o.scale); if o.keys.is_empty() { return (p, r, s); }
    let mut ks: Vec<&Key> = o.keys.iter().collect(); ks.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    let lerp3 = |a: V3, b: V3, u: f64| [a[0] + (b[0] - a[0]) * u, a[1] + (b[1] - a[1]) * u, a[2] + (b[2] - a[2]) * u];
    for (get, dst) in [(0usize, 0usize), (1, 1), (2, 2)] { let sel = |k: &Key| match get { 0 => k.pos, 1 => k.rot, _ => k.scale };
        let pts: Vec<(f64, V3)> = ks.iter().filter_map(|k| sel(k).map(|v| (k.t, v))).collect(); if pts.is_empty() { continue; }
        let v = if t <= pts[0].0 { pts[0].1 } else if t >= pts[pts.len() - 1].0 { pts[pts.len() - 1].1 } else { let i = pts.iter().position(|q| q.0 > t).unwrap(); let (a, b) = (pts[i - 1], pts[i]); lerp3(a.1, b.1, sm((t - a.0) / (b.0 - a.0).max(1e-9))) };
        match dst { 0 => p = v, 1 => r = v, _ => s = v } }
    (p, r, s)
}
fn world(objs: &[Obj], i: usize, t: f64, depth: usize) -> ([[f64; 3]; 3], V3, V3) {
    let (p, r, s) = local(&objs[i], t); let rm = rot_matrix(r);
    match objs[i].parent { Some(pi) if pi < objs.len() && pi != i && depth < 8 => { let (pr, pp, ps) = world(objs, pi, t, depth + 1); let sp = [p[0] * ps[0], p[1] * ps[1], p[2] * ps[2]]; let wp = mv(pr, sp); ([pr, rm].iter().fold([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]], |a, m| mat_mul(a, *m)), [pp[0] + wp[0], pp[1] + wp[1], pp[2] + wp[2]], [ps[0] * s[0], ps[1] * s[1], ps[2] * s[2]]) }
        _ => (rm, p, s) }
}

pub fn render(sc: &Scene3D, f: &mut Frame, t: f64, cam: &Camera) {
    let (w, h) = (f.w, f.h); let ss = if sc.ssaa == 0 { 2 } else { sc.ssaa.min(4) as usize }; let (sw, sh) = (w * ss, h * ss); let cv = CamView::new(sw, sh, cam); let eye = cv.eye(); let fl = cv.focal() / ss as f64;
    let mut zb = vec![f32::INFINITY; sw * sh]; let mut cb = vec![0f32; sw * sh * 4]; // premultiplied color + coverage
    let mut zout = vec![0f32; w * h];
    // gather draw list: opaque first, then transparent back-to-front by object center depth
    struct Draw { tris: Vec<Tri>, oi: usize, zc: f64 }
    let mut draws: Vec<Draw> = vec![];
    for (i, o) in sc.objects.iter().enumerate() { if o.kind == "null" { continue; } let (rm, wp, ws) = world(&sc.objects, i, t, 0); let tris = mesh(o);
        let tr = |p: V3| { let q = [p[0] * ws[0], p[1] * ws[1], p[2] * ws[2]]; let r = mv(rm, q); [r[0] + wp[0], r[1] + wp[1], r[2] + wp[2]] };
        let tn = |n: V3| norm(mv(rm, [n[0] / ws[0].abs().max(1e-6), n[1] / ws[1].abs().max(1e-6), n[2] / ws[2].abs().max(1e-6)]));
        let wt: Vec<Tri> = tris.iter().map(|q| Tri { p: [tr(q.p[0]), tr(q.p[1]), tr(q.p[2])], n: [tn(q.n[0]), tn(q.n[1]), tn(q.n[2])] }).collect();
        draws.push(Draw { tris: wt, oi: i, zc: wp[2] - eye[2] }); }
    draws.sort_by(|a, b| { let (ao, bo) = (sc.objects[a.oi].opacity >= 0.999, sc.objects[b.oi].opacity >= 0.999); bo.cmp(&ao).then(b.zc.partial_cmp(&a.zc).unwrap_or(std::cmp::Ordering::Equal)).then(std::cmp::Ordering::Equal) });
    // opaque (true) first: cmp(bo,ao) puts opaque first; within group farther first (fine for z-buffer)
    let lights: Vec<(V3, V3, f64, bool)> = sc.lights.iter().map(|l| (if l.kind == "point" { l.pos } else { norm([-l.dir[0], -l.dir[1], -l.dir[2]]) }, l.color, l.intensity, l.kind == "point")).collect();
    for d in &draws { let o = &sc.objects[d.oi]; let opaque = o.opacity >= 0.999; let rough = o.rough.clamp(0.04, 1.0); let shin = (2.0 / (rough * rough) - 2.0).clamp(2.0, 2000.0); let f0 = |c: f64| 0.04 * (1.0 - o.metal) + c * o.metal;
        for tri in &d.tris { let mut sp = [(0.0, 0.0, 0.0); 3]; let mut ok = true; for k in 0..3 { match cv.project(tri.p[k]) { Some(q) => sp[k] = q, None => ok = false } } if !ok { continue; }
            let area = (sp[1].0 - sp[0].0) * (sp[2].1 - sp[0].1) - (sp[2].0 - sp[0].0) * (sp[1].1 - sp[0].1); if area.abs() < 1e-9 { continue; }
            let (x0, x1) = (sp[0].0.min(sp[1].0).min(sp[2].0).floor().max(0.0) as usize, (sp[0].0.max(sp[1].0).max(sp[2].0).ceil().min(sw as f64 - 1.0)).max(0.0) as usize); let (y0, y1) = (sp[0].1.min(sp[1].1).min(sp[2].1).floor().max(0.0) as usize, (sp[0].1.max(sp[1].1).max(sp[2].1).ceil().min(sh as f64 - 1.0)).max(0.0) as usize);
            if sp[0].0.max(sp[1].0).max(sp[2].0) < 0.0 || sp[0].1.max(sp[1].1).max(sp[2].1) < 0.0 { continue; }
            let iz = [1.0 / sp[0].2, 1.0 / sp[1].2, 1.0 / sp[2].2];
            for y in y0..=y1 { for x in x0..=x1 { let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let w0 = ((sp[1].0 - px) * (sp[2].1 - py) - (sp[2].0 - px) * (sp[1].1 - py)) / area; let w1 = ((sp[2].0 - px) * (sp[0].1 - py) - (sp[0].0 - px) * (sp[2].1 - py)) / area; let w2 = 1.0 - w0 - w1;
                if w0 < -1e-9 || w1 < -1e-9 || w2 < -1e-9 { continue; } let izp = w0 * iz[0] + w1 * iz[1] + w2 * iz[2]; let z = 1.0 / izp; let o_ = y * sw + x; if (z as f32) >= zb[o_] { continue; }
                let (b0, b1, b2) = (w0 * iz[0] * z, w1 * iz[1] * z, w2 * iz[2] * z); let pw = [0, 1, 2].map(|a| b0 * tri.p[0][a] + b1 * tri.p[1][a] + b2 * tri.p[2][a]); let mut n = norm([0, 1, 2].map(|a| b0 * tri.n[0][a] + b1 * tri.n[1][a] + b2 * tri.n[2][a]));
                if o.flat { n = norm(cross(sub(tri.p[1], tri.p[0]), sub(tri.p[2], tri.p[0]))); }
                let vdir = norm(sub(eye, pw)); if dotv(n, vdir) < 0.0 { n = [-n[0], -n[1], -n[2]]; }
                let nv = dotv(n, vdir).max(0.0); let mut col = [sc.ambient[0] * o.color[0], sc.ambient[1] * o.color[1], sc.ambient[2] * o.color[2]];
                for (lp, lc, li, point) in &lights { let (l, att) = if *point { let dv = sub(*lp, pw); let d2 = dotv(dv, dv).max(1.0); (norm(dv), 1.0e5 / d2) } else { (*lp, 1.0) }; let ndl = dotv(n, l).max(0.0); if ndl <= 0.0 { continue; }
                    let hv = norm([l[0] + vdir[0], l[1] + vdir[1], l[2] + vdir[2]]); let ndh = dotv(n, hv).max(0.0); let vdh = dotv(vdir, hv).max(0.0); let fr = (1.0 - vdh).powi(5);
                    let sp_ = ndh.powf(shin) * (shin + 8.0) / 25.0 * ndl; let li2 = li * att;
                    for c in 0..3 { let fc = f0(o.color[c]) + (1.0 - f0(o.color[c])) * fr; col[c] += lc[c] * li2 * (o.color[c] * (1.0 - o.metal) * ndl / 1.0 + fc * sp_); } }
                let rim = (1.0 - nv).powi(5) * (1.0 - rough) * 0.5; for c in 0..3 { col[c] += o.emissive[c] + rim * 0.5; }
                let a = if opaque { 1.0 } else { (o.opacity + (1.0 - o.opacity) * rim * 2.0).clamp(0.0, 1.0) };
                let i4 = o_ * 4; if opaque { zb[o_] = z as f32; for c in 0..3 { cb[i4 + c] = col[c] as f32; } cb[i4 + 3] = 1.0; } else { let ia = 1.0 - a as f32; for c in 0..3 { cb[i4 + c] = cb[i4 + c] * ia + col[c] as f32 * a as f32; } cb[i4 + 3] = cb[i4 + 3] * ia + a as f32; } } } } }
    // downsample + depth
    let ssn = (ss * ss) as f32; let mut tmp = vec![0f32; w * h * 4];
    for y in 0..h { for x in 0..w { let mut acc = [0f32; 4]; let mut zs = 0.0f32; let mut zn = 0f32; for sy in 0..ss { for sx in 0..ss { let o = ((y * ss + sy) * sw + x * ss + sx) * 4; for c in 0..4 { acc[c] += cb[o + c]; } let z = zb[(y * ss + sy) * sw + x * ss + sx]; if z.is_finite() { zs += z; zn += 1.0; } } }
        let o = (y * w + x) * 4; for c in 0..4 { tmp[o + c] = acc[c] / ssn; } zout[y * w + x] = if zn > 0.0 { zs / zn } else { 1e9 }; } }
    let focus = if sc.focus > 0.0 { sc.focus } else { cv.focal() };
    if sc.dof > 0.0 { let src = tmp.clone(); let taps: Vec<(f32, f32)> = (0..24).map(|i| { let a = i as f32 * 2.399963; let r = ((i as f32 + 0.5) / 24.0).sqrt(); (a.cos() * r, a.sin() * r) }).collect();
        for y in 0..h { for x in 0..w { let z = zout[y * w + x] as f64; let zz = if z > 1e8 { focus * 4.0 } else { z }; let coc = (sc.dof * (zz - focus).abs() / zz * fl * 0.04 * (h as f64 / 720.0)).min(40.0) as f32; if coc < 0.6 { continue; }
            let mut acc = [0f32; 4]; for (tx, ty) in &taps { let (sx, sy) = (((x as f32 + tx * coc).round() as i32).clamp(0, w as i32 - 1) as usize, ((y as f32 + ty * coc).round() as i32).clamp(0, h as i32 - 1) as usize); let o = (sy * w + sx) * 4; for c in 0..4 { acc[c] += src[o + c]; } }
            let o = (y * w + x) * 4; for c in 0..4 { tmp[o + c] = acc[c] / 24.0; } } } }
    for i in 0..w * h { let a = tmp[i * 4 + 3]; let o = i * 4; if a > 1e-4 { for c in 0..3 { f.d[o + c] = (tmp[o + c] / a).min(4.0); } f.d[o + 3] = a.min(1.0); } }
}
