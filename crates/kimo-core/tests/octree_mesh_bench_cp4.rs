//! Octree-mesh CP4: benchmark verification vs pinned numbers + adaptive-density suite
//! (curvature vs flat) + env-gated contact-sheet dumps (KIMO_SDF_FRAMES).
//! Honest scope: timing numbers are measured on this box and reported, not asserted hard;
//! density renders use the EXISTING scene render_cpu path with per-sextile albedo bins
//! (no new pipeline, no shading changes).
#![cfg(feature="surface-contour")]
use std::time::Instant;
use kimo_core::threed::{sdf_graph::{Field,Graph},octree::{self,OctreeConfig},contour_wrapped,scene,vertex_stream::Stream,texture_material::Surface,lighting_stage::Light};
use kimo_core::camera_projection::Camera;

fn cfg(min_d:u32,max_d:u32)->OctreeConfig{OctreeConfig{bounds:[[-2.2;3],[2.2;3]],min_depth:min_d,max_depth:max_d,error_tolerance:0.02,normal_angle_degrees:20.,normal_epsilon:0.001}}
fn sphere()->Graph{Graph{nodes:vec![Field::Sphere{center:[0.;3],radius:1.}],output:0}}
fn torus()->Graph{Graph{nodes:vec![Field::Torus{center:[0.;3],major_radius:1.,minor_radius:0.4}],output:0}}
fn boxg()->Graph{Graph{nodes:vec![Field::Box{center:[0.;3],half_size:[0.6;3]}],output:0}}
fn union_scene()->Graph{Graph{nodes:vec![Field::Sphere{center:[0.35,0.,0.],radius:0.8},Field::Box{center:[-0.35,0.,0.],half_size:[0.6;3]},Field::SmoothUnion{a:0,b:1,k:0.25}],output:2}}
fn near_tangent_scene()->Graph{Graph{nodes:vec![Field::Torus{center:[0.,-0.15,0.],major_radius:1.05,minor_radius:0.42},Field::Capsule{a:[-0.85,-0.95,0.15],b:[0.85,0.95,-0.15],radius:0.34},Field::SmoothUnion{a:0,b:1,k:0.35}],output:2}}
fn camera()->Camera{Camera{eye:[4.8,3.8,-6.8],pan:324.79,tilt:24.55,roll:0.,sensor_height:24.,focal_length:35.,near:0.1}}
fn dir_light()->Light{Light::Directional{travel_direction:[-0.3,-0.5,1.],radiance:[2.5;3]}}
fn point_light()->Light{Light::Point{position:[2.,3.,-4.],intensity:[80.;3],min_distance:0.01}}
fn surf(a:[f64;3])->Surface{Surface{albedo:a,metallic:0.,roughness:0.55,base:None,metal_rough:None,occlusion:None,normal:None,ao_strength:1.,normal_scale:1.,double_sided:false}}
fn ident()->[[f64;4];4]{[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]]}

fn positions(s:&Stream)->Vec<[f64;3]>{s.vertices.iter().map(|v|v.position.map(|x|x as f64)).collect()}
fn face_areas(s:&Stream)->Vec<f64>{let p=positions(s);s.indices.iter().map(|t|{let[a,b,c]=t.map(|i|p[i as usize]);let ab=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];let ac=[c[0]-a[0],c[1]-a[1],c[2]-a[2]];let n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];0.5*(n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt()}).collect()}
fn centroids(s:&Stream)->Vec<[f64;3]>{let p=positions(s);s.indices.iter().map(|t|{let[a,b,c]=t.map(|i|p[i as usize]);[(a[0]+b[0]+c[0])/3.,(a[1]+b[1]+c[1])/3.,(a[2]+b[2]+c[2])/3.]}).collect()}
// same dihedral-fold metric as the pinned uniform benchmark (tests/sdf_contour_frames.rs)
fn folds(s:&Stream)->(usize,usize){
 let p=positions(s);
 let fn_:Vec<[f64;3]>=s.indices.iter().map(|t|{let[a,b,c]=t.map(|i|p[i as usize]);let ab=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];let ac=[c[0]-a[0],c[1]-a[1],c[2]-a[2]];let n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];let l=(n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt();if l>1e-300{[n[0]/l,n[1]/l,n[2]/l]}else{[0.;3]}}).collect();
 use std::collections::HashMap as HM;let mut et:HM<(u32,u32),Vec<usize>>=HM::new();
 for(ti,t)in s.indices.iter().enumerate(){for k in 0..3{let(a,b)=(t[k],t[(k+1)%3]);et.entry((a.min(b),a.max(b))).or_default().push(ti);}}
 let(mut o60,mut o120)=(0,0);
 for(_,ts)in &et{if ts.len()==2{let d=fn_[ts[0]][0]*fn_[ts[1]][0]+fn_[ts[0]][1]*fn_[ts[1]][1]+fn_[ts[0]][2]*fn_[ts[1]][2];let a=d.clamp(-1.,1.).acos().to_degrees();if a>60.{o60+=1;}if a>120.{o120+=1;}}}
 (o60,o120)}

