//! Motion tracking and stabilization by block matching on decoded frames.
use crate::render::{prepare, Cache};
use crate::model::*;

fn gray(frame: &[u8], w: usize, h: usize) -> Vec<f32> { (0..w*h).map(|i| 0.299*frame[i*4] as f32 + 0.587*frame[i*4+1] as f32 + 0.114*frame[i*4+2] as f32).collect() }

/// Find where `tpl` (size*size, centred at (cx,cy) in prev) moved in `cur`, within +-range. Returns (x,y,score) of centre.
fn match_block(prev: &[f32], cur: &[f32], w: usize, h: usize, cx: f32, cy: f32, size: i32, range: i32) -> (f32, f32, f32) {
    let r = size / 2; let (ix, iy) = (cx.round() as i32, cy.round() as i32);
    let mut best = (f32::MAX, 0, 0); let mut grid = std::collections::HashMap::new();
    for dy in -range..=range { for dx in -range..=range {
        let mut sad = 0f32; let mut ok = true;
        'o: for y in (-r..r).step_by(2) { for x in (-r..r).step_by(2) {
            let (px, py, qx, qy) = (ix + x, iy + y, ix + x + dx, iy + y + dy);
            if px < 0 || py < 0 || qx < 0 || qy < 0 || px >= w as i32 || qx >= w as i32 || py >= h as i32 || qy >= h as i32 { ok = false; break 'o; }
            sad += (prev[py as usize*w+px as usize] - cur[qy as usize*w+qx as usize]).abs(); } }
        if !ok { continue; } grid.insert((dx, dy), sad); if sad < best.0 { best = (sad, dx, dy); } } }
    let (s, dx, dy) = best; let g = |a: i32, b: i32| grid.get(&(a, b)).copied().unwrap_or(s);
    let sub = |m: f32, c: f32, p: f32| { let d = m - 2.0*c + p; if d.abs() < 1e-6 { 0.0 } else { 0.5*(m - p)/d } };
    let (fx, fy) = (sub(g(dx-1, dy), s, g(dx+1, dy)), sub(g(dx, dy-1), s, g(dx, dy+1)));
    (cx + dx as f32 + fx.clamp(-0.5, 0.5), cy + dy as f32 + fy.clamp(-0.5, 0.5), s / ((size/2) * (size/2)) as f32 / 4.0)
}

/// Track a point (normalized 0..1) through a media clip. Returns per-frame (clip_local_time, px_x, px_y).
pub fn track_point(p: &Project, clip_id: &str, nx: f64, ny: f64, size: i32, range: i32) -> Result<Vec<(f64, f64, f64)>, String> {
    let c = p.find_clip(clip_id).ok_or("no such clip")?; let mut cache = Cache::new(); prepare(p, &mut cache)?;
    let frames = cache.frames_of(clip_id).ok_or("clip is not a media clip")?; let (w, h) = (p.width as usize, p.height as usize);
    let mut out = vec![]; let (mut x, mut y) = (nx as f32 * w as f32, ny as f32 * h as f32);
    let mut prev = gray(&frames[0], w, h); out.push((0.0, x as f64, y as f64));
    for i in 1..frames.len() { let cur = gray(&frames[i], w, h);
        let (nx2, ny2, _) = match_block(&prev, &cur, w, h, x, y, size, range); x = nx2; y = ny2; prev = cur; out.push((i as f64 / p.fps, x as f64, y as f64)); }
    let _ = c; Ok(out)
}

/// Estimate camera shake: global translation per frame from 5 patches (median), returns smoothed-compensation offsets (dx,dy) px per frame.
pub fn stabilize(p: &Project, clip_id: &str, smooth: usize) -> Result<Vec<(f64, f64, f64)>, String> {
    let mut cache = Cache::new(); prepare(p, &mut cache)?; let frames = cache.frames_of(clip_id).ok_or("not a media clip")?; let (w, h) = (p.width as usize, p.height as usize);
    let pts = [(0.3, 0.3), (0.7, 0.3), (0.5, 0.5), (0.3, 0.7), (0.7, 0.7)];
    let mut path = vec![(0f64, 0f64)]; let mut prev = gray(&frames[0], w, h);
    for i in 1..frames.len() { let cur = gray(&frames[i], w, h); let mut dxs = vec![]; let mut dys = vec![];
        for (px, py) in pts { let (cx, cy) = (px * w as f32, py * h as f32); let (x, y, _) = match_block(&prev, &cur, w, h, cx, cy, 48, 12); dxs.push((x - cx) as f64); dys.push((y - cy) as f64); }
        dxs.sort_by(|a, b| a.partial_cmp(b).unwrap()); dys.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let l = path[i - 1]; path.push((l.0 + dxs[2], l.1 + dys[2])); prev = cur; }
    let n = path.len(); let r = smooth.max(1);
    Ok((0..n).map(|i| { let (a, b) = (i.saturating_sub(r), (i + r).min(n - 1)); let m = (b - a + 1) as f64;
        let (sx, sy) = path[a..=b].iter().fold((0.0, 0.0), |s, q| (s.0 + q.0, s.1 + q.1)); (i as f64 / p.fps, sx / m - path[i].0, sy / m - path[i].1) }).collect())
}
