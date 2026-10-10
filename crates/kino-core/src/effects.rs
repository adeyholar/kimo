//! Per-pixel and spatial effects on f32 RGBA frames (premultiplied-free, 0..1).
use crate::keyframe::Prop;
use std::collections::BTreeMap;

pub struct Frame { pub w: usize, pub h: usize, pub d: Vec<f32> }
impl Frame {
    pub fn new(w: usize, h: usize) -> Self { crate::guard::reserve(w.saturating_mul(h).saturating_mul(16)); Frame { w, h, d: vec![0.0; w * h * 4] } }
}

pub type Params = BTreeMap<String, Prop>;
pub(crate) fn p(m: &Params, k: &str, def: f64, t: f64) -> f32 { m.get(k).map(|x| x.at(t)).unwrap_or(def) as f32 }

/// (kind, description, [(param, default, description)]) - drives list_effects and tool schemas.
pub const CATALOG: &[(&str, &str, &[(&str, f64, &str)])] = &[
    ("exposure", "Exposure in stops", &[("stops", 0.0, "-5..5")]),
    ("contrast", "Contrast around mid grey", &[("amount", 1.0, "0..3")]),
    ("saturation", "Color saturation", &[("amount", 1.0, "0=mono, 1=unchanged, 2=vivid")]),
    ("temperature", "White balance warm/cool", &[("amount", 0.0, "-1 cool..1 warm")]),
    ("tint", "Green/magenta tint", &[("amount", 0.0, "-1..1")]),
    ("color_wheels", "Lift/gamma/gain per channel (shadows/mids/highlights)", &[
        ("lift_r",0.0,"shadows R"),("lift_g",0.0,"shadows G"),("lift_b",0.0,"shadows B"),
        ("gamma_r",1.0,"mids R"),("gamma_g",1.0,"mids G"),("gamma_b",1.0,"mids B"),
        ("gain_r",1.0,"highlights R"),("gain_g",1.0,"highlights G"),("gain_b",1.0,"highlights B")]),
    ("curves", "Luma S-curve", &[("amount", 0.0, "-1..1 (positive = more contrast S-curve)")]),
    ("teal_orange", "Hollywood teal/orange split tone", &[("amount", 0.5, "0..1")]),
    ("vignette", "Darkened edges", &[("amount", 0.5, "0..1"), ("softness", 0.5, "0..1")]),
    ("blur", "Box-approx gaussian blur", &[("radius", 4.0, "pixels")]),
    ("glow", "Bloom on highlights", &[("amount", 0.5, "0..2"), ("radius", 12.0, "pixels"), ("threshold", 0.7, "0..1")]),
    ("grain", "Film grain", &[("amount", 0.08, "0..0.5")]),
    ("rgb_split", "Chromatic aberration / glitch shift", &[("amount", 6.0, "pixels")]),
    ("invert", "Invert colors", &[]),
    ("chroma_key", "Remove a key color to alpha", &[("r",0.0,"key R"),("g",1.0,"key G"),("b",0.0,"key B"),("tolerance",0.35,"0..1"),("softness",0.1,"0..1")]),
    ("lut", "3D LUT (.cube) via strings.path", &[("intensity", 1.0, "0..1")]),
    ("hsl", "Hue/Sat/Lum, global + per color band (red,yellow,green,cyan,blue,magenta _hue/_sat/_lum)", &[("hue",0.0,"-1..1 (x60deg)"),("sat",0.0,"-1..1"),("lum",0.0,"-1..1"),("red_sat",0.0,"band"),("blue_hue",0.0,"band")]),
    ("sharpen", "Unsharp mask", &[("amount", 0.8, "0..3"), ("radius", 2.0, "px")]),
    ("pixelate", "Mosaic blocks", &[("size", 12.0, "px")]),
    ("posterize", "Reduce color levels", &[("levels", 5.0, "2..32")]),
    ("sepia", "Sepia tone", &[("amount", 1.0, "0..1")]),
    ("color_isolate", "Black & white except one hue (keep gun/scope color)", &[("hue", 190.0, "degrees 0..360"), ("range", 40.0, "degrees"), ("softness", 25.0, "degrees"), ("bw_amount", 1.0, "0..1")]),
    ("shake", "Camera shake / impact (translation jitter)", &[("amount", 8.0, "px"), ("speed", 24.0, "Hz")]),
    ("impact_shake", "Beat/impact camera shake: damped spring kicks at hit times (strings.hits=\"t1,t2,..\" clip-local seconds), x/y/rotation/zoom; measured from the SCUMBLE reference", &[("amount", 29.0, "px @720p, x kick"), ("amount_y", 9.0, "px @720p, y kick"), ("decay", 27.0, "1/s, exp decay (0.34/frame at 25fps)"), ("frequency", 12.5, "Hz, 12.5 flips sign every frame at 25fps"), ("rotation", 0.0, "deg kick"), ("zoom", 0.0, "scale kick, e.g. 0.03")]),
    ("mirror", "Mirror half of frame", &[("axis", 0.0, "0=horizontal(left->right), 1=vertical(top->bottom)")]),
    ("scanlines", "CRT scanlines", &[("amount", 0.3, "0..1"), ("spacing", 3.0, "px")]),
    ("wave", "Sine wave distortion", &[("amplitude", 10.0, "px"), ("wavelength", 80.0, "px"), ("speed", 3.0, "cycles/s")]),
    ("light_leak", "Warm animated light leak", &[("amount", 0.6, "0..1"), ("speed", 0.5, "cycles/s")]),
    ("zoom_blur", "Radial zoom blur", &[("amount", 0.06, "0..0.3")]),
    ("luma_key", "Key out by brightness (keep=0 removes darks, keep=1 removes brights)", &[("threshold", 0.2, "0..1"), ("softness", 0.1, "0..1"), ("keep_bright", 1.0, "1 keeps brights, 0 keeps darks")]),
    ("ripple", "Concentric water ripple displacement", &[("amplitude", 8.0, "px"), ("wavelength", 60.0, "px"), ("speed", 2.0, "cycles/s"), ("cx", 0.5, "0..1"), ("cy", 0.5, "0..1")]),
    ("lens_distortion", "Barrel/pincushion lens distortion", &[("amount", 0.3, "-1..1 (positive = barrel)")]),
    ("glitch_blocks", "Digital glitch: horizontal slice displacement + color shift", &[("amount", 30.0, "max px shift"), ("slices", 12.0, "count"), ("rate", 12.0, "changes/s")]),
    ("light_rays", "God rays: radial blur of highlights from a point", &[("amount", 0.8, "0..2"), ("threshold", 0.6, "0..1"), ("cx", 0.5, "0..1"), ("cy", 0.2, "0..1"), ("length", 0.4, "0..1")]),
    ("drop_shadow", "Drop shadow from alpha (use on text/shapes/stickers)", &[("dx", 8.0, "px"), ("dy", 8.0, "px"), ("blur", 8.0, "px"), ("opacity", 0.6, "0..1")]),
    ("chromatic_aberration", "Radial color fringing", &[("amount", 4.0, "px at edges")]),
    ("flicker", "Brightness flicker", &[("amount", 0.15, "0..1"), ("rate", 18.0, "Hz")]),
    ("halftone", "Halftone dots", &[("size", 8.0, "px")]),
    ("edge_glow", "Neon edge detect glow", &[("amount", 1.0, "0..3"), ("r", 0.0, "R"), ("g", 0.9, "G"), ("b", 1.0, "B")]),
    ("letterbox", "Cinematic bars", &[("ratio", 2.39, "target aspect")]),
];

