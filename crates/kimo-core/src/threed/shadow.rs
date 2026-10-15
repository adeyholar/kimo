//! Directional shadow maps, shared by lit_raster (CPU production), pbr_offscreen (wgpu
//! executor) and textured_bvh (raycast reference). ONE light-space definition (view frame
//! + ortho bounds), ONE bias contract, so the three paths cannot drift: the wgpu pass gets
//! the same constants as uniforms and mirrors visibility() tap-for-tap (compare-then-average
//! PCF, never average-then-compare).
//!
//! PL1 scope: directional lights only (point cube shadows are PL2). Hard shadows = PCF
//! radius 0; the raycast reference matches PCF radius 0 exactly (its shadow rays are hard).
//! Bias contract (pinned, mirrored in WGSL uniforms): receiver point is first offset along
//! the surface normal by NORMAL_OFFSET_TEXELS shadow texels (world units), then the depth
//! compare uses bias_depth = (BIAS_CONST_TEXELS + BIAS_SLOPE_TEXELS * tan(acos(nl))) texels,
//! slope term clamped to [0,4], converted to normalized depth by the ortho depth range.
use super::mesh::Matrix;
use super::vertex_stream::unit;

pub const BIAS_CONST_TEXELS:f64=1.0;
pub const BIAS_SLOPE_TEXELS:f64=2.0;
pub const NORMAL_OFFSET_TEXELS:f64=1.0;
pub const PCF_RADIUS:i32=1; // 3x3 taps; 0 = single-tap hard shadow (matches raycast reference)

#[derive(Clone,Debug)]
pub struct DirectionalShadowMap{
 /// world -> light frame (x right, y up, z along travel direction, origin at light)
 pub view:Matrix,
 /// ortho bounds in the light frame: [min,max] per axis; depth = (z-minz)/(maxz-minz) in [0,1]
 pub bounds:[[f64;3];2],
 pub width:usize,pub height:usize,
 /// nearest occluder depth per texel, normalized [0,1]; f64::INFINITY = no occluder
 pub depth:Vec<f64>,
}

fn mv(m:&Matrix,v:[f64;4])->[f64;4]{std::array::from_fn(|i|m[i][0]*v[0]+m[i][1]*v[1]+m[i][2]*v[2]+m[i][3]*v[3])}
fn sub(a:[f64;3],b:[f64;3])->[f64;3]{[a[0]-b[0],a[1]-b[1],a[2]-b[2]]}
fn cross(a:[f64;3],b:[f64;3])->[f64;3]{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}

/// Light frame + ortho bounds covering the scene AABB. Deterministic: no sampling.
pub fn directional_light_space(travel_direction:[f64;3],aabb:[[f64;3];2])->Result<(Matrix,[[f64;3];2]),String>{
 if aabb.iter().flatten().any(|v|!v.is_finite()){return Err("finite scene bounds required".into());}
 let f=unit(travel_direction)?; // light looks along the travel direction
 let seed=if f[1].abs()<0.99{[0.,1.,0.]}else{[1.,0.,0.]};
 let r=unit(cross(f,seed))?;let u=cross(r,f);
 // frame origin: AABB center pulled back along -f by the AABB radius (covers everything)
 let c=std::array::from_fn(|j|(aabb[0][j]+aabb[1][j])/2.);
 let rad=(0..3).map(|j|(aabb[1][j]-aabb[0][j])/2.).fold(0.,|s,d|s+d*d).sqrt();
 let eye=sub(c,f.map(|v|v*(rad+1e-6)));
 let view=[[r[0],r[1],r[2],-(r[0]*eye[0]+r[1]*eye[1]+r[2]*eye[2])],
           [u[0],u[1],u[2],-(u[0]*eye[0]+u[1]*eye[1]+u[2]*eye[2])],
           [f[0],f[1],f[2],-(f[0]*eye[0]+f[1]*eye[1]+f[2]*eye[2])],
           [0.,0.,0.,1.]];
 let(mut lo,mut hi)=([f64::INFINITY;3],[f64::NEG_INFINITY;3]);
 for k in 0..8{let p=[if k&1==0{aabb[0][0]}else{aabb[1][0]},if k&2==0{aabb[0][1]}else{aabb[1][1]},if k&4==0{aabb[0][2]}else{aabb[1][2]}];
  let q=mv(&view,[p[0],p[1],p[2],1.]);for j in 0..3{lo[j]=lo[j].min(q[j]);hi[j]=hi[j].max(q[j]);}}
 let pad=1e-3*(1.+rad); // keep min-depth texels off the exact near plane
 lo[2]-=pad;hi[2]+=pad;
 Ok((view,[lo,hi]))}

