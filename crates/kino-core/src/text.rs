use crate::effects::Frame;
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static FONTS: OnceLock<Mutex<HashMap<String, std::sync::Arc<FontVec>>>> = OnceLock::new();
fn font(path: &str) -> Result<std::sync::Arc<FontVec>, String> {
    let m = FONTS.get_or_init(|| Mutex::new(HashMap::new())); let mut g = m.lock().unwrap();
    if let Some(f) = g.get(path) { return Ok(f.clone()); }
    let data = std::fs::read(path).map_err(|e| format!("font {path}: {e}"))?;
    let f = std::sync::Arc::new(FontVec::try_from_vec(data).map_err(|e| format!("font parse {path}: {e}"))?);
    g.insert(path.into(), f.clone()); Ok(f)
}

/// Coverage mask (w*h) of `text` laid out on the canvas, centered, with `reveal` 0..1 of characters visible.
fn coverage(cw: usize, ch: usize, text: &str, f: &FontVec, size: f32, spacing: f32, align: &str, reveal: f64, ox: f32, oy: f32) -> Vec<f32> {
    let sf = f.as_scaled(PxScale::from(size)); let mut cov = vec![0f32; cw * ch];
    let lines: Vec<&str> = text.split('\n').collect(); let lh = sf.height() * 1.15;
    let total_chars: usize = lines.iter().map(|l| l.chars().count()).sum(); let mut shown = (reveal.clamp(0.0, 1.0) * total_chars as f64).ceil() as usize;
    let block_h = lh * lines.len() as f32; let mut y0 = (ch as f32 - block_h) / 2.0 + sf.ascent() + oy;
    for line in lines {
        let w: f32 = line.chars().map(|c| sf.h_advance(f.glyph_id(c)) + spacing).sum::<f32>() - spacing;
        let mut x = match align { "left" => cw as f32 * 0.08, "right" => cw as f32 * 0.92 - w, _ => (cw as f32 - w) / 2.0 } + ox;
        for c in line.chars() {
            let gid = f.glyph_id(c); let adv = sf.h_advance(gid) + spacing;
            if shown == 0 { break; } shown -= 1;
            let g = gid.with_scale_and_position(PxScale::from(size), ab_glyph::point(x, y0));
            if let Some(o) = f.outline_glyph(g) { let b = o.px_bounds();
                o.draw(|px, py, v| { let X = b.min.x as i32 + px as i32; let Y = b.min.y as i32 + py as i32; if X >= 0 && Y >= 0 && (X as usize) < cw && (Y as usize) < ch { let i = Y as usize * cw + X as usize; cov[i] = cov[i].max(v); } }); }
            x += adv;
        }
        y0 += lh;
    }
    cov
}

pub fn render_text(fr: &mut Frame, text: &str, fontpath: &str, size: f64, col: [f64; 3], spacing: f64, stroke_w: f64, stroke_col: [f64; 3], shadow: f64, align: &str, reveal: f64) -> Result<(), String> {
    let f = font(fontpath)?; let (w, h) = (fr.w, fr.h);
    let main = coverage(w, h, text, &f, size as f32, spacing as f32, align, reveal, 0.0, 0.0);
    let over = |fr: &mut Frame, cov: &[f32], c: [f32; 3]| { for i in 0..w*h { let a = cov[i]; if a <= 0.0 { continue; } let o = &mut fr.d[i*4..i*4+4]; let na = a + o[3]*(1.0-a);
        for j in 0..3 { o[j] = (c[j]*a + o[j]*o[3]*(1.0-a)) / na.max(1e-6); } o[3] = na; } };
    if shadow > 0.0 { let sc = coverage(w, h, text, &f, size as f32, spacing as f32, align, reveal, shadow as f32, shadow as f32); let sc: Vec<f32> = sc.iter().map(|v| v * 0.6).collect(); over(fr, &sc, [0.0; 3]); }
    if stroke_w > 0.0 {
        let mut st = main.clone(); let r = stroke_w.ceil() as i32;
        for dy in -r..=r { for dx in -r..=r { if ((dx*dx + dy*dy) as f64).sqrt() > stroke_w { continue; }
            for y in 0..h as i32 { let sy = y - dy; if sy < 0 || sy >= h as i32 { continue; } for x in 0..w as i32 { let sx = x - dx; if sx < 0 || sx >= w as i32 { continue; }
                let v = main[sy as usize * w + sx as usize]; let i = y as usize * w + x as usize; if v > st[i] { st[i] = v; } } } } }
        over(fr, &st, [stroke_col[0] as f32, stroke_col[1] as f32, stroke_col[2] as f32]);
    }
    over(fr, &main, [col[0] as f32, col[1] as f32, col[2] as f32]);
    Ok(())
}