pub fn apply(kind: &str, params: &Params, strings: &BTreeMap<String, String>, f: &mut Frame, t: f64, seed: u64) -> Result<(), String> {
    match kind {
        "exposure" => { let g = 2f32.powf(p(params, "stops", 0.0, t)); px(f, |c| [c[0]*g, c[1]*g, c[2]*g]); }
        "contrast" => { let a = p(params, "amount", 1.0, t); px(f, |c| [(c[0]-0.5)*a+0.5, (c[1]-0.5)*a+0.5, (c[2]-0.5)*a+0.5]); }
        "saturation" => { let a = p(params, "amount", 1.0, t); px(f, |c| { let l = lum(c); [l+(c[0]-l)*a, l+(c[1]-l)*a, l+(c[2]-l)*a] }); }
        "temperature" => { let a = p(params, "amount", 0.0, t)*0.2; px(f, |c| [c[0]+a, c[1], c[2]-a]); }
        "tint" => { let a = p(params, "amount", 0.0, t)*0.2; px(f, |c| [c[0]+a*0.5, c[1]-a, c[2]+a*0.5]); }
        "color_wheels" => {
            let g = |k: &str, d: f64| p(params, k, d, t);
            let (lr,lg,lb) = (g("lift_r",0.0),g("lift_g",0.0),g("lift_b",0.0));
            let (mr,mg,mb) = (g("gamma_r",1.0),g("gamma_g",1.0),g("gamma_b",1.0));
            let (hr,hg,hb) = (g("gain_r",1.0),g("gain_g",1.0),g("gain_b",1.0));
            px(f, |c| {
                let o = |v: f32, l: f32, m: f32, h: f32| { let v = (v*h + l*(1.0-v*h)).max(0.0); v.powf(1.0/m.max(0.01)) };
                [o(c[0],lr,mr,hr), o(c[1],lg,mg,hg), o(c[2],lb,mb,hb)]
            });
        }
        "curves" => { let a = p(params, "amount", 0.0, t); px(f, |c| { let s = |v: f32| { let v = v.clamp(0.0,1.0); let sc = v*v*(3.0-2.0*v); v + (sc - v)*a }; [s(c[0]), s(c[1]), s(c[2])] }); }
        "teal_orange" => {
            let a = p(params, "amount", 0.5, t);
            px(f, |c| { let l = lum(c).clamp(0.0, 1.0);
                // shadows -> teal, highlights -> orange
                let sh = (1.0 - l).powi(2) * a * 0.12; let hi = l.powi(2) * a * 0.14;
                [c[0] - sh*1.0 + hi*1.0, c[1] + sh*0.35 + hi*0.25, c[2] + sh*0.9 - hi*1.0] });
        }
        "vignette" => {
            let a = p(params, "amount", 0.5, t); let s = p(params, "softness", 0.5, t).max(0.05);
            let (w, h) = (f.w as f32, f.h as f32);
            for y in 0..f.h { for x in 0..f.w {
                let dx = (x as f32 / w - 0.5) * 2.0; let dy = (y as f32 / h - 0.5) * 2.0;
                let r = (dx*dx + dy*dy).sqrt() / 1.414;
                let v = 1.0 - a * ((r - (1.0 - s)).max(0.0) / s).powi(2).min(1.0);
                let i = (y*f.w+x)*4; f.d[i]*=v; f.d[i+1]*=v; f.d[i+2]*=v; } }
        }
        "blur" => { let r = p(params, "radius", 4.0, t) as usize; box_blur3(f, r); }
        "glow" => {
            let a = p(params, "amount", 0.5, t); let r = p(params, "radius", 12.0, t) as usize; let th = p(params, "threshold", 0.7, t);
            let mut b = Frame { w: f.w, h: f.h, d: f.d.clone() };
            for i in 0..f.w*f.h { let c = [b.d[i*4],b.d[i*4+1],b.d[i*4+2]]; let l = lum(c);
                let k = ((l - th)/(1.0-th).max(0.01)).clamp(0.0,1.0); for j in 0..3 { b.d[i*4+j] *= k; } }
            box_blur3(&mut b, r);
            for i in 0..f.w*f.h { for j in 0..3 { f.d[i*4+j] += b.d[i*4+j]*a; } }
        }
        "grain" => {
            let a = p(params, "amount", 0.08, t); let mut s = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            for i in 0..f.w*f.h { s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let n = ((s >> 40) as f32 / 16777216.0 - 0.5) * a; for j in 0..3 { f.d[i*4+j] += n; } }
        }
        "rgb_split" => {
            let a = p(params, "amount", 6.0, t) as isize; let src = f.d.clone(); let w = f.w as isize;
            for y in 0..f.h { for x in 0..f.w { let i = (y*f.w+x)*4;
                let xr = (x as isize + a).clamp(0, w-1) as usize; let xb = (x as isize - a).clamp(0, w-1) as usize;
                f.d[i] = src[(y*f.w+xr)*4]; f.d[i+2] = src[(y*f.w+xb)*4+2]; } }
        }
        "invert" => px(f, |c| [1.0-c[0], 1.0-c[1], 1.0-c[2]]),
        "chroma_key" => {
            let k = [p(params,"r",0.0,t), p(params,"g",1.0,t), p(params,"b",0.0,t)];
            let tol = p(params,"tolerance",0.35,t); let soft = p(params,"softness",0.1,t).max(0.001);
            for i in 0..f.w*f.h { let c = &mut f.d[i*4..i*4+4];
                let dist = ((c[0]-k[0]).powi(2)+(c[1]-k[1]).powi(2)+(c[2]-k[2]).powi(2)).sqrt()/1.732;
                let al = ((dist - tol)/soft).clamp(0.0,1.0); c[3] *= al;
                if al < 1.0 && c[1] > c[0].max(c[2]) { c[1] = c[0].max(c[2]); } } // despill
        }
        "letterbox" => {
            let ratio = p(params, "ratio", 2.39, t); let vis_h = ((f.w as f32 / ratio) as usize).min(f.h);
            let bar = (f.h - vis_h)/2;
            for y in 0..f.h { if y < bar || y >= f.h - bar { for x in 0..f.w { let i=(y*f.w+x)*4; f.d[i]=0.0; f.d[i+1]=0.0; f.d[i+2]=0.0; f.d[i+3]=1.0; } } }
        }
        "lut" => { let path = strings.get("path").ok_or("lut needs strings.path (.cube)")?; let l = load_lut(path)?; let a = p(params, "intensity", 1.0, t);
            px(f, |c| { let o = l.sample(c); [c[0]+(o[0]-c[0])*a, c[1]+(o[1]-c[1])*a, c[2]+(o[2]-c[2])*a] }); }
        "hsl" => {
            let g = |k: &str| p(params, k, 0.0, t);
            const BANDS: [(&str, f32); 6] = [("red",0.0),("yellow",60.0),("green",120.0),("cyan",180.0),("blue",240.0),("magenta",300.0)];
            px(f, |c| { let (mut h, mut s, mut l) = rgb2hsl(c);
                let (mut dh, mut ds, mut dl) = (g("hue"), g("sat"), g("lum"));
                for (n, c0) in BANDS { let mut d = (h - c0).abs(); if d > 180.0 { d = 360.0 - d; } let w = (1.0 - d/60.0).max(0.0);
                    if w > 0.0 { dh += g(&format!("{n}_hue"))*w; ds += g(&format!("{n}_sat"))*w; dl += g(&format!("{n}_lum"))*w; } }
                h = (h + dh*60.0).rem_euclid(360.0); s = (s*(1.0+ds)).clamp(0.0,1.0); l = (l + dl*0.5).clamp(0.0,1.0); hsl2rgb(h, s, l) });
        }
        "sharpen" => { let a = p(params, "amount", 0.8, t); let r = p(params, "radius", 2.0, t) as usize; let mut b = Frame { w: f.w, h: f.h, d: f.d.clone() }; box_blur3(&mut b, r);
            for i in 0..f.w*f.h { for j in 0..3 { f.d[i*4+j] += (f.d[i*4+j] - b.d[i*4+j]) * a; } } }
        "pixelate" => { let s = (p(params, "size", 12.0, t) as usize).max(1); let src = f.d.clone();
            for y in 0..f.h { for x in 0..f.w { let (sx, sy) = ((x / s * s + s / 2).min(f.w-1), (y / s * s + s / 2).min(f.h-1)); for j in 0..4 { f.d[(y*f.w+x)*4+j] = src[(sy*f.w+sx)*4+j]; } } } }
        "posterize" => { let l = p(params, "levels", 5.0, t).max(2.0); px(f, |c| [(c[0].clamp(0.0,1.0)*l).floor()/(l-1.0), (c[1].clamp(0.0,1.0)*l).floor()/(l-1.0), (c[2].clamp(0.0,1.0)*l).floor()/(l-1.0)]); }
        "sepia" => { let a = p(params, "amount", 1.0, t); px(f, |c| { let l = lum(c); let s = [l*1.07, l*0.74, l*0.43]; [c[0]+(s[0]-c[0])*a, c[1]+(s[1]-c[1])*a, c[2]+(s[2]-c[2])*a] }); }
        "color_isolate" => { let (h0, rg, so, bw) = (p(params, "hue", 190.0, t), p(params, "range", 40.0, t), p(params, "softness", 25.0, t).max(1.0), p(params, "bw_amount", 1.0, t));
            px(f, |c| { let (h, s, _) = rgb2hsl(c); let mut d = (h - h0).abs(); if d > 180.0 { d = 360.0 - d; } let keep = (1.0 - ((d - rg) / so).clamp(0.0, 1.0)) * (s * 2.0).min(1.0); let l = lum(c); let k = keep.max(1.0 - bw);
                [l + (c[0]-l)*k, l + (c[1]-l)*k, l + (c[2]-l)*k] }); }
        "shake" => { let a = p(params, "amount", 8.0, t); let sp = p(params, "speed", 24.0, t); let n = |k: f32| (((k * 12.9898).sin() * 43758.545).fract()) * 2.0 - 1.0; let step = (t as f32 * sp).floor();
            let (dx, dy) = ((n(step) * a) as isize, (n(step + 7.0) * a) as isize); let src = f.d.clone(); let (w, h) = (f.w as isize, f.h as isize);
            for y in 0..h { for x in 0..w { let (sx, sy) = ((x - dx).clamp(0, w-1), (y - dy).clamp(0, h-1)); for j in 0..4 { f.d[((y*w+x)*4) as usize+j] = src[((sy*w+sx)*4) as usize+j]; } } } }
        "impact_shake" => {
            let hits: Vec<f64> = strings.get("hits").map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect()).unwrap_or_default();
            let k = f.h as f32 / 720.0; let (ax, ay) = (p(params, "amount", 29.0, t) * k, p(params, "amount_y", 9.0, t) * k); let (dec, fr) = (p(params, "decay", 27.0, t), p(params, "frequency", 12.5, t));
            let (rot, zm) = (p(params, "rotation", 0.0, t), p(params, "zoom", 0.0, t));
            let (mut dx, mut dy, mut dr, mut dz) = (0f32, 0f32, 0f32, 0f32);
            if let Some(pr) = strings.get("profile") { // measured per-frame kick table "dx,dy;dx,dy;..." (pixels @720p), discrete frames like the reference
                let tab: Vec<(f32, f32)> = pr.split(';').filter_map(|s| { let mut it = s.split(',').filter_map(|x| x.trim().parse::<f32>().ok()); Some((it.next()?, it.next()?)) }).collect();
                let fps: f64 = strings.get("fps").and_then(|s| s.parse().ok()).unwrap_or(25.0); let sc = p(params, "scale", 1.0, t);
                for h in hits.iter() { let fi = ((t - h) * fps + 1e-4).floor(); if fi >= 0.0 && (fi as usize) < tab.len() { let (a, b) = tab[fi as usize]; dx += a * k * sc; dy += b * k * sc; } }
            } else {
            for (n, h) in hits.iter().enumerate() { let dt = (t - h) as f32; if dt < 0.0 || dt > 1.5 { continue; }
                let env = (-dec * dt).exp(); let osc = (std::f32::consts::TAU * fr * dt).cos(); let sg: f32 = if n % 2 == 0 { 1.0 } else { -1.0 } * if (n / 2) % 2 == 0 { 1.0 } else { 0.7 };
                dx += ax * env * osc; dy -= ay * env; dr += rot * env * osc * sg; dz += zm * env; }
            }
            if dx.abs() < 1e-3 && dy.abs() < 1e-3 && dr.abs() < 1e-4 && dz.abs() < 1e-5 { return Ok(()); }
            let src = f.d.clone(); let (w, h) = (f.w, f.h); let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0); let (s_, c_) = (dr.to_radians().sin(), dr.to_radians().cos()); let inv = 1.0 / (1.0 + dz);
            for y in 0..h { for x in 0..w { let (px, py) = (x as f32 - cx - dx, y as f32 - cy - dy); let sx = ((px * c_ + py * s_) * inv + cx).clamp(0.0, w as f32 - 1.0); let sy = ((-px * s_ + py * c_) * inv + cy).clamp(0.0, h as f32 - 1.0);
                let (x0, y0) = (sx.floor() as usize, sy.floor() as usize); let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1)); let (fx, fy) = (sx - x0 as f32, sy - y0 as f32);
                for j in 0..4 { let g = |xx: usize, yy: usize| src[(yy * w + xx) * 4 + j]; f.d[(y * w + x) * 4 + j] = g(x0, y0) * (1.0 - fx) * (1.0 - fy) + g(x1, y0) * fx * (1.0 - fy) + g(x0, y1) * (1.0 - fx) * fy + g(x1, y1) * fx * fy; } } } }
        "mirror" => { let v = p(params, "axis", 0.0, t) > 0.5; for y in 0..f.h { for x in 0..f.w { let (sx, sy) = if v { (x, if y >= f.h/2 { f.h-1-y } else { y }) } else { (if x >= f.w/2 { f.w-1-x } else { x }, y) };
            for j in 0..4 { f.d[(y*f.w+x)*4+j] = f.d[(sy*f.w+sx)*4+j]; } } } }
        "scanlines" => { let (a, s) = (p(params, "amount", 0.3, t), (p(params, "spacing", 3.0, t) as usize).max(2)); for y in 0..f.h { if y % s == 0 { for x in 0..f.w { for j in 0..3 { f.d[(y*f.w+x)*4+j] *= 1.0 - a; } } } } }
        "wave" => { let (a, wl, sp) = (p(params, "amplitude", 10.0, t), p(params, "wavelength", 80.0, t), p(params, "speed", 3.0, t)); let src = f.d.clone();
            for y in 0..f.h { let off = (a * ((y as f32 / wl + t as f32 * sp) * 6.2831853).sin()) as isize; for x in 0..f.w { let sx = (x as isize + off).clamp(0, f.w as isize - 1) as usize; for j in 0..4 { f.d[(y*f.w+x)*4+j] = src[(y*f.w+sx)*4+j]; } } } }
        "light_leak" => { let (a, sp) = (p(params, "amount", 0.6, t), p(params, "speed", 0.5, t)); let ph = t as f32 * sp * 6.2831853; let (w, h) = (f.w as f32, f.h as f32);
            for y in 0..f.h { for x in 0..f.w { let (u, v) = (x as f32 / w, y as f32 / h); let cx = 0.5 + 0.5 * ph.sin(); let d = ((u - cx).powi(2) * 2.0 + (v - 0.1).powi(2)).sqrt(); let k = (1.0 - d * 1.8).max(0.0).powi(2) * a * (0.7 + 0.3 * (ph * 2.0).sin());
                let i = (y*f.w+x)*4; f.d[i] += k * 1.0; f.d[i+1] += k * 0.45; f.d[i+2] += k * 0.12; } } }
        "zoom_blur" => { let a = p(params, "amount", 0.06, t); let src = f.d.clone(); let (w, h) = (f.w as f32, f.h as f32); let n = 8;
            for y in 0..f.h { for x in 0..f.w { let mut acc = [0f32; 4]; for k in 0..n { let s = 1.0 - a * k as f32 / n as f32; let sx = ((x as f32 - w/2.0) * s + w/2.0).clamp(0.0, w-1.0) as usize; let sy = ((y as f32 - h/2.0) * s + h/2.0).clamp(0.0, h-1.0) as usize; for j in 0..4 { acc[j] += src[(sy*f.w+sx)*4+j]; } }
                for j in 0..4 { f.d[(y*f.w+x)*4+j] = acc[j] / n as f32; } } } }
        "luma_key" => { let (th, so, kb) = (p(params, "threshold", 0.2, t), p(params, "softness", 0.1, t).max(0.001), p(params, "keep_bright", 1.0, t) > 0.5);
            for i in 0..f.w*f.h { let l = lum([f.d[i*4], f.d[i*4+1], f.d[i*4+2]]); let k = ((l - th) / so).clamp(0.0, 1.0); f.d[i*4+3] *= if kb { k } else { 1.0 - k }; } }
        "ripple" => { let (a, wl, sp, cx, cy) = (p(params, "amplitude", 8.0, t), p(params, "wavelength", 60.0, t), p(params, "speed", 2.0, t), p(params, "cx", 0.5, t) * f.w as f32, p(params, "cy", 0.5, t) * f.h as f32); let src = f.d.clone(); let (w, h) = (f.w, f.h);
            for y in 0..h { for x in 0..w { let (dx, dy) = (x as f32 - cx, y as f32 - cy); let d = (dx*dx + dy*dy).sqrt().max(1.0); let o = a * ((d / wl - t as f32 * sp) * 6.2831853).sin() * (-d / (w as f32)).exp();
                let sx = (x as f32 + dx / d * o).clamp(0.0, w as f32 - 1.0) as usize; let sy = (y as f32 + dy / d * o).clamp(0.0, h as f32 - 1.0) as usize; for j in 0..4 { f.d[(y*w+x)*4+j] = src[(sy*w+sx)*4+j]; } } } }
        "lens_distortion" => { let a = p(params, "amount", 0.3, t); let src = f.d.clone(); let (w, h) = (f.w as f32, f.h as f32);
            for y in 0..f.h { for x in 0..f.w { let (nx, ny) = ((x as f32 / w - 0.5) * 2.0, (y as f32 / h - 0.5) * 2.0); let r2 = nx*nx + ny*ny; let k = 1.0 + a * r2; let (sx, sy) = (((nx * k) * 0.5 + 0.5) * w, ((ny * k) * 0.5 + 0.5) * h);
                let o = (y*f.w+x)*4; if sx < 0.0 || sy < 0.0 || sx >= w || sy >= h { f.d[o..o+4].copy_from_slice(&[0.0, 0.0, 0.0, 1.0]); continue; } let s = (sy as usize * f.w + sx as usize) * 4; for j in 0..4 { f.d[o+j] = src[s+j]; } } } }
        "glitch_blocks" => { let (a, n, rate) = (p(params, "amount", 30.0, t), p(params, "slices", 12.0, t).max(1.0) as usize, p(params, "rate", 12.0, t)); let step = (t as f32 * rate).floor(); let src = f.d.clone(); let (w, h) = (f.w, f.h); let sh = (h / n).max(1);
            let rnd = |k: f32| (((k * 12.9898).sin() * 43758.545).fract()).abs();
            for y in 0..h { let sl = (y / sh) as f32; let r = rnd(sl + step * 17.0); let off = if r > 0.55 { ((rnd(sl * 3.1 + step) * 2.0 - 1.0) * a) as isize } else { 0 };
                for x in 0..w { let sx = (x as isize + off).rem_euclid(w as isize) as usize; let o = (y*w+x)*4; f.d[o] = src[(y*w+sx)*4]; f.d[o+1] = src[(y*w+((sx as isize + off.signum() * 4).rem_euclid(w as isize) as usize))*4+1]; f.d[o+2] = src[(y*w+sx)*4+2]; f.d[o+3] = src[(y*w+sx)*4+3]; } } }
        "light_rays" => { let (a, th, cx, cy, len) = (p(params, "amount", 0.8, t), p(params, "threshold", 0.6, t), p(params, "cx", 0.5, t), p(params, "cy", 0.2, t), p(params, "length", 0.4, t)); let (w, h) = (f.w as f32, f.h as f32); let n = 24; let src = f.d.clone();
            for y in 0..f.h { for x in 0..f.w { let mut acc = [0f32; 3]; for k in 0..n { let s = 1.0 - len * k as f32 / n as f32; let sx = ((x as f32 - cx*w) * s + cx*w).clamp(0.0, w - 1.0) as usize; let sy = ((y as f32 - cy*h) * s + cy*h).clamp(0.0, h - 1.0) as usize; let i = (sy*f.w+sx)*4;
                let l = lum([src[i], src[i+1], src[i+2]]); let m = ((l - th) / (1.0 - th).max(0.01)).clamp(0.0, 1.0) * (1.0 - k as f32 / n as f32); for j in 0..3 { acc[j] += src[i+j] * m; } }
                let o = (y*f.w+x)*4; for j in 0..3 { f.d[o+j] += acc[j] / n as f32 * a * 2.0; } } } }
        "drop_shadow" => { let (dx, dy, bl, op) = (p(params, "dx", 8.0, t) as isize, p(params, "dy", 8.0, t) as isize, p(params, "blur", 8.0, t) as usize, p(params, "opacity", 0.6, t)); let mut sh = Frame::new(f.w, f.h); let (w, h) = (f.w as isize, f.h as isize);
            for y in 0..h { for x in 0..w { let (sx, sy) = (x - dx, y - dy); if sx >= 0 && sy >= 0 && sx < w && sy < h { sh.d[((y*w+x)*4+3) as usize] = f.d[((sy*w+sx)*4+3) as usize]; } } } box_blur3(&mut sh, bl);
            for i in 0..f.w*f.h { let a = sh.d[i*4+3] * op; let o = &mut f.d[i*4..i*4+4]; let na = o[3] + a * (1.0 - o[3]); if na > 0.0 { for j in 0..3 { o[j] = (o[j] * o[3]) / na; } } o[3] = na; } }
        "chromatic_aberration" => { let a = p(params, "amount", 4.0, t); let src = f.d.clone(); let (w, h) = (f.w as f32, f.h as f32);
            for y in 0..f.h { for x in 0..f.w { let (nx, ny) = (x as f32 / w - 0.5, y as f32 / h - 0.5); let o = (y*f.w+x)*4;
                for (j, s) in [(0usize, 1.0f32), (2usize, -1.0f32)] { let sx = (x as f32 + nx * a * 2.0 * s).clamp(0.0, w - 1.0) as usize; let sy = (y as f32 + ny * a * 2.0 * s).clamp(0.0, h - 1.0) as usize; f.d[o+j] = src[(sy*f.w+sx)*4+j]; } } } }
        "flicker" => { let (a, r) = (p(params, "amount", 0.15, t), p(params, "rate", 18.0, t)); let k = 1.0 - a * (((t as f32 * r).floor() * 78.233).sin() * 43758.545).fract().abs(); px(f, |c| [c[0]*k, c[1]*k, c[2]*k]); }
        "halftone" => { let s = p(params, "size", 8.0, t).max(2.0); let src = f.d.clone();
            for y in 0..f.h { for x in 0..f.w { let (cx, cy) = (((x as f32 / s).floor() + 0.5) * s, ((y as f32 / s).floor() + 0.5) * s); let (sx, sy) = ((cx as usize).min(f.w-1), (cy as usize).min(f.h-1)); let i = (sy*f.w+sx)*4; let l = lum([src[i], src[i+1], src[i+2]]).clamp(0.0, 1.0);
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt(); let r = s * 0.7 * l.sqrt(); let k = (r - d + 0.5).clamp(0.0, 1.0); let o = (y*f.w+x)*4; for j in 0..3 { f.d[o+j] = src[i+j] * k; } } } }
        "edge_glow" => { let (a, col) = (p(params, "amount", 1.0, t), [p(params, "r", 0.0, t), p(params, "g", 0.9, t), p(params, "b", 1.0, t)]); let src = f.d.clone(); let (w, h) = (f.w, f.h);
            let mut ed = Frame::new(w, h); for y in 1..h-1 { for x in 1..w-1 { let g = |xx: usize, yy: usize| { let i = (yy*w+xx)*4; lum([src[i], src[i+1], src[i+2]]) };
                let gx = g(x+1, y) - g(x-1, y); let gy = g(x, y+1) - g(x, y-1); let m = ((gx*gx + gy*gy).sqrt() * 3.0).min(1.0); let o = (y*w+x)*4; ed.d[o] = col[0]*m; ed.d[o+1] = col[1]*m; ed.d[o+2] = col[2]*m; ed.d[o+3] = 1.0; } }
            let mut bl = Frame { w, h, d: ed.d.clone() }; box_blur3(&mut bl, 6); for i in 0..w*h { for j in 0..3 { f.d[i*4+j] = f.d[i*4+j] * 0.35 + (ed.d[i*4+j] + bl.d[i*4+j]) * a; } } }
        other => return crate::effects2::apply2(other, params, f, t, seed),
    }
    Ok(())
}