/// world -> (u,v,depth) in shadow-map space; None when outside [0,1] on u/v (treated lit).
pub fn shadow_uvz(map:&DirectionalShadowMap,world:[f64;3])->Option<(f64,f64,f64)>{
 let q=mv(&map.view,[world[0],world[1],world[2],1.]);
 let u=(q[0]-map.bounds[0][0])/(map.bounds[1][0]-map.bounds[0][0]);
 let v=(q[1]-map.bounds[0][1])/(map.bounds[1][1]-map.bounds[0][1]);
 let d=(q[2]-map.bounds[0][2])/(map.bounds[1][2]-map.bounds[0][2]);
 if !(0.0..=1.0).contains(&u)||!(0.0..=1.0).contains(&v){None}else{Some((u,v,d))}
}

/// World size of one depth texel along the light direction is implicit; we expose the
/// SCREEN texel in world units (max of the two axes) to scale the normal offset, and the
/// normalized-depth value of one texel for the depth bias.
fn texel_scales(map:&DirectionalShadowMap)->(f64,f64){
 let wx=(map.bounds[1][0]-map.bounds[0][0])/map.width as f64;
 let wy=(map.bounds[1][1]-map.bounds[0][1])/map.height as f64;
 let world=wx.max(wy);
 let depth_norm=world/(map.bounds[1][2]-map.bounds[0][2]); // 1 texel ~ 1 texel of depth slope scale
 (world,depth_norm)}

/// Pinned bias contract: normal offset (world) + normalized depth bias from nl in (0,1].
pub fn bias(map:&DirectionalShadowMap,n_dot_l:f64)->(f64,f64){
 let(world,depth_norm)=texel_scales(map);
 let slope=((1.-n_dot_l*n_dot_l).max(0.)).sqrt()/n_dot_l.max(1e-3);
 let slope=slope.min(4.);
 (NORMAL_OFFSET_TEXELS*world,(BIAS_CONST_TEXELS+BIAS_SLOPE_TEXELS*slope)*depth_norm)}

/// PCF visibility in [0,1]: fraction of taps where the (biased) receiver depth is nearer
 /// than the stored occluder depth. radius 0 = single-tap hard shadow.
pub fn visibility(map:&DirectionalShadowMap,world:[f64;3],normal:[f64;3],n_dot_l:f64,radius:i32)->f64{
 let(no,db)=bias(map,n_dot_l);
 let p=std::array::from_fn(|j|world[j]+normal[j]*no);
 let Some((u,v,d))=shadow_uvz(map,p)else{return 1.}; // outside the map: lit (scene AABB covers casters+receivers)
 let d=d-db;
 if !d.is_finite(){return 1.;}
 let(du,dv)=(1./map.width as f64,1./map.height as f64);
 let(mut lit,mut taps)=(0usize,0usize);
 for oy in -radius..=radius{for ox in -radius..=radius{
  let(uu,vv)=(u+ox as f64*du,v+oy as f64*dv);
  if !(0.0..=1.0).contains(&uu)||!(0.0..=1.0).contains(&vv){lit+=1;taps+=1;continue;} // off-map taps count lit
  let x=(uu*map.width as f64).min(map.width as f64-1e-9) as usize;
  let y=(vv*map.height as f64).min(map.height as f64-1e-9) as usize;
  let stored=map.depth[y*map.width+x];
  if stored==f64::INFINITY||d<=stored{lit+=1;}
  taps+=1;}}
 lit as f64/taps as f64}