#[test]fn cp4_benchmark_verification_vs_pinned(){
 // 1) the pinned UNIFORM reference numbers themselves still hold (re-derived, exact)
 let uc=|res:u32|contour_wrapped::Config{bounds:[[-2.2;3],[2.2;3]],resolution:res,normal_epsilon:0.001};
 let t0=Instant::now();let u48=contour_wrapped::polygonize(&near_tangent_scene(),&uc(48)).unwrap();let ms48=t0.elapsed().as_millis();
 let(o60_48,o120_48)=folds(&u48.stream);
 assert_eq!((u48.stream.indices.len(),o60_48,o120_48,u48.normal_fallbacks,u48.sliver_triangles,u48.topology_verified),(7612,543,346,0,0,true),"uniform res48 pinned reference drifted");
 let t0=Instant::now();let u64=contour_wrapped::polygonize(&near_tangent_scene(),&uc(64)).unwrap();let ms64=t0.elapsed().as_millis();
 let(o60_64,o120_64)=folds(&u64.stream);
 assert_eq!((u64.stream.indices.len(),o60_64,o120_64,u64.normal_fallbacks,u64.sliver_triangles,u64.topology_verified),(13608,482,288,2,0,true),"uniform res64 pinned reference drifted");
 // 2) adaptive extraction matches the CP2/CP3-pinned numbers exactly (deterministic)
 let t0=Instant::now();let a=octree::extract_mesh(&union_scene(),&cfg(2,8)).unwrap();let ms_ad=t0.elapsed().as_millis();
 assert_eq!(a.stream.indices.len(),9168,"adaptive crease tris drifted");
 let(o60,o120)=folds(&a.stream);
 assert_eq!((o60,o120,a.stats.transition_quads),(471,6,2254),"adaptive crease folds/transitions drifted");
 assert!((a.stats.residual_max-0.0065).abs()<0.0005,"resid {}",a.stats.residual_max);
 let s=octree::extract_mesh(&sphere(),&cfg(2,6)).unwrap();
 assert_eq!((s.stream.vertices.len(),s.stream.indices.len(),s.euler_characteristic),(1016,2028,2),"adaptive sphere drifted");
 assert!((s.signed_volume-4.1958).abs()<0.002,"sphere vol {}",s.signed_volume);
 let t=octree::extract_mesh(&torus(),&cfg(2,7)).unwrap();
 assert_eq!((t.stream.vertices.len(),t.stream.indices.len(),t.euler_characteristic),(4696,9392,0),"adaptive torus drifted");
 assert!((t.signed_volume-3.1599).abs()<0.002,"torus vol {}",t.signed_volume);
 let bc=OctreeConfig{bounds:[[-1.6;3],[1.6;3]],..cfg(2,7)};
 let b=octree::extract_mesh(&boxg(),&bc).unwrap();
 assert_eq!((b.stream.vertices.len(),b.stream.indices.len(),b.euler_characteristic),(4751,9498,2),"adaptive box drifted");
 assert!((b.signed_volume-1.7280).abs()<0.001,"box vol {}",b.signed_volume);
 // 3) measured cross claims (adaptive vs uniform on the same crease scene)
 assert!(9168<13608,"adaptive tris below uniform64");
 assert!(o120*48<=288,"adaptive folds120 at least 48x below uniform64: {o120} vs 288");
 println!("CP4-BENCH uniform48 tris=7612 folds120=346 ms={ms48}; uniform64 tris=13608 folds120=288 ms={ms64}; adaptive tris=9168 folds120=6 trans=2254 ms={ms_ad}");
}

