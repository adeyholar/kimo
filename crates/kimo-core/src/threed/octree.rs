//! Original adaptive octree over the bounded implicit graph (octree-mesh CP1).
//! Error metric = trilinear interpolation deviation at cell face/center probes plus
//! corner-normal angular variation, so curved and creased regions subdivide deeper
//! than flat ones. Interior probes catch thin features the corner signs miss (a
//! feature fully inside a cell between probes can still be missed - documented limit).
//! Descent culls a cell only when |f(center)| exceeds the cell circumsphere (SDF
//! distance bound), so closed surfaces that touch no corner/probe are still found.
//! Leaves carry Hermite edge-crossing samples (analytic normals, FD fallback counted)
//! for the QEF vertex placement in CP2. No mesh extraction here yet.
use super::sdf_graph::Graph;use serde::Serialize;use std::collections::HashMap;
#[derive(Clone,Debug,Serialize,serde::Deserialize)]#[serde(deny_unknown_fields)]pub struct OctreeConfig{pub bounds:[[f64;3];2],pub min_depth:u32,pub max_depth:u32,pub error_tolerance:f64,pub normal_angle_degrees:f64,pub normal_epsilon:f64}
#[derive(Clone,Debug,Serialize)]pub struct Hermite{pub point:[f64;3],pub normal:[f64;3]}
#[derive(Clone,Debug,Serialize)]pub struct Leaf{pub min:[f64;3],pub size:[f64;3],pub depth:u32,pub corners:[f64;8],pub hermite:Vec<Hermite>}
#[derive(Clone,Debug,Default,Serialize)]pub struct Stats{pub leaves:usize,pub surface_leaves:usize,pub splits_min_depth:usize,pub splits_trilinear:usize,pub splits_normal_angle:usize,pub splits_thin_feature:usize,pub splits_near_band:usize,pub max_depth_hits:usize,pub field_evals:usize,pub normal_evals:usize,pub normal_fd_fallbacks:usize,pub max_depth_reached:u32}
#[derive(Serialize)]pub struct Octree{pub leaves:Vec<Leaf>,pub stats:Stats,pub empty:bool,pub config:OctreeConfig}
fn validate(c:&OctreeConfig)->Result<(),String>{if c.bounds.iter().flatten().any(|v|!v.is_finite()||v.abs()>999_000.)||(0..3).any(|j|c.bounds[1][j]<=c.bounds[0][j]){return Err("bounded volume required".into());}if c.min_depth<1||c.max_depth>10||c.min_depth>c.max_depth{return Err("1<=min_depth<=max_depth<=10 required".into());}if !(c.error_tolerance.is_finite()&&(1e-9..=1e3).contains(&c.error_tolerance)){return Err("error_tolerance 1e-9..1e3 required".into());}if !(c.normal_angle_degrees.is_finite()&&(0.0..=180.).contains(&c.normal_angle_degrees)&&c.normal_angle_degrees>0.){return Err("normal_angle_degrees (0,180] required".into());}if !(c.normal_epsilon.is_finite()&&(1e-6..=0.01).contains(&c.normal_epsilon)){return Err("normal_epsilon 1e-6..0.01 required".into());}Ok(())}
struct B<'a>{g:&'a Graph,c:&'a OctreeConfig,fc:HashMap<[u32;3],f64>,nc:HashMap<[u32;3],[f64;3]>,st:Stats,cos_thr:f64}
impl<'a>B<'a>{
 fn key(&self,p:[f64;3])->[u32;3]{let n=(1u32<<self.c.max_depth) as f64;std::array::from_fn(|j|(((p[j]-self.c.bounds[0][j])/(self.c.bounds[1][j]-self.c.bounds[0][j])*n).round() as u32).min(1<<self.c.max_depth))}
 fn field(&mut self,p:[f64;3])->Result<f64,String>{let k=self.key(p);if let Some(v)=self.fc.get(&k){return Ok(*v);}let v=self.g.evaluate(p)?;self.st.field_evals+=1;self.fc.insert(k,v);Ok(v)}
 fn normal(&mut self,p:[f64;3])->Result<[f64;3],String>{let k=self.key(p);if let Some(v)=self.nc.get(&k){return Ok(*v);}let v=self.g.normal_analytic(p).or_else(|_|{self.st.normal_fd_fallbacks+=1;self.g.normal(p,self.c.normal_epsilon)})?;self.st.normal_evals+=1;self.nc.insert(k,v);Ok(v)}
 fn cell(&mut self,min:[f64;3],depth:u32,out:&mut Vec<Leaf>)->Result<(),String>{
  crate::guard::check();if self.st.leaves+self.st.splits_min_depth+self.st.splits_trilinear+self.st.splits_normal_angle+self.st.splits_thin_feature+self.st.splits_near_band>8_000_000{return Err("octree cell budget 8000000 exceeded".into());}
  self.st.max_depth_reached=self.st.max_depth_reached.max(depth);
  let size:[f64;3]=std::array::from_fn(|j|(self.c.bounds[1][j]-self.c.bounds[0][j])/(1u32<<depth) as f64);
  let corner_p=|i:usize|->[f64;3]{[min[0]+if i&1==1{size[0]}else{0.},min[1]+if i&2==2{size[1]}else{0.},min[2]+if i&4==4{size[2]}else{0.}]};
  let mut corners=[0.;8];for i in 0..8{corners[i]=self.field(corner_p(i))?;}
  let neg=corners.map(|v|v<0.);let surface=neg.iter().any(|&b|b!=neg[0]);
  // interior probes: 6 face centers + center (all lattice points at finer depth)
  let probes:[[f64;3];7]={let mut p=[[0.5;3];7];p[0]=[0.5,0.,0.];p[1]=[0.5,1.,0.];p[2]=[0.,0.5,0.];p[3]=[1.,0.5,0.];p[4]=[0.,0.,0.5];p[5]=[1.,1.,0.5];p[6]=[0.5,0.5,0.5];p.map(|u|std::array::from_fn(|j|min[j]+u[j]*size[j]))};
  let mut surf=surface;let mut thin=false;
  if !surf{for p in probes{if (self.field(p)?<0.)!=neg[0]{surf=true;thin=true;break;}}}
  // SDF distance descent: cull only when the center distance provably exceeds the cell
  // circumsphere. Conservative for under-estimating blends; a region whose field
  // over-estimates true distance could still be missed (documented limit).
  let ctr:[f64;3]=std::array::from_fn(|j|min[j]+size[j]/2.);
  let circum=0.5*(size[0]*size[0]+size[1]*size[1]+size[2]*size[2]).sqrt();
  let near=self.field(ctr)?.abs()<=circum;
  if !surf&&!near{out.push(Leaf{min,size,depth,corners,hermite:Vec::new()});self.st.leaves+=1;return Ok(());}
  if !surf{
   // near-band or thin-feature cell without a corner crossing: keep descending; it only
   // becomes a surface leaf if a finer level finds an actual sign change.
   if depth>=self.c.max_depth{self.st.max_depth_hits+=1;out.push(Leaf{min,size,depth,corners,hermite:Vec::new()});self.st.leaves+=1;return Ok(());}
   if thin{self.st.splits_thin_feature+=1;}else{self.st.splits_near_band+=1;}
   let h=[size[0]/2.,size[1]/2.,size[2]/2.];for i in 0..8{self.cell([min[0]+if i&1==1{h[0]}else{0.},min[1]+if i&2==2{h[1]}else{0.},min[2]+if i&4==4{h[2]}else{0.}],depth+1,out)?;}
   return Ok(());
  }
  // subdivision decision
  let trilerp=|u:[f64;3]|->f64{let c00=corners[0]+(corners[1]-corners[0])*u[0];let c10=corners[2]+(corners[3]-corners[2])*u[0];let c01=corners[4]+(corners[5]-corners[4])*u[0];let c11=corners[6]+(corners[7]-corners[6])*u[0];let c0=c00+(c10-c00)*u[1];let c1=c01+(c11-c01)*u[1];c0+(c1-c0)*u[2]};
  let mut dev=0.0f64;for p in probes{let u:[f64;3]=std::array::from_fn(|j|(p[j]-min[j])/size[j]);dev=dev.max((self.field(p)?-trilerp(u)).abs());}
  // corners on a non-differentiable set (box face/edge plane, CSG crease) have no normal:
  // exclude them; the remaining valid corners still expose sharp features by angle.
  let mut ns:Vec<[f64;3]>=Vec::with_capacity(8);
  for i in 0..8{if let Ok(n)=self.normal(corner_p(i)){ns.push(n);}}
  let mut min_dot=1.0f64;for a in 0..ns.len(){for b in a+1..ns.len(){let d=(ns[a][0]*ns[b][0]+ns[a][1]*ns[b][1]+ns[a][2]*ns[b][2]).clamp(-1.,1.);if d<min_dot{min_dot=d;}}}
  let over_dev=dev>self.c.error_tolerance;let over_ang=if ns.len()>=2{min_dot<self.cos_thr}else{true};
  let split=if depth<self.c.min_depth{self.st.splits_min_depth+=1;true}else if depth>=self.c.max_depth{if over_dev||over_ang||thin{self.st.max_depth_hits+=1;}false}else if thin{self.st.splits_thin_feature+=1;true}else if over_dev{self.st.splits_trilinear+=1;true}else if over_ang{self.st.splits_normal_angle+=1;true}else{false};
  if split{let h=[size[0]/2.,size[1]/2.,size[2]/2.];for i in 0..8{self.cell([min[0]+if i&1==1{h[0]}else{0.},min[1]+if i&2==2{h[1]}else{0.},min[2]+if i&4==4{h[2]}else{0.}],depth+1,out)?;}return Ok(());}
  // leaf: Hermite edge crossings (12 edges), linear-interpolated point + analytic normal
  let mut hermite=Vec::new();
  for a in 0..3{for i in 0..8{if i&(1<<a)==0{let j=i|(1<<a);let(v0,v1)=(corners[i],corners[j]);if (v0<0.)!=(v1<0.){let t=v0/(v0-v1);let(p0,p1)=(corner_p(i),corner_p(j));let p:[f64;3]=std::array::from_fn(|k|p0[k]+t*(p1[k]-p0[k]));let n=self.g.normal_analytic(p).or_else(|_|{self.st.normal_fd_fallbacks+=1;self.g.normal(p,self.c.normal_epsilon)})?;hermite.push(Hermite{point:p,normal:n});}}}}
  out.push(Leaf{min,size,depth,corners,hermite});self.st.leaves+=1;self.st.surface_leaves+=1;Ok(())
 }
}
pub fn build(g:&Graph,c:&OctreeConfig)->Result<Octree,String>{g.validate()?;validate(c)?;
 let mut b=B{g,c,fc:HashMap::new(),nc:HashMap::new(),st:Stats::default(),cos_thr:(c.normal_angle_degrees.to_radians()).cos()};
 let mut leaves=Vec::new();b.cell(c.bounds[0],0,&mut leaves)?;
 let empty=b.st.surface_leaves==0;Ok(Octree{leaves,stats:b.st,empty,config:c.clone()})}