// Rotated-disk soft shadows (PL1b). Tap positions are the D3D standard MSAA sample points
// with their Gaussian PDF weights, and per-pixel rotation by interleaved gradient noise -
// both adapted from Bevy (github.com/bevyengine/bevy,
// crates/bevy_pbr/src/render/shadow_sampling.wgsl, MIT OR Apache-2.0). Bevy cites
// https://learn.microsoft.com/en-us/windows/win32/api/d3d11/ne-d3d11-d3d11_standard_multisample_quality_levels
// for the positions. Weights are NOT normalized in Bevy's table (they sum ~1.0); we normalize.
pub const DISK_TAPS:[(f64,f64);8]=[(0.125,-0.375),(-0.125,0.375),(0.625,0.125),(-0.375,-0.625),(-0.625,0.625),(-0.875,-0.125),(0.375,0.875),(0.875,-0.875)];
pub const DISK_WEIGHTS:[f64;8]=[0.157112,0.157112,0.138651,0.130251,0.114946,0.110269,0.098503,0.093156];
/// Jimenez 2014 interleaved gradient noise - stable per-pixel rotation seed (no temporal crawl in stills).
pub fn interleaved_gradient_noise(x:usize,y:usize)->f64{let v=52.9829189*(0.06711056*x as f64+0.00583715*y as f64).fract();v.fract()}
/// visibility() with a rotated Gaussian disk instead of the 3x3 grid: same bias contract,
/// same compare, dithered soft edges. radius_texels scales the disk in map texels.
pub fn visibility_disk(map:&DirectionalShadowMap,world:[f64;3],normal:[f64;3],n_dot_l:f64,radius_texels:f64,px:usize,py:usize)->f64{
 if n_dot_l<=0.||radius_texels<=0.{return if n_dot_l<=0.{0.}else{visibility(map,world,normal,n_dot_l,0)};}
 let(no,dnb)=bias(map,n_dot_l);
 let p=std::array::from_fn(|j|world[j]+normal[j]*no);
 let Some((u,v,d))=shadow_uvz(map,p) else{return 1.};
 let d=d-dnb;
 let angle=interleaved_gradient_noise(px,py)*std::f64::consts::TAU;let(sn,cs)=angle.sin_cos();
 let(du,dv)=(1./map.width as f64,1./map.height as f64);
 let(mut lit,mut wsum)=(0.,0.);
 for k in 0..8{let(tx,ty)=DISK_TAPS[k];let rx=cs*tx-sn*ty;let ry=sn*tx+cs*ty;
  let stored=sample_map(map,u+rx*radius_texels*du,v+ry*radius_texels*dv);
  lit+=DISK_WEIGHTS[k]*if d<=stored{1.}else{0.};wsum+=DISK_WEIGHTS[k];}
 lit/wsum}
fn sample_map(map:&DirectionalShadowMap,u:f64,v:f64)->f64{
 if !(0.0..=1.0).contains(&u)||!(0.0..=1.0).contains(&v){return f64::INFINITY;} // off-map taps count lit
 // pinned texel addressing: identical to visibility()/raster_depth (u*width truncated)
 let x=(u*map.width as f64).min(map.width as f64-1e-9) as usize;
 let y=(v*map.height as f64).min(map.height as f64-1e-9) as usize;
 map.depth[y*map.width+x]}

/// CPU light-POV depth rasterizer: world-space triangles -> nearest normalized depth.
/// Same edge-function convention as lit_raster (top-left fill via positive barycentrics).
pub fn raster_depth(positions:&[[f64;3]],indices:&[[u32;3]],map:&mut DirectionalShadowMap)->Result<(),String>{
 if map.width==0||map.height==0||map.width>4096||map.height>4096{return Err("shadow map 1..4096 per axis".into());}
 if map.depth.len()!=map.width*map.height{return Err("shadow map depth buffer size mismatch".into());}
 for d in map.depth.iter_mut(){*d=f64::INFINITY;}
 raster_depth_into(positions,indices,map)}
