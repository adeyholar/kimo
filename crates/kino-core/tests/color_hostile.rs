use kino_core::{color::{apply3,catalog3},effects::{Frame,Params},keyframe::Prop};
#[test]fn every_color_effect_extreme_render_is_finite_or_error(){
    let skip=["keyer"];let mut calls=0;
    for (kind,_,ps) in catalog3(){if skip.contains(kind){continue}for v in [-1e8,0.,1e8]{let mut pr=Params::new();for(k,_,_)in *ps{pr.insert(k.to_string(),Prop::new(v));}let mut f=Frame{w:16,h:16,d:vec![0.3;16*16*4]};let r=apply3(kind,&pr,&mut f,0.,0);if r.is_ok(){assert!(f.d.iter().all(|x|x.is_finite()),"{kind} returned nonfinite output")}calls+=1;}}
    assert!(calls>=54);
}