#[test]fn cp4_adaptive_density_curvature_vs_flat(){
 // box: flat face interiors must carry LARGE triangles, edges/corners small ones
 let bc=OctreeConfig{bounds:[[-1.6;3],[1.6;3]],..cfg(2,7)};
 let b=octree::extract_mesh(&boxg(),&bc).unwrap();
 let areas=face_areas(&b.stream);let cents=centroids(&b.stream);
 // distance from a centroid to the nearest of the 12 box edges
 let edge_d=|c:[f64;3]|->f64{let mut best=f64::INFINITY;for ax in 0..3{for sy in [-0.6,0.6]{for sz in [-0.6,0.6]{let mut e=[0.;3];e[ax]=c[ax].clamp(-0.6,0.6);e[(ax+1)%3]=sy;e[(ax+2)%3]=sz;let d=((c[0]-e[0]).powi(2)+(c[1]-e[1]).powi(2)+(c[2]-e[2]).powi(2)).sqrt();if d<best{best=d;}}}}best};
 let near:Vec<f64>=areas.iter().zip(&cents).filter(|(_,c)|edge_d(**c)<0.15).map(|(a,_)|*a).collect();
 let far:Vec<f64>=areas.iter().zip(&cents).filter(|(_,c)|edge_d(**c)>0.35).map(|(a,_)|*a).collect();
 let mean=|v:&[f64]|v.iter().sum::<f64>()/v.len() as f64;
 let(max,min)=(areas.iter().cloned().fold(0.,f64::max),areas.iter().cloned().fold(f64::INFINITY,f64::min));
 println!("CP4-DENSITY box near_edge n={} mean={:.6} far_edge n={} mean={:.6} ratio={:.1} maxmin={:.0}",near.len(),mean(&near),far.len(),mean(&far),mean(&far)/mean(&near),max/min);
 assert!(!near.is_empty()&&!far.is_empty(),"both density populations present");
 assert!(mean(&far)>4.*mean(&near),"flat interiors coarser than edges: {} vs {}",mean(&far),mean(&near));
 assert!(max/min>16.,"box area spread max/min {max}/{min}");
 // union crease: blend-seam region denser than flat box faces
 let u=octree::extract_mesh(&union_scene(),&cfg(2,8)).unwrap();
 let fs=Graph{nodes:vec![Field::Sphere{center:[0.35,0.,0.],radius:0.8}],output:0};
 let fb=Graph{nodes:vec![Field::Box{center:[-0.35,0.,0.],half_size:[0.6;3]}],output:0};
 let areas=face_areas(&u.stream);let cents=centroids(&u.stream);
 let cls:Vec<(f64,f64)>=areas.iter().zip(&cents).map(|(_,c)|(fs.sample(*c).unwrap().abs(),fb.sample(*c).unwrap().abs())).collect(); // (|f_sphere|,|f_box|)
 // distance to nearest edge of the union box (center [-0.35,0,0], half 0.6)
 let uedge=|c:[f64;3]|->f64{let ctr=-0.35;let mut best=f64::INFINITY;for ax in 0..3{for sy in [-0.6,0.6]{for sz in [-0.6,0.6]{let mut e=[0.;3];e[ax]=(c[ax]-if ax==0{ctr}else{0.}).clamp(-0.6,0.6)+if ax==0{ctr}else{0.};e[(ax+1)%3]=if (ax+1)%3==0{sy+ctr}else{sy};e[(ax+2)%3]=if (ax+2)%3==0{sz+ctr}else{sz};let d=((c[0]-e[0]).powi(2)+(c[1]-e[1]).powi(2)+(c[2]-e[2]).powi(2)).sqrt();if d<best{best=d;}}}}best};
 let seam:Vec<f64>=areas.iter().zip(&cls).filter(|(_,(ds,db))|ds.max(*db)<0.25).map(|(a,_)|*a).collect(); // both fields inside blend band
 let flat:Vec<f64>=areas.iter().zip(&cls).zip(&cents).filter(|((_,(ds,db)),c)|*db<0.02&&*ds>0.4&&uedge(**c)>0.15).map(|((a,_),_)|*a).collect(); // box flat interiors far from sphere AND edges
 let edgeband:Vec<f64>=areas.iter().zip(&cents).filter(|(_,c)|uedge(**c)<0.1).map(|(a,_)|*a).collect(); // crease band on the box
 let curved:Vec<f64>=areas.iter().zip(&cls).filter(|(_,(ds,db))|*ds<0.02&&*db>0.3).map(|(a,_)|*a).collect(); // sphere surface away from blend
 println!("CP4-DENSITY union seam n={} mean={:.6} flat_interior n={} mean={:.6} edgeband n={} mean={:.6} curved n={} mean={:.6}",seam.len(),mean(&seam),flat.len(),mean(&flat),edgeband.len(),mean(&edgeband),curved.len(),mean(&curved));
 assert!(!seam.is_empty()&&!flat.is_empty()&&!edgeband.is_empty()&&!curved.is_empty(),"all density populations present");
 assert!(mean(&flat)>2.*mean(&edgeband),"flat interiors coarser than crease band: {} vs {}",mean(&flat),mean(&edgeband));
 assert!(mean(&flat)>mean(&seam),"flat interiors coarser than blend seam: {} vs {}",mean(&flat),mean(&seam));
 // sphere: uniform curvature -> far tighter area spread than the box
 let s=octree::extract_mesh(&sphere(),&cfg(2,6)).unwrap();
 let sa=face_areas(&s.stream);
 let(smax,smin)=(sa.iter().cloned().fold(0.,f64::max),sa.iter().cloned().fold(f64::INFINITY,f64::min));
 println!("CP4-DENSITY sphere maxmin={:.1} (box maxmin above; uniform curvature -> tight spread)",smax/smin);
 assert!(smax/smin<max/min,"sphere area spread {} below box spread {}",smax/smin,max/min);
}