pub(crate) fn lum(c: [f32; 3]) -> f32 { 0.2126*c[0] + 0.7152*c[1] + 0.0722*c[2] }
pub(crate) fn px(f: &mut Frame, op: impl Fn([f32; 3]) -> [f32; 3]) {
    for i in 0..f.w*f.h { let o = op([f.d[i*4], f.d[i*4+1], f.d[i*4+2]]); f.d[i*4]=o[0]; f.d[i*4+1]=o[1]; f.d[i*4+2]=o[2]; }
}
pub(crate) fn box_blur3(f: &mut Frame, r: usize) {
    if r == 0 { return; }
    let r = (r / 2).max(1);
    for _ in 0..3 { blur_axis(f, r, true); blur_axis(f, r, false); }
}
fn blur_axis(f: &mut Frame, r: usize, horiz: bool) {
    let (w, h) = (f.w, f.h); let src = f.d.clone();
    let (len, lines) = if horiz { (w, h) } else { (h, w) };
    for l in 0..lines {
        let idx = |i: usize| if horiz { (l*w+i)*4 } else { (i*w+l)*4 };
        let mut acc = [0f32; 4]; let win = (2*r+1) as f32;
        for i in 0..=r.min(len-1) { for j in 0..4 { acc[j] += src[idx(i)+j]; } }
        for j in 0..4 { acc[j] += src[idx(0)+j] * r as f32; }
        for i in 0..len {
            let o = idx(i); for j in 0..4 { f.d[o+j] = acc[j]/win; }
            let add = src[idx((i+r+1).min(len-1))..][..4].to_vec(); let sub = src[idx(i.saturating_sub(r))..][..4].to_vec();
            for j in 0..4 { acc[j] += add[j] - sub[j]; }
        }
    }
}

