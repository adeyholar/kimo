//! light_wrap: backdrop light bleeding over a layer's edges. wrap = blur(backdrop) * edge, edge = blur(1 - alpha, width) so it is
//! strongest at the silhouette and fades inward; added to the layer's own colour where the layer is opaque.
use crate::effects::{box_blur3, p, Frame, Params};

pub fn apply(pr: &Params, f: &mut Frame, t: f64) -> Result<(), String> {
    let g = |k: &str, d: f64| p(pr, k, d, t);
    let (w, h) = (f.w, f.h);
    let bd = crate::render::backdrop().filter(|b| b.w == w && b.h == h).ok_or("light_wrap needs a layer below it (nothing to wrap)")?;
    let width = g("width", 24.0).clamp(1.0, 300.0) as usize; let inten = g("intensity", 1.0).clamp(0.0, 4.0) as f32; let blur = g("blur", 20.0).clamp(0.0, 200.0) as usize; let desat = g("desaturate", 0.0).clamp(0.0, 1.0) as f32;
    let mut col = Frame { w, h, d: bd.d.clone() }; box_blur3(&mut col, blur);
    let mut inv = Frame { w, h, d: vec![0.0; w * h * 4] }; for i in 0..w * h { inv.d[i * 4] = 1.0 - f.d[i * 4 + 3]; inv.d[i * 4 + 3] = 1.0; } box_blur3(&mut inv, width);
    for i in 0..w * h { let a = f.d[i * 4 + 3]; if a <= 0.0 { continue; } let edge = inv.d[i * 4].clamp(0.0, 1.0) * inten;
        let (r, gc, b) = (col.d[i * 4], col.d[i * 4 + 1], col.d[i * 4 + 2]); let l = 0.2126 * r + 0.7152 * gc + 0.0722 * b;
        for (j, v) in [r, gc, b].into_iter().enumerate() { let wv = l + (v - l) * (1.0 - desat); f.d[i * 4 + j] += wv * edge * a; } }
    Ok(())
}