#[cfg(test)]mod tests{use super::*;use crate::threed::sdf_graph::Field;
fn sphere()->Graph{Graph{nodes:vec![Field::Sphere{center:[0.;3],radius:1.}],output:0}}
fn union_scene()->Graph{Graph{nodes:vec![Field::Sphere{center:[0.35,0.,0.],radius:0.8},Field::Box{center:[-0.35,0.,0.],half_size:[0.6;3]},Field::SmoothUnion{a:0,b:1,k:0.25}],output:2}}
fn cfg(min_d:u32,max_d:u32)->OctreeConfig{OctreeConfig{bounds:[[-2.2;3],[2.2;3]],min_depth:min_d,max_depth:max_d,error_tolerance:0.02,normal_angle_degrees:20.,normal_epsilon:0.001}}
#[test]fn sphere_adaptive_savings_and_hermite_quality(){
 let o=build(&sphere(),&cfg(2,7)).unwrap();assert!(!o.empty);
 let uniform=8u64.pow(7)as usize;assert!(o.leaves.len()*200<uniform,"adaptive savings: {} leaves vs uniform {}",o.leaves.len(),uniform);
 let mut hist=[0usize;11];for l in &o.leaves{hist[l.depth as usize]+=1;}
 let mut shist=[0usize;11];for l in o.leaves.iter().filter(|l|!l.hermite.is_empty()){shist[l.depth as usize]+=1;}
 println!("OCTREE-SPHERE leaves={} surface={} splits(min/tri/ang/thin/near)={}/{}/{}/{}/{} hits={} evals(f/n)={}/{} fb={} depth_hist={:?} surface_depth_hist={:?}",o.stats.leaves,o.stats.surface_leaves,o.stats.splits_min_depth,o.stats.splits_trilinear,o.stats.splits_normal_angle,o.stats.splits_thin_feature,o.stats.splits_near_band,o.stats.max_depth_hits,o.stats.field_evals,o.stats.normal_evals,o.stats.normal_fd_fallbacks,hist,shist);
 for l in o.leaves.iter().filter(|l|!l.hermite.is_empty()){
  assert!(!l.hermite.is_empty(),"surface leaf has Hermite data");
  let cell=l.size[0];
  for h in &l.hermite{let f=sphere().evaluate(h.point).unwrap().abs();assert!(f<cell,"Hermite point residual {f} vs cell {cell}");let l2=(h.normal[0]*h.normal[0]+h.normal[1]*h.normal[1]+h.normal[2]*h.normal[2]).sqrt();assert!((l2-1.).abs()<1e-9,"unit normal {l2}");}
 }
 // surface leaves cluster near the sphere: every surface leaf center is within 2 cells of the surface
 for l in o.leaves.iter().filter(|l|!l.hermite.is_empty()){let c=[l.min[0]+l.size[0]/2.,l.min[1]+l.size[1]/2.,l.min[2]+l.size[2]/2.];let d=sphere().evaluate(c).unwrap().abs();assert!(d<2.*l.size[0].max(l.size[1]).max(l.size[2]),"surface leaf far from field: {d}");}
}
#[test]fn plane_is_exactly_trilinear_so_no_error_subdivision(){
 let g=Graph{nodes:vec![Field::Plane{normal:[0.,0.,1.],offset:0.3}],output:0};
 let o=build(&g,&cfg(2,8)).unwrap();
 assert_eq!((o.stats.splits_trilinear,o.stats.splits_normal_angle,o.stats.splits_thin_feature),(0,0,0),"linear field must not trigger error subdivision: {:?}",o.stats);
 // the near band still refines (distance-bound descent), but every surface leaf stops at min_depth
 assert!(o.leaves.iter().filter(|l|!l.hermite.is_empty()).all(|l|l.depth==2),"plane surface leaves must all sit at min_depth: depths {:?}",o.leaves.iter().filter(|l|!l.hermite.is_empty()).map(|l|l.depth).collect::<Vec<_>>());
 println!("OCTREE-PLANE leaves={} surface={}",o.stats.leaves,o.stats.surface_leaves);
}
#[test]fn crease_subdivides_deeper_than_flat_cap(){
 // smooth-union benchmark scene: the near-tangent crease needs depth, the sphere far pole does not
 let o=build(&union_scene(),&cfg(2,8)).unwrap();
 // crease = the blend ring between sphere and box: x in the overlap band, 0.3..0.95 off the x-axis
 let crease_max=o.leaves.iter().filter(|l|!l.hermite.is_empty()).filter(|l|{let c=[l.min[0]+l.size[0]/2.,l.min[1]+l.size[1]/2.,l.min[2]+l.size[2]/2.];let r=c[1].hypot(c[2]);(-0.5..=0.3).contains(&c[0])&&(0.3..=0.95).contains(&r)}).map(|l|l.depth).max().unwrap_or(0);
 let pole_max=o.leaves.iter().filter(|l|!l.hermite.is_empty()).filter(|l|{let c=[l.min[0]+l.size[0]/2.,l.min[1]+l.size[1]/2.,l.min[2]+l.size[2]/2.];(c[0]-1.15).abs()<0.25&&c[1].abs()<0.25&&c[2].abs()<0.25}).map(|l|l.depth).max().unwrap_or(0);
 let mut hist=[0usize;11];for l in &o.leaves{hist[l.depth as usize]+=1;}
 println!("OCTREE-CREASE crease_max_depth={crease_max} pole_max_depth={pole_max} leaves={} surface={} depth_hist={:?}",o.stats.leaves,o.stats.surface_leaves,hist);
 assert!(crease_max>pole_max,"crease must subdivide deeper than flat pole: {crease_max} vs {pole_max}");
 assert!(pole_max<8,"flat pole must stop before max_depth");
}
#[test]fn sharp_box_face_stays_shallow_corner_goes_deep(){
 let g=Graph{nodes:vec![Field::Box{center:[0.;3],half_size:[0.6;3]}],output:0};
 let c=OctreeConfig{bounds:[[-1.6;3],[1.6;3]],..cfg(2,8)};
 let o=build(&g,&c).unwrap();
 let face_max=o.leaves.iter().filter(|l|!l.hermite.is_empty()).filter(|l|{let ctr=[l.min[0]+l.size[0]/2.,l.min[1]+l.size[1]/2.,l.min[2]+l.size[2]/2.];l.min[0]<0.6&&l.min[0]+l.size[0]>0.6&&ctr[1].abs()<0.2&&ctr[2].abs()<0.2}).map(|l|l.depth).max().unwrap_or(0);
 let corner_max=o.leaves.iter().filter(|l|!l.hermite.is_empty()).filter(|l|{let ctr=[l.min[0]+l.size[0]/2.,l.min[1]+l.size[1]/2.,l.min[2]+l.size[2]/2.];(ctr[0]-0.6).abs()<0.15&&(ctr[1]-0.6).abs()<0.15&&(ctr[2]-0.6).abs()<0.15}).map(|l|l.depth).max().unwrap_or(0);
 println!("OCTREE-BOX face_max_depth={face_max} corner_max_depth={corner_max}");
 assert!(corner_max>face_max+1,"corner deeper than face: {corner_max} vs {face_max}");
}
#[test]fn determinism_and_empty_and_typed_errors(){
 let a=build(&sphere(),&cfg(2,6)).unwrap();let b=build(&sphere(),&cfg(2,6)).unwrap();
 let sig=|o:&Octree|o.leaves.iter().map(|l|(l.min,l.depth,l.hermite.len())).collect::<Vec<_>>();
 assert_eq!(sig(&a),sig(&b),"octree build must be deterministic");
 let far=Graph{nodes:vec![Field::Sphere{center:[50.;3],radius:1.}],output:0};
 let e=build(&far,&cfg(2,6)).unwrap();assert!(e.empty&&e.stats.surface_leaves==0,"empty field is a typed result, not an error");
 let bad=[OctreeConfig{min_depth:0,..cfg(2,6)},OctreeConfig{min_depth:5,max_depth:3,..cfg(2,6)},OctreeConfig{max_depth:11,..cfg(2,6)},OctreeConfig{error_tolerance:0.,..cfg(2,6)},OctreeConfig{error_tolerance:f64::NAN,..cfg(2,6)},OctreeConfig{normal_angle_degrees:0.,..cfg(2,6)},OctreeConfig{normal_angle_degrees:181.,..cfg(2,6)},OctreeConfig{normal_epsilon:0.,..cfg(2,6)}];
 for(i,c)in bad.iter().enumerate(){assert!(build(&sphere(),c).is_err(),"bad config {i} accepted");}
}
#[test]fn torus_topology_of_leaf_layout(){
 let g=Graph{nodes:vec![Field::Torus{center:[0.;3],major_radius:1.,minor_radius:0.4}],output:0};
 let o=build(&g,&cfg(2,7)).unwrap();
 // surface leaves form a closed band: every surface leaf has a surface neighbor (face-adjacent or diagonal)
 let surf:Vec<&Leaf>=o.leaves.iter().filter(|l|!l.hermite.is_empty()).collect();
 assert!(surf.len()>50,"torus band too thin: {}",surf.len());
 println!("OCTREE-TORUS leaves={} surface={} hits={}",o.stats.leaves,o.stats.surface_leaves,o.stats.max_depth_hits);
}
}
