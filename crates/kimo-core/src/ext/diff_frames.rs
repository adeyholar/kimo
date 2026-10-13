//! RV-26 deterministic two-render frame comparison, read-only and advisory.
use super::kit::*;
use crate::perception::analysis::{self, Image};
pub const METRICS: &[&str] = &[
    "mse",
    "psnr",
    "ssim",
    "max_abs",
    "changed_ratio",
    "luma_delta",
    "hist_distance",
];
pub fn compare(a: &Image, b: &Image, threshold: f64, cols: usize, rows: usize) -> Value {
    let n = a.w * a.h;
    let (mut mse, mut max, mut changed) = (0., 0f64, 0usize);
    let mut grid = vec![0.; cols * rows];
    let mut counts = vec![0usize; cols * rows];
    for i in 0..n {
        if i % 16384 == 0 {
            crate::guard::check();
        }
        let mut abs = 0.;
        let mut mx = 0f64;
        for c in 0..3 {
            let d = (a.rgb[c][i] - b.rgb[c][i]).abs();
            mse += d * d;
            mx = mx.max(d);
            abs += d / 3.;
        }
        max = max.max(mx);
        if mx > threshold {
            changed += 1;
        }
        let x = i % a.w;
        let y = i / a.w;
        let g = (y * rows / a.h).min(rows - 1) * cols + (x * cols / a.w).min(cols - 1);
        grid[g] += abs;
        counts[g] += 1;
    }
    mse /= (n * 3) as f64;
    for (i, v) in grid.iter_mut().enumerate() {
        *v /= counts[i].max(1) as f64;
    }
    let mut ssim = 0.;
    let mut windows = 0;
    for y in (0..a.h).step_by(8) {
        for x in (0..a.w).step_by(8) {
            let mut va = vec![];
            let mut vb = vec![];
            for yy in y..(y + 8).min(a.h) {
                for xx in x..(x + 8).min(a.w) {
                    let i = yy * a.w + xx;
                    va.push(a.luma(i));
                    vb.push(b.luma(i));
                }
            }
            let nn = va.len() as f64;
            let ma = va.iter().sum::<f64>() / nn;
            let mb = vb.iter().sum::<f64>() / nn;
            let sa = va.iter().map(|v| (v - ma).powi(2)).sum::<f64>() / nn;
            let sb = vb.iter().map(|v| (v - mb).powi(2)).sum::<f64>() / nn;
            let cov = va
                .iter()
                .zip(&vb)
                .map(|(a, b)| (a - ma) * (b - mb))
                .sum::<f64>()
                / nn;
            ssim += ((2. * ma * mb + 0.0001) * (2. * cov + 0.0009))
                / ((ma * ma + mb * mb + 0.0001) * (sa + sb + 0.0009));
            windows += 1;
        }
    }
    let ratio = changed as f64 / n as f64;
    json!({"metrics":{"mse":mse,"psnr_db":if mse==0.{99.}else{-10.*mse.log10()},"ssim":ssim/windows as f64,"max_abs":max,"changed_ratio":ratio,"luma_delta":analysis::luma_delta(a,b),"hist_distance":analysis::hist_distance(a,b)},"grid":{"cols":cols,"rows":rows,"mean_abs_delta":grid},"verdict_hint":if mse==0.{"identical"}else if ratio<0.02{"minor"}else if ratio<0.2{"moderate"}else{"major"}})
}
fn run(ss: &mut Session, a: &Value) -> Result<Value, String> {
    let aa = a.get("a").ok_or("missing object 'a'")?;
    let bb = a.get("b").ok_or("missing object 'b'")?;
    let ar = analysis::parse(ss, aa)?;
    let br = analysis::parse(ss, bb)?;
    let metrics = match a.get("metrics") {
        None => vec!["mse", "psnr", "changed_ratio", "luma_delta"],
        Some(v) => v
            .as_array()
            .ok_or("invalid_metric: metrics must be an array")?
            .iter()
            .map(|x| x.as_str().ok_or("invalid_metric: metric must be a string"))
            .collect::<Result<Vec<_>, _>>()?,
    };
    for m in &metrics {
        if !METRICS.contains(m) {
            return Err("invalid_metric: Valid: mse, psnr, ssim, max_abs, changed_ratio, luma_delta, hist_distance".into());
        }
    }
    let threshold = a
        .get("threshold")
        .map(|v| v.as_f64().ok_or("out_of_range: threshold numeric 0..1"))
        .transpose()?
        .unwrap_or(0.04);
    if !threshold.is_finite() || !(0.0..=1.0).contains(&threshold) {
        return Err("out_of_range: threshold must be 0..1".into());
    }
    let scale = a
        .get("scale")
        .map(|v| v.as_f64().ok_or("out_of_range: scale numeric"))
        .transpose()?
        .unwrap_or(480.);
    if !scale.is_finite() || scale <= 0. || scale > 8192. {
        return Err("out_of_range: scale must be >0 and <=8192 (1 = full resolution)".into());
    }
    let ga = a.get("grid");
    if ga.map_or(false, |v| !v.is_object()) {
        return Err("out_of_range: grid must be object cols/rows".into());
    }
    let gridnum = |key: &str| -> Result<usize, String> {
        ga.and_then(|v| v.get(key))
            .map(|v| {
                v.as_u64()
                    .map(|v| v as usize)
                    .ok_or_else(|| format!("out_of_range: grid {key} integer required"))
            })
            .transpose()
            .map(|v| v.unwrap_or(4))
    };
    let cols = gridnum("cols")?;
    let rows = gridnum("rows")?;
    if cols == 0 || rows == 0 || cols > 64 || rows > 64 {
        return Err("out_of_range: grid cols/rows must be 1..64".into());
    }
    let mut ia = analysis::render(ss, &ar)?;
    let mut ib = analysis::render(ss, &br)?;
    let (aw, ah, bw, bh) = (ia.w, ia.h, ib.w, ib.h);
    if aw != bw || ah != bh {
        return Err(format!("size_mismatch: a={aw}x{ah}, b={bw}x{bh}; choose equal source canvas sizes before scale/region"));
    }
    let region = match a.get("region") {
        None => [0, 0, aw, ah],
        Some(v) => {
            let v = v
                .as_array()
                .filter(|v| v.len() == 4)
                .ok_or("out_of_range: region must be [x,y,w,h] pixels")?;
            let r = v
                .iter()
                .map(|v| {
                    v.as_u64()
                        .map(|v| v as usize)
                        .ok_or("out_of_range: region integer pixels required")
                })
                .collect::<Result<Vec<_>, _>>()?;
            [r[0], r[1], r[2], r[3]]
        }
    };
    if region[2] == 0
        || region[3] == 0
        || region[0].checked_add(region[2]).map_or(true, |v| v > aw)
        || region[1].checked_add(region[3]).map_or(true, |v| v > ah)
    {
        return Err("out_of_range: region outside image".into());
    }
    ia = ia.crop(region);
    ib = ib.crop(region);
    let ratio = if scale == 1. {
        1.
    } else {
        (scale / ia.w.max(ia.h) as f64).min(1.)
    };
    let (w, h) = (
        ((ia.w as f64 * ratio).round() as usize).max(1),
        ((ia.h as f64 * ratio).round() as usize).max(1),
    );
    if w != ia.w || h != ia.h {
        ia = ia.downscale(w, h);
        ib = ib.downscale(w, h);
    }
    let mut out = compare(&ia, &ib, threshold, cols, rows);
    let selected = out["metrics"].as_object_mut().unwrap();
    selected.retain(|k, _| metrics.contains(&if k == "psnr_db" { "psnr" } else { k.as_str() }));
    out["a"] = json!({"time":ar.time,"source":ar.source,"w":aw,"h":ah});
    out["b"] = json!({"time":br.time,"source":br.source,"w":bw,"h":bh});
    out["compared"] = json!({"w":w,"h":h,"scale":ratio,"region":region,"threshold":threshold,"space":"display-referred RGB u8/255","method":"fixed-order f64, box downscale, Rec709 luma, SSIM luma 8x8 population variance C1=.01^2 C2=.03^2, 8x8x8 RGB histogram half-L1"});
    if a.get("heatmap").and_then(Value::as_bool).unwrap_or(false) {
        let mut fr = crate::effects::Frame::new(w, h);
        for i in 0..w * h {
            for c in 0..3 {
                fr.d[i * 4 + c] = ((ia.rgb[c][i] - ib.rgb[c][i]).abs() * 4.).min(1.) as f32;
            }
            fr.d[i * 4 + 3] = 1.;
        }
        let mut hash = 0xcbf29ce484222325u64;
        for v in &fr.d {
            for b in v.to_bits().to_le_bytes() {
                hash = (hash ^ b as u64).wrapping_mul(0x100000001b3);
            }
        }
        let path = std::env::temp_dir().join(format!("kimo-diff-{w}x{h}-{hash:016x}.png"));
        crate::render::write_image(
            &fr,
            path.to_str()
                .ok_or("invalid_source: non-UTF8 output path")?,
        )?;
        ss.media.push(crate::commands::Media {
            path: path.to_string_lossy().into(),
            w: w as u32,
            h: h as u32,
            mime: "image/png",
        });
        out["heatmap"] = json!({"path":path,"image":{"mime_type":"image/png","gain":4.0}});
    }
    Ok(out)
}
pub fn commands() -> Vec<Command> {
    vec![Command{name:"diff_frames",mutates:false,description:"Compare two rendered frames, advisory only. a/b={time or frame, source:composite|clip:<id>|file:<path>}. At most two full render pipeline calls, no thumbnail comparisons. Different source sizes error before identical box-filter analysis downscale. RGB u8/255 MSE, PSNR 99 on identical, SSIM 8x8 luma C1=.01 C2=.03, max absolute delta, changed pixel ratio, Rec709 luma delta and 8x8x8 histogram half-L1. Region pixel crop, 4x4 mean-delta grid; optional abs-diff x4 PNG heatmap file and inline image. No edit or undo entry.",schema:obj(vec![("a",json!({"type":"object"})),("b",json!({"type":"object"})),("region",json!({"type":"array","items":{"type":"integer"},"minItems":4,"maxItems":4})),("scale",n("max analysis side, default480;1 means full res")),("metrics",json!({"type":"array","items":{"type":"string"}})),("threshold",n("changed_ratio abs threshold 0..1, default.04")),("heatmap",json!({"type":"boolean"})),("grid",json!({"type":"object","properties":{"cols":{"type":"integer"},"rows":{"type":"integer"}}}))],&["a","b"]),run}]
}
