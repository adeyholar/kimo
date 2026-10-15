use serde::{Serialize,Deserialize};use super::{pbr::{Material,sample},vertex_stream::unit};
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]pub enum Light{Directional{travel_direction:[f64;3],radiance:[f64;3]},Point{position:[f64;3],intensity:[f64;3],min_distance:f64}}
fn dot(a:[f64;3],b:[f64;3])->f64{a.iter().zip(b).map(|(a,b)|a*b).sum()}
pub fn validate(lights:&[Light])->Result<(),String>{if lights.len()>64{return Err("max64 explicit lights".into());}for l in lights{let (rgb,dir)=match l{Light::Directional{travel_direction,radiance}=>(radiance,*travel_direction),Light::Point{position,intensity,min_distance}=>{if !min_distance.is_finite()||!(1e-6..=1e6).contains(min_distance){return Err("point min_distance1e-6..1e6 required".into());}(intensity,*position)}};if rgb.iter().any(|v|!v.is_finite()||*v<0.||*v>1e9)||dir.iter().any(|v|!v.is_finite()||v.abs()>1e9){return Err("bounded finite light values required".into());}if matches!(l,Light::Directional{..}){unit(dir)?;}}Ok(())}
/// World-space, linear radiance; sum direct lighting. No ambient/shadow/IBL.
pub fn shade(material:&Material,point:[f64;3],normal:[f64;3],eye:[f64;3],lights:&[Light])->Result<[f64;3],String>{validate(lights)?;if point.iter().chain(eye.iter()).any(|v|!v.is_finite()||v.abs()>1e9){return Err("bounded finite point/eye required".into());}let n=unit(normal)?;let view=unit(std::array::from_fn(|j|eye[j]-point[j]))?;sample(material,n,view,view)?;let mut out=[0.;3];for light in lights{let(l,rgb)=match light{Light::Directional{travel_direction,radiance}=>(unit(travel_direction.map(|v|-v))?,*radiance),Light::Point{position,intensity,min_distance}=>{let d:[f64;3]=std::array::from_fn(|j|position[j]-point[j]);let distance=d[0].hypot(d[1]).hypot(d[2]);if distance==0.{continue;}let denom=distance.max(*min_distance).powi(2);(unit(d)?,intensity.map(|i|i/denom))}};let nl=dot(n,l).max(0.);let brdf=sample(material,n,l,view)?;for j in 0..3{out[j]+=(brdf.diffuse[j]+brdf.specular[j])*rgb[j]*nl;}}if out.iter().any(|v|!v.is_finite()||*v>f32::MAX as f64){return Err("radiance overflow".into());}Ok(out)}
/// shade() with per-light visibility (1.0 = fully lit). vis.len() must equal lights.len();
/// shade() is the all-lit special case. Same BRDF order/constants so all-lit output is
/// bit-identical to shade().
pub fn shade_shadowed(material:&Material,point:[f64;3],normal:[f64;3],eye:[f64;3],lights:&[Light],vis:&[f64])->Result<[f64;3],String>{
 validate(lights)?;if vis.len()!=lights.len(){return Err("visibility per light required".into());}
 if vis.iter().any(|v|!v.is_finite()||!(0.0..=1.0).contains(v)){return Err("visibility 0..1 required".into());}
 if point.iter().chain(eye.iter()).any(|v|!v.is_finite()||v.abs()>1e9){return Err("bounded finite point/eye required".into());}
 let n=unit(normal)?;let view=unit(std::array::from_fn(|j|eye[j]-point[j]))?;sample(material,n,view,view)?;let mut out=[0.;3];
 for (i,light) in lights.iter().enumerate(){let v=vis[i];if v==0.0{continue;}
  let(l,rgb)=match light{Light::Directional{travel_direction,radiance}=>(unit(travel_direction.map(|x|-x))?,*radiance),Light::Point{position,intensity,min_distance}=>{let d:[f64;3]=std::array::from_fn(|j|position[j]-point[j]);let distance=d[0].hypot(d[1]).hypot(d[2]);if distance==0.{continue;}let denom=distance.max(*min_distance).powi(2);(unit(d)?,intensity.map(|i|i/denom))}};
  let nl=dot(n,l).max(0.);let brdf=sample(material,n,l,view)?;for j in 0..3{out[j]+=(brdf.diffuse[j]+brdf.specular[j])*rgb[j]*v*nl;}}
 if out.iter().any(|v|!v.is_finite()||*v>f32::MAX as f64){return Err("radiance overflow".into());}Ok(out)}

#[cfg(test)]mod tests{use super::*;
 #[test]fn directional_and_point_finite_inverse_square(){let m=Material{albedo:[0.5;3],metallic:0.,roughness:0.5};let light=|z|Light::Point{position:[0.,0.,z],intensity:[1.;3],min_distance:0.01};let a=shade(&m,[0.;3],[0.,0.,1.],[0.,0.,4.],&[light(2.)]).unwrap();let b=shade(&m,[0.;3],[0.,0.,1.],[0.,0.,4.],&[light(4.)]).unwrap();for j in 0..3{assert!((a[j]/b[j]-4.).abs()<1e-12);}let d=Light::Directional{travel_direction:[0.,0.,-1.],radiance:[1.;3]};assert!(shade(&m,[0.;3],[0.,0.,1.],[0.,0.,4.],&[d]).unwrap().iter().all(|v|v.is_finite()&&*v>0.));}
 #[test]fn coincident_point_is_explicit_zero_contribution(){let m=Material{albedo:[0.5;3],metallic:1.,roughness:0.5};assert_eq!(shade(&m,[0.;3],[0.,0.,1.],[0.,0.,4.],&[Light::Point{position:[0.;3],intensity:[1.;3],min_distance:0.01}]).unwrap(),[0.;3]);}
}
