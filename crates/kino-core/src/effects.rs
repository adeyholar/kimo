//! Per-pixel and spatial effects on f32 RGBA frames (premultiplied-free, 0..1).
use crate::keyframe::Prop;
use std::collections::BTreeMap;

pub struct Frame { pub w: usize, pub h: usize, pub d: Vec<f32> }
impl Frame {
    pub fn new(w: usize, h: usize) -> Self { Frame { w, h, d: vec![0.0; w * h * 4] } }
}

pub type Params = BTreeMap<String, Prop>;
fn p(m: &Params, k: &str, def: f64, t: f64) -> f32 { m.get(k).map(|x| x.at(t)).unwrap_or(def) as f32 }

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
        other => return Err(format!("unknown effect '{other}'")),
    }
    Ok(())
}

fn lum(c: [f32; 3]) -> f32 { 0.2126*c[0] + 0.7152*c[1] + 0.0722*c[2] }
fn px(f: &mut Frame, op: impl Fn([f32; 3]) -> [f32; 3]) {
    for i in 0..f.w*f.h { let o = op([f.d[i*4], f.d[i*4+1], f.d[i*4+2]]); f.d[i*4]=o[0]; f.d[i*4+1]=o[1]; f.d[i*4+2]=o[2]; }
}
fn box_blur3(f: &mut Frame, r: usize) {
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
fn rgb2hsl(c: [f32; 3]) -> (f32, f32, f32) {
    let (r, g, b) = (c[0].clamp(0.0,1.0), c[1].clamp(0.0,1.0), c[2].clamp(0.0,1.0)); let mx = r.max(g).max(b); let mn = r.min(g).min(b); let l = (mx+mn)/2.0; let d = mx-mn;
    if d < 1e-6 { return (0.0, 0.0, l); } let s = d / (1.0 - (2.0*l-1.0).abs()).max(1e-6);
    let h = if mx == r { ((g-b)/d).rem_euclid(6.0) } else if mx == g { (b-r)/d + 2.0 } else { (r-g)/d + 4.0 }; (h*60.0, s, l)
}
fn hsl2rgb(h: f32, s: f32, l: f32) -> [f32; 3] {
    let c = (1.0 - (2.0*l-1.0).abs()) * s; let x = c * (1.0 - ((h/60.0).rem_euclid(2.0) - 1.0).abs()); let m = l - c/2.0;
    let (r, g, b) = match (h/60.0) as u32 { 0 => (c,x,0.0), 1 => (x,c,0.0), 2 => (0.0,c,x), 3 => (0.0,x,c), 4 => (x,0.0,c), _ => (c,0.0,x) }; [r+m, g+m, b+m]
}
