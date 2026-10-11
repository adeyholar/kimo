//! Classical (non-ML) trimap alpha matting: sampling-based (F,B) pair estimation for every unknown pixel, edge-aware guided-filter
//! smoothing, and foreground colour decontamination. Keeps partial alpha on hair, motion blur and glass edges. No learned model is used.
pub struct Opts { pub band: usize, pub samples: usize, pub radius: usize, pub guide_radius: usize, pub eps: f32, pub spatial: f32 }
impl Default for Opts { fn default() -> Self { Opts { band: 8, samples: 10, radius: 0, guide_radius: 4, eps: 1e-4, spatial: 0.0 } } }
pub struct Matte { pub alpha: Vec<f32>, pub fg: Vec<f32>, pub unknown: usize, pub trimap: Vec<u8> }

fn dist_transform(mask: &[bool], w: usize, h: usize, maxd: usize) -> Vec<f32> { // chamfer distance to nearest true, capped
    let inf = 1e9f32; let mut d: Vec<f32> = mask.iter().map(|&m| if m { 0.0 } else { inf }).collect(); let (a, b) = (1.0f32, 1.4142f32);
    for y in 0..h { for x in 0..w { let i = y * w + x; let mut v = d[i]; if x > 0 { v = v.min(d[i - 1] + a); } if y > 0 { v = v.min(d[i - w] + a); if x > 0 { v = v.min(d[i - w - 1] + b); } if x + 1 < w { v = v.min(d[i - w + 1] + b); } } d[i] = v; } }
    for y in (0..h).rev() { for x in (0..w).rev() { let i = y * w + x; let mut v = d[i]; if x + 1 < w { v = v.min(d[i + 1] + a); } if y + 1 < h { v = v.min(d[i + w] + a); if x + 1 < w { v = v.min(d[i + w + 1] + b); } if x > 0 { v = v.min(d[i + w - 1] + b); } } d[i] = v; } }
    let _ = maxd; d
}
/// Trimap from a rough alpha: 255 = sure foreground, 0 = sure background, 128 = unknown band of `band` px each side of the 0.5 contour.
/// Pixels with intermediate alpha in the seed (already soft) are always unknown.
pub fn trimap_from_alpha(seed: &[f32], w: usize, h: usize, band: usize) -> Vec<u8> {
    let fgm: Vec<bool> = seed.iter().map(|&a| a > 0.5).collect(); let bgm: Vec<bool> = fgm.iter().map(|&f| !f).collect();
    let to_bg = dist_transform(&bgm, w, h, band); let to_fg = dist_transform(&fgm, w, h, band); // distance of each pixel to nearest bg / fg pixel
    (0..w * h).map(|i| { let soft = seed[i] > 0.02 && seed[i] < 0.98; if soft { 128 } else if fgm[i] { if to_bg[i] > band as f32 { 255 } else { 128 } } else if to_fg[i] > band as f32 { 0 } else { 128 } }).collect()
}
fn box_filter(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0f32; w * h]; let mut out = vec![0f32; w * h];
    for y in 0..h { let mut acc = 0.0f32; let mut cnt = 0.0f32; for x in 0..=r.min(w - 1) { acc += src[y * w + x]; cnt += 1.0; } for x in 0..w { tmp[y * w + x] = acc / cnt; let add = x + r + 1; if add < w { acc += src[y * w + add]; cnt += 1.0; } if x >= r { acc -= src[y * w + x - r]; cnt -= 1.0; } } }
    for x in 0..w { let mut acc = 0.0f32; let mut cnt = 0.0f32; for y in 0..=r.min(h - 1) { acc += tmp[y * w + x]; cnt += 1.0; } for y in 0..h { out[y * w + x] = acc / cnt; let add = y + r + 1; if add < h { acc += tmp[add * w + x]; cnt += 1.0; } if y >= r { acc -= tmp[(y - r) * w + x]; cnt -= 1.0; } } }
    out
}
/// Guided filter (He et al.) with a colour guide (3 channels), channel-wise covariance.
pub fn guided_filter(guide: &[f32], p: &[f32], w: usize, h: usize, r: usize, eps: f32) -> Vec<f32> {
    let n = w * h; let ch = |c: usize| -> Vec<f32> { (0..n).map(|i| guide[i * 3 + c]).collect() }; let g = [ch(0), ch(1), ch(2)];
    let mg: Vec<Vec<f32>> = g.iter().map(|c| box_filter(c, w, h, r)).collect(); let mp = box_filter(p, w, h, r);
    let gp: Vec<Vec<f32>> = (0..3).map(|c| { let m: Vec<f32> = (0..n).map(|i| g[c][i] * p[i]).collect(); box_filter(&m, w, h, r) }).collect();
    let mut cov = vec![vec![0f32; 6]; 0]; let _ = &mut cov; let mut vv = vec![vec![0f32; n]; 6]; let pairs = [(0, 0), (0, 1), (0, 2), (1, 1), (1, 2), (2, 2)];
    for (k, (a, b)) in pairs.iter().enumerate() { let m: Vec<f32> = (0..n).map(|i| g[*a][i] * g[*b][i]).collect(); let mm = box_filter(&m, w, h, r); for i in 0..n { vv[k][i] = mm[i] - mg[*a][i] * mg[*b][i]; } }
    let mut a3 = vec![vec![0f32; n]; 3]; let mut b = vec![0f32; n];
    for i in 0..n { let s = [vv[0][i] + eps, vv[1][i], vv[2][i], vv[3][i] + eps, vv[4][i], vv[5][i] + eps]; // symmetric 3x3
        let det = s[0] * (s[3] * s[5] - s[4] * s[4]) - s[1] * (s[1] * s[5] - s[4] * s[2]) + s[2] * (s[1] * s[4] - s[3] * s[2]); let id = if det.abs() < 1e-20 { 0.0 } else { 1.0 / det };
        let inv = [(s[3] * s[5] - s[4] * s[4]) * id, (s[2] * s[4] - s[1] * s[5]) * id, (s[1] * s[4] - s[2] * s[3]) * id, (s[0] * s[5] - s[2] * s[2]) * id, (s[1] * s[2] - s[0] * s[4]) * id, (s[0] * s[3] - s[1] * s[1]) * id];
        let c = [gp[0][i] - mg[0][i] * mp[i], gp[1][i] - mg[1][i] * mp[i], gp[2][i] - mg[2][i] * mp[i]];
        let a = [inv[0] * c[0] + inv[1] * c[1] + inv[2] * c[2], inv[1] * c[0] + inv[3] * c[1] + inv[4] * c[2], inv[2] * c[0] + inv[4] * c[1] + inv[5] * c[2]];
        for k in 0..3 { a3[k][i] = a[k]; } b[i] = mp[i] - a[0] * mg[0][i] - a[1] * mg[1][i] - a[2] * mg[2][i]; }
    let ma: Vec<Vec<f32>> = a3.iter().map(|x| box_filter(x, w, h, r)).collect(); let mb = box_filter(&b, w, h, r);
    (0..n).map(|i| ma[0][i] * g[0][i] + ma[1][i] * g[1][i] + ma[2][i] * g[2][i] + mb[i]).collect()
}
/// Matte an RGB float image (0..1, w*h*3) given a trimap (255/0/128).
pub fn matte(img: &[f32], trimap: &[u8], w: usize, h: usize, o: &Opts) -> Matte {
    let n = w * h; let rad = if o.radius > 0 { o.radius } else { o.band * 2 + 6 } as i32; let k = o.samples.max(2);
    let mut alpha: Vec<f32> = trimap.iter().map(|&t| if t == 255 { 1.0 } else { 0.0 }).collect(); let mut conf = vec![0f32; n]; let mut fg = img.to_vec(); let mut unknown = 0;
    let px = |i: usize| [img[i * 3], img[i * 3 + 1], img[i * 3 + 2]];
    for y in 0..h { for x in 0..w { let i = y * w + x; if trimap[i] != 128 { continue; } unknown += 1;
        let mut fs: Vec<(f32, usize)> = vec![]; let mut bs: Vec<(f32, usize)> = vec![];
        let (y0, y1, x0, x1) = ((y as i32 - rad).max(0), (y as i32 + rad).min(h as i32 - 1), (x as i32 - rad).max(0), (x as i32 + rad).min(w as i32 - 1));
        for yy in y0..=y1 { for xx in x0..=x1 { let j = yy as usize * w + xx as usize; let t = trimap[j]; if t == 128 { continue; } let d = ((yy - y as i32).pow(2) + (xx - x as i32).pow(2)) as f32; if d > (rad * rad) as f32 { continue; } if t == 255 { fs.push((d, j)); } else { bs.push((d, j)); } } }
        if fs.is_empty() || bs.is_empty() { alpha[i] = if fs.is_empty() { 0.0 } else { 1.0 }; continue; }
        // keep k nearest of each, spread: take every other of the 2k nearest to get variety
        let pick = |v: &mut Vec<(f32, usize)>| -> Vec<usize> { v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap()); v.iter().take(k * 2).step_by(2).map(|s| s.1).collect() };
        let (fi, bi) = (pick(&mut fs), pick(&mut bs)); let c = px(i); let mut best: Vec<(f32, f32, usize, usize)> = vec![];
        for &f in &fi { let fc = px(f); for &b in &bi { let bc = px(b); let d = [fc[0] - bc[0], fc[1] - bc[1], fc[2] - bc[2]]; let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2]; if dd < 1e-5 { continue; }
            let a = (((c[0] - bc[0]) * d[0] + (c[1] - bc[1]) * d[1] + (c[2] - bc[2]) * d[2]) / dd).clamp(0.0, 1.0); let r = [c[0] - a * fc[0] - (1.0 - a) * bc[0], c[1] - a * fc[1] - (1.0 - a) * bc[1], c[2] - a * fc[2] - (1.0 - a) * bc[2]];
            let cost = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt() / dd.sqrt().max(0.05); best.push((cost, a, f, b)); } }
        if best.is_empty() { alpha[i] = 0.5; continue; } best.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap()); let m = best.len().min(4);
        let (mut sa, mut sw) = (0.0f32, 0.0f32); for (cost, a, _, _) in &best[..m] { let wt = (-cost * 8.0).exp() + 1e-6; sa += a * wt; sw += wt; } alpha[i] = sa / sw; conf[i] = (-best[0].0 * 4.0).exp();
        // foreground colour: best pair's F
        let fc = px(best[0].2); for ch in 0..3 { fg[i * 3 + ch] = fc[ch]; } } }
    // edge-aware smoothing of the unknown region guided by the image (confidence-weighted blend with raw estimate)
    let sm = guided_filter(img, &alpha, w, h, o.guide_radius, o.eps);
    for i in 0..n { if trimap[i] == 128 { let a = sm[i].clamp(0.0, 1.0); let wgt = 0.35 + 0.65 * (1.0 - conf[i]); alpha[i] = (alpha[i] * (1.0 - wgt) + a * wgt).clamp(0.0, 1.0); } else { alpha[i] = if trimap[i] == 255 { 1.0 } else { 0.0 }; } }
    // decontaminate: F = (C - (1-a) B) / a with B from the blended bg; here use the sampled F directly where alpha is partial, original colour where opaque
    for i in 0..n { if trimap[i] == 255 || alpha[i] > 0.98 { for c in 0..3 { fg[i * 3 + c] = img[i * 3 + c]; } } }
    Matte { alpha, fg, unknown, trimap: trimap.to_vec() }
}