/// Accumulate one draw's casters into the map WITHOUT clearing (nearest wins across calls).
/// raster_depth = clear + into; build multi-draw maps with one clear then into() per draw.
pub fn raster_depth_into(positions:&[[f64;3]],indices:&[[u32;3]],map:&mut DirectionalShadowMap)->Result<(),String>{
 if map.width==0||map.height==0||map.width>4096||map.height>4096{return Err("shadow map 1..4096 per axis".into());}
 if map.depth.len()!=map.width*map.height{return Err("shadow map depth buffer size mismatch".into());}
 let edge=|a:[f64;2],b:[f64;2],x:f64,y:f64|(b[0]-a[0])*(y-a[1])-(b[1]-a[1])*(x-a[0]);
 for t in indices{
  let mut p=[[0.;3];3];
  for k in 0..3{let w=positions[t[k] as usize];let q=mv(&map.view,[w[0],w[1],w[2],1.]);
   p[k]=[(q[0]-map.bounds[0][0])/(map.bounds[1][0]-map.bounds[0][0])*map.width as f64,
         (q[1]-map.bounds[0][1])/(map.bounds[1][1]-map.bounds[0][1])*map.height as f64,
         (q[2]-map.bounds[0][2])/(map.bounds[1][2]-map.bounds[0][2])];}
  let area=edge([p[0][0],p[0][1]],[p[1][0],p[1][1]],p[2][0],p[2][1]);
  if !area.is_finite(){return Err("shadow triangle area overflow".into());}
  if area==0.{continue;}
  let(a,b,c)=([p[0][0],p[0][1]],[p[1][0],p[1][1]],[p[2][0],p[2][1]]);
  // rasterize both windings: casters are single- or double-sided, shadow depth is winding-agnostic
  let x0=p.iter().map(|q|q[0]).fold(f64::INFINITY,f64::min).floor().clamp(0.,map.width as f64) as usize;
  let x1=p.iter().map(|q|q[0]).fold(f64::NEG_INFINITY,f64::max).ceil().clamp(0.,map.width as f64) as usize;
  let y0=p.iter().map(|q|q[1]).fold(f64::INFINITY,f64::min).floor().clamp(0.,map.height as f64) as usize;
  let y1=p.iter().map(|q|q[1]).fold(f64::NEG_INFINITY,f64::max).ceil().clamp(0.,map.height as f64) as usize;
  for y in y0..y1{for x in x0..x1{let(sx,sy)=(x as f64+0.5,y as f64+0.5);
   let(w0,w1,w2)=(edge(b,c,sx,sy)/area,edge(c,a,sx,sy)/area,edge(a,b,sx,sy)/area);
   if (w0<0.||w1<0.||w2<0.)&&(w0>0.||w1>0.||w2>0.){continue;} // outside (either winding)
   let d=w0*p[0][2]+w1*p[1][2]+w2*p[2][2];
   if !d.is_finite(){continue;}
   let id=y*map.width+x;if d<map.depth[id]{map.depth[id]=d;}}}
 }
 Ok(())}

/// Combined world-space AABB over draw position arrays.
pub fn scene_aabb(draws:&[&[[f64;3]]])->Result<[[f64;3];2],String>{
 let(mut lo,mut hi)=([f64::INFINITY;3],[f64::NEG_INFINITY;3]);let mut n=0usize;
 for ps in draws.iter().copied(){for p in ps{if p.iter().any(|v|!v.is_finite()){return Err("finite positions required".into());}for j in 0..3{lo[j]=lo[j].min(p[j]);hi[j]=hi[j].max(p[j]);}n+=1;}}
 if n==0{return Err("at least one vertex required".into());}
 Ok([lo,hi])}