pub struct Lut { n: usize, d: Vec<[f32; 3]> }
impl Lut { pub fn sample(&self, c: [f32; 3]) -> [f32; 3] {
    let m = (self.n - 1) as f32; let f = |v: f32| v.clamp(0.0, 1.0) * m; let (r, g, b) = (f(c[0]), f(c[1]), f(c[2]));
    let (r0, g0, b0) = (r.floor() as usize, g.floor() as usize, b.floor() as usize); let (r1, g1, b1) = ((r0+1).min(self.n-1), (g0+1).min(self.n-1), (b0+1).min(self.n-1));
    let (fr, fg, fb) = (r-r0 as f32, g-g0 as f32, b-b0 as f32);
    let at = |r: usize, g: usize, b: usize| self.d[r + g*self.n + b*self.n*self.n];
    let mut o = [0f32; 3];
    for j in 0..3 { let c00 = at(r0,g0,b0)[j]*(1.0-fr)+at(r1,g0,b0)[j]*fr; let c10 = at(r0,g1,b0)[j]*(1.0-fr)+at(r1,g1,b0)[j]*fr;
        let c01 = at(r0,g0,b1)[j]*(1.0-fr)+at(r1,g0,b1)[j]*fr; let c11 = at(r0,g1,b1)[j]*(1.0-fr)+at(r1,g1,b1)[j]*fr;
        o[j] = (c00*(1.0-fg)+c10*fg)*(1.0-fb) + (c01*(1.0-fg)+c11*fg)*fb; }
    o } }
