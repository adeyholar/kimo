//! frosted_glass: a glass pane whose shape is the layer's alpha. Inside the pane the layer shows
//! the composite below it (the backdrop) blurred, tinted and refracted at the pane edge.
use crate::effects::{box_blur3, p, Frame, Params};

fn sample(src: &[f32], w: usize, h: usize, x: f32, y: f32, ch: usize) -> f32 {
    let x = x.clamp(0.0, (w - 1) as f32); let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x as usize, y as usize); let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1)); let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |xx: usize, yy: usize| src[(yy * w + xx) * 4 + ch];
    (at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx) * (1.0 - fy) + (at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx) * fy
}

thread_local! { static POST: std::cell::Cell<bool> = Default::default(); }
/// True only while the renderer runs frosted_glass after the layer transform.
pub fn in_post() -> bool { POST.with(|c| c.get()) }
pub fn set_post(v: bool) { POST.with(|c| c.set(v)) }

pub fn apply(pr: &Params, f: &mut Frame, t: f64) -> Result<(), String> {
    let g = |k: &str, d: f64| p(pr, k, d, t);
    let bd = crate::render::backdrop().filter(|b| b.w == f.w && b.h == f.h).ok_or("frosted_glass needs a layer below it (nothing to frost)")?;
    let (w, h) = (f.w, f.h);
    let blur = g("blur", 24.0).clamp(0.0, 200.0) as usize; let refr = g("refraction", 14.0).clamp(0.0, 80.0) as f32; let disp = g("dispersion", 0.25).clamp(0.0, 1.5) as f32;
    let bevel = g("bevel", 10.0).clamp(1.0, 80.0) as usize; let tint = [g("tint_r", 1.0) as f32, g("tint_g", 1.0) as f32, g("tint_b", 1.0) as f32]; let tamt = g("tint_amount", 0.12).clamp(0.0, 1.0) as f32;
    let sat = g("saturation", 1.15).clamp(0.0, 3.0) as f32; let rim = g("edge_light", 0.5).clamp(0.0, 2.0) as f32; let mix = g("layer_mix", 0.0).clamp(0.0, 1.0) as f32;
    let mut frost = Frame { w, h, d: bd.d.clone() }; box_blur3(&mut frost, blur);
    // smooth alpha -> height field; its gradient is the bevel normal, nonzero only near the pane edge
    let mut hf = Frame { w, h, d: vec![0.0; w * h * 4] }; for i in 0..w * h { hf.d[i * 4] = f.d[i * 4 + 3]; hf.d[i * 4 + 3] = 1.0; } box_blur3(&mut hf, bevel * 2);
    let hh = |x: usize, y: usize| hf.d[(y.min(h - 1) * w + x.min(w - 1)) * 4];
    let src_lum = |i: usize| 0.2126 * f.d[i * 4] + 0.7152 * f.d[i * 4 + 1] + 0.0722 * f.d[i * 4 + 2]; let _ = src_lum;
    let mut out = vec![0f32; w * h * 4];
    for y in 0..h { for x in 0..w { let i = y * w + x; let a = f.d[i * 4 + 3]; if a <= 0.0 { continue; }
        let gx = hh(x + 1, y) - hh(x.saturating_sub(1), y); let gy = hh(x, y + 1) - hh(x, y.saturating_sub(1)); let gm = (gx * gx + gy * gy).sqrt();
        let (ox, oy) = (-gx * refr * bevel as f32 * 0.5, -gy * refr * bevel as f32 * 0.5);
        let mut c = [0f32; 3]; for (j, cj) in c.iter_mut().enumerate() { let s = 1.0 + (j as f32 - 1.0) * disp * 0.5; *cj = sample(&frost.d, w, h, x as f32 + ox * s, y as f32 + oy * s, j); }
        let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]; for cj in c.iter_mut() { *cj = l + (*cj - l) * sat; }
        for j in 0..3 { c[j] = c[j] * (1.0 - tamt) + tint[j] * tamt; }
        // rim light: bright where the bevel normal faces up-left, darker down-right
        let lit = (-(gx * 0.7 + gy * 0.7) / gm.max(1e-6)) * (gm * bevel as f32 * 2.0).min(1.0);
        for j in 0..3 { c[j] = (c[j] + lit * rim * 0.35).max(0.0); c[j] = c[j] * (1.0 - mix) + f.d[i * 4 + j] * mix; out[i * 4 + j] = c[j]; }
        out[i * 4 + 3] = a; } }
    f.d = out; Ok(())
}
