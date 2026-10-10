//! Scene-linear grading: exposed gains, signed contrast, luminance saturation.
use crate::effects::{p,Params,Frame};
pub fn apply(pr:&Params,f:&mut Frame,t:f64)->Result<(),String>{
    let g=|k:&str,d:f64|p(pr,k,d,t) as f64;let gamut=g("gamut",0.);if gamut.fract()!=0.||!(0. ..=4.).contains(&gamut){return Err("linear grade gamut 0..4 required".into())}
    let exposure=g("exposure",0.);let contrast=g("contrast",1.);let pivot=g("pivot",0.18);let saturation=g("saturation",1.);let offset=g("offset",0.);let gains=[g("gain_r",1.),g("gain_g",1.),g("gain_b",1.)];
    if !exposure.is_finite()||exposure.abs()>20.||!(0.01..=10.).contains(&contrast)||!(1e-5..=16.).contains(&pivot)||!(0. ..=10.).contains(&saturation)||!offset.is_finite()||offset.abs()>16.||gains.iter().any(|x|!x.is_finite()||*x<0.||*x>100.){return Err("linear grade parameters outside bounded ranges".into())}
    let weights=super::managed::luma_weights(gamut as usize);let ex=2f64.powf(exposure);
    for px in f.d.chunks_exact_mut(4){let mut c=[0.;3];for j in 0..3{let v=px[j] as f64*ex*gains[j]+offset;c[j]=pivot*(v.abs()/pivot).powf(contrast).copysign(v)}let y=(0..3).map(|j|c[j]*weights[j]).sum::<f64>();for j in 0..3{let v=y+(c[j]-y)*saturation;if !v.is_finite()||v.abs()>f32::MAX as f64{return Err("linear grade overflow".into())}px[j]=v as f32}}
    Ok(())
}
pub fn catalog()->Vec<super::Entry>{vec![("linear_grade","Scene-linear gain/exposure + signed pivoted power contrast + gamut-aware luminance saturation. Preserves negative and HDR values. Input must already be linear; insert color_managed decode first, encode/output-map afterward.",&[("gamut",0.,"0 Rec709,1 Rec2020,2 P3,3 AP1,4 AP0"),("exposure",0.,"stops +/-20"),("gain_r",1.,"0..100"),("gain_g",1.,"0..100"),("gain_b",1.,"0..100"),("offset",0.,"+/-16 linear"),("contrast",1.,"0.01..10, pivoted power"),("pivot",0.18,"linear middle grey"),("saturation",1.,"0..10")])]}