#[cfg(test)]mod tests{use super::*;
 fn box_aabb()->[[f64;3];2]{[[-1.;3],[1.;3]]}
 #[test]fn light_space_covers_aabb_and_is_deterministic(){
  let(v1,b1)=directional_light_space([0.3,-0.5,1.],box_aabb()).unwrap();
  let(v2,b2)=directional_light_space([0.3,-0.5,1.],box_aabb()).unwrap();
  assert_eq!(v1,v2);assert_eq!(b1,b2);
  let map=DirectionalShadowMap{view:v1,bounds:b1,width:4,height:4,depth:vec![f64::INFINITY;16]};
  for k in 0..8{let p=[if k&1==0{-1.}else{1.},if k&2==0{-1.}else{1.},if k&4==0{-1.}else{1.}];
   let(u,v,d)=shadow_uvz(&map,p).expect("every AABB corner inside the map");
   assert!((0.0..=1.0).contains(&u)&&(0.0..=1.0).contains(&v)&&(0.0..=1.0).contains(&d),"corner {k} -> {u} {v} {d}");}
 }
 #[test]fn bias_contract_zero_front_on_grows_grazing(){
  let(view,bounds)=directional_light_space([0.,0.,1.],box_aabb()).unwrap();
  let map=DirectionalShadowMap{view,bounds,width:64,height:64,depth:vec![f64::INFINITY;64*64]};
  let(no0,db0)=bias(&map,1.0);let(_,db1)=bias(&map,0.5);let(_,db2)=bias(&map,0.1);
  assert!(no0>0.,"normal offset always on");
  assert!(db0>0.&&db1>db0&&db2>db1,"bias grows with grazing: {db0} {db1} {db2}");
  let(_,dbt)=bias(&map,0.0);let(_,dbc)=bias(&map,1e-9);assert_eq!(dbt,dbc,"slope clamped: nl=0 equals nl~0");}
 #[test]fn visibility_hard_and_pcf_penumbra(){
  // light travels +z: smaller world z = nearer the light. Occluder quad at z=0, half the map (x<0).
  let(view,bounds)=directional_light_space([0.,0.,1.],[[-2.,-2.,-1.],[2.,2.,1.]]).unwrap();
  let mut map=DirectionalShadowMap{view,bounds,width:64,height:64,depth:vec![f64::INFINITY;64*64]};
  let pos=vec![[-2.,-2.,0.],[0.,-2.,0.],[0.,2.,0.],[-2.,2.,0.]];
  let idx=vec![[0u32,1,2],[0,2,3]];
  raster_depth(&pos,&idx,&mut map).unwrap();
  let behind=visibility(&map,[-1.,0.,0.5],[0.,0.,-1.],1.0,0);let front=visibility(&map,[-1.,0.,-0.5],[0.,0.,-1.],1.0,0);
  assert_eq!((behind,front),(0.0,1.0),"hard shadow: behind occluder {behind}, in front {front}");
  let behind_g=visibility(&map,[-1.,0.,0.5],[0.,0.,-1.],0.8,0);assert_eq!(behind_g,0.0,"bias must not erase a real shadow at moderate incidence");
  // documented tradeoff: at grazing incidence (nl=0.3) the pinned slope bias CAN erase a
  // shallow shadow on a small map - the acne/peter-panning tradeoff is deliberate.
  assert_eq!(visibility(&map,[-1.,0.,0.5],[0.,0.,-1.],0.3,0),1.0,"grazing bias erasure is the pinned contract (documented)");
  // no self-shadow acne: a receiver ON the occluder plane stays lit under its own depth
  let on=visibility(&map,[-1.,0.,0.],[0.,0.,-1.],1.0,0);assert_eq!(on,1.0,"bias prevents acne on the occluder plane");
  // open half of the map: fully lit
  let open=visibility(&map,[1.,0.,0.5],[0.,0.,-1.],1.0,0);assert_eq!(open,1.0,"no occluder texel -> lit");
  // penumbra at the occluder silhouette (x=0): hard is binary, PCF straddles
  let edge_x=0.5/64.; // half a texel inside the covered half
  let hard=visibility(&map,[-edge_x,0.,0.5],[0.,0.,-1.],1.0,0);
  let soft=visibility(&map,[-edge_x,0.,0.5],[0.,0.,-1.],1.0,1);
  assert_eq!(hard,0.0,"single tap binary at the edge");
  assert!(soft>0.0&&soft<1.0,"PCF penumbra strictly between: {soft}");
 }
 #[test]fn raster_depth_nearest_wins_and_empty_is_infinite(){
  let(view,bounds)=directional_light_space([0.,0.,1.],[[-2.;3],[2.;3]]).unwrap();
  let mut map=DirectionalShadowMap{view,bounds,width:32,height:32,depth:vec![f64::INFINITY;32*32]};
  raster_depth(&[[0.,0.,0.],[0.1,0.,0.],[0.,0.1,0.]],&[[0u32,1,2]],&mut map).unwrap();
  assert!(map.depth.iter().all(|d|*d==f64::INFINITY),"sub-texel triangle may cover nothing; all empty stays infinite");
  // two overlapping quads: nearer (smaller light-z) must win
  let(view,bounds)=directional_light_space([0.,0.,1.],[[-2.;3],[2.;3]]).unwrap();
  let mut map=DirectionalShadowMap{view,bounds,width:32,height:32,depth:vec![f64::INFINITY;32*32]};
  let far=vec![[-1.,-1.,0.5],[1.,-1.,0.5],[1.,1.,0.5],[-1.,1.,0.5]];
  let near=vec![[-1.,-1.,-0.5],[1.,-1.,-0.5],[1.,1.,-0.5],[-1.,1.,-0.5]];
  let idx=vec![[0u32,1,2],[0,2,3]];
  raster_depth(&far,&idx,&mut map).unwrap();
  raster_depth(&near,&idx,&mut map).unwrap();
  let d=*map.depth.iter().filter(|d|**d!=f64::INFINITY).next().expect("quads rasterized");
  let(_,_,expect)=shadow_uvz(&map,[0.,0.,-0.5]).unwrap(); // same transform the rasterizer applies
  assert!((d-expect).abs()<1e-12,"nearest occluder depth {d} vs {expect}");
 }

}