#[test]fn cp4_density_contact_sheet(){
 let dump_env=std::env::var("KIMO_SDF_FRAMES").is_ok();
 if dump_env{std::fs::create_dir_all("/tmp/pm3-frames/cp4").unwrap();}
 // red(small/dense) -> blue(large/flat) ramp, 6 sextile bins per scene (per-scene quantiles:
 // colors compare density WITHIN a scene, not across scenes - captioned on the sheet)
 let ramp=[[0.85,0.2,0.2],[0.9,0.5,0.15],[0.85,0.75,0.2],[0.2,0.75,0.45],[0.15,0.65,0.8],[0.18,0.35,0.85]];
 let scenes:[(&str,Graph,OctreeConfig);3]=[("sphere",sphere(),cfg(2,6)),("box",boxg(),OctreeConfig{bounds:[[-1.6;3],[1.6;3]],..cfg(2,7)}),("crease",union_scene(),cfg(2,8))];
 for(name,g,c)in &scenes{
  let m=octree::extract_mesh(g,c).unwrap();
  assert_eq!((m.boundary_edges,m.nonmanifold_edges),(0,0),"{name} closed manifold");
  let mut order:Vec<usize>=(0..m.stream.indices.len()).collect();
  let areas=face_areas(&m.stream);
  order.sort_by(|a,b|areas[*a].partial_cmp(&areas[*b]).unwrap());
  let n=order.len();let mut bin_of=vec![0usize;n];
  for(rank,ti)in order.into_iter().enumerate(){bin_of[ti]=(rank*6/n).min(5);}
  let mut sources=Vec::new();let mut nodes=Vec::new();let mut materials=Vec::new();
  for b in 0..6{
   let idx:Vec<[u32;3]>=m.stream.indices.iter().enumerate().filter(|(ti,_)|bin_of[*ti]==b).map(|(_,t)|*t).collect();
   if idx.is_empty(){continue;}
   sources.push(scene::Source::Stream{stream:Stream{vertices:m.stream.vertices.clone(),indices:idx}});
   nodes.push(scene::Node{source:sources.len()-1,world:ident(),material:materials.len()});
   materials.push(surf(ramp[b]));}
  let sc=scene::Scene{sources,nodes,materials,lights:vec![dir_light(),point_light()],camera:camera()};
  let c2=scene::compile(&sc,&mut scene::Cache::new()).unwrap();
  let out=scene::render_cpu(&c2,320,180).unwrap();
  assert!(out.covered>1500,"{name} density covered {}",out.covered);
  if dump_env{
   let out=scene::render_cpu(&c2,640,360).unwrap();
   let rgba=kimo_core::render::to_rgba8_tpdf(&out.frame);
   let mut ch=std::process::Command::new("ffmpeg").args(["-v","error","-y","-f","rawvideo","-pix_fmt","rgba","-s","640x360","-i","-","-frames:v","1",&format!("/tmp/pm3-frames/cp4/{name}_density.png")]).stdin(std::process::Stdio::piped()).spawn().unwrap();
   std::io::Write::write_all(&mut ch.stdin.take().unwrap(),&rgba).unwrap();assert!(ch.wait().unwrap().success());
   println!("CP4-FRAME dumped /tmp/pm3-frames/cp4/{name}_density.png");}}
}