static LUTS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<Lut>>>> = std::sync::OnceLock::new();
fn load_lut(path: &str) -> Result<std::sync::Arc<Lut>, String> {
    let m = LUTS.get_or_init(|| Default::default()); let mut g = m.lock().unwrap(); if let Some(l) = g.get(path) { return Ok(l.clone()); }
    let t = std::fs::read_to_string(path).map_err(|e| format!("lut {path}: {e}"))?; let mut n = 0; let mut d = vec![];
    for l in t.lines() { let l = l.trim(); if l.starts_with("LUT_3D_SIZE") { n = l.split_whitespace().nth(1).and_then(|x| x.parse().ok()).unwrap_or(0); }
        else if l.chars().next().map(|c| c.is_ascii_digit() || c == '-' || c == '.').unwrap_or(false) { let v: Vec<f32> = l.split_whitespace().filter_map(|x| x.parse().ok()).collect(); if v.len() == 3 { d.push([v[0], v[1], v[2]]); } } }
    if n == 0 || d.len() != n*n*n { return Err("bad .cube (need LUT_3D_SIZE and n^3 rows)".into()); }
    let l = std::sync::Arc::new(Lut { n, d }); g.insert(path.into(), l.clone()); Ok(l)
}
pub(crate) fn rgb2hsl(c: [f32; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (c[0].clamp(0.0,1.0), c[1].clamp(0.0,1.0), c[2].clamp(0.0,1.0)); let mx = r.max(g).max(b); let mn = r.min(g).min(b); let l = (mx+mn)/2.0; let d = mx-mn;
    if d < 1e-6 { return (0.0, 0.0, l); } let s = d / (1.0 - (2.0*l-1.0).abs()).max(1e-6);
    let h = if mx == r { ((g-b)/d).rem_euclid(6.0) } else if mx == g { (b-r)/d + 2.0 } else { (r-g)/d + 4.0 }; (h*60.0, s, l)
}
pub(crate) fn hsl2rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let c = (1.0 - (2.0*l-1.0).abs()) * s; let x = c * (1.0 - ((h/60.0).rem_euclid(2.0) - 1.0).abs()); let m = l - c/2.0;
    let (r, g, b) = match (h/60.0) as u32 { 0 => (c,x,0.0), 1 => (x,c,0.0), 2 => (0.0,c,x), 3 => (0.0,x,c), 4 => (x,0.0,c), _ => (c,0.0,x) }; [r+m, g+m, b+m]
}
