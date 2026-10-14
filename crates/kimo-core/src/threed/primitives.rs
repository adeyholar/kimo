//! Original raw parametric geometry. No external implementation copied.
use serde::{Serialize,Deserialize};
use super::mesh::Mesh;
/// Interleaved GPU-ready ABI: offsets0,12,24,32, stride48 bytes.
/// tangent.w handedness: bitangent = w * cross(normal,tangent.xyz).
#[repr(C)]#[derive(Clone,Copy,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Vertex{pub position:[f32;3],pub normal:[f32;3],pub uv:[f32;2],pub tangent:[f32;4]}
const _:()=assert!(std::mem::size_of::<Vertex>()==48,"Vertex ABI stride must stay 48 bytes");
const _:()=assert!(std::mem::align_of::<Vertex>()==4,"Vertex alignment must stay 4 (all-f32 layout)");
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(tag="kind",rename_all="snake_case",deny_unknown_fields)]
pub enum Primitive{Cube{size:[f64;3],subdivisions:u32},Sphere{radius:f64,longitude_slices:u32,latitude_slices:u32},Torus{major_radius:f64,minor_radius:f64,radial_segments:u32,tubular_segments:u32}}
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Geometry{pub primitive:Primitive,pub vertices:Vec<Vertex>,pub indices:Vec<[u32;3]>}
fn dot(a:[f64;3],b:[f64;3])->f64{a.iter().zip(b).map(|(a,b)|a*b).sum()}
fn cross(a:[f64;3],b:[f64;3])->[f64;3]{[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]]}
fn sub(a:[f64;3],b:[f64;3])->[f64;3]{std::array::from_fn(|i|a[i]-b[i])}
fn positive(x:f64)->bool{x.is_finite()&&(1e-6..=1e6).contains(&x)}
impl Primitive{
 pub fn counts(&self)->Result<(usize,usize),String>{match self{
 Self::Cube{size,subdivisions:n}=>{if size.iter().any(|v|!positive(*v))||!(1..=128).contains(n){return Err("cube sizes1e-6..1e6 subdivisions1..128".into());}let n=*n as usize;Ok((6*(n+1)*(n+1),12*n*n))},
 Self::Sphere{radius,longitude_slices:s,latitude_slices:r}=>{if !positive(*radius)||!(3..=256).contains(s)||!(2..=256).contains(r){return Err("sphere radius1e-6..1e6 longitude3..256 latitude2..256".into());}let(s,r)=(*s as usize,*r as usize);Ok((2*s+(r-1)*(s+1),2*s*(r-1)))},
 Self::Torus{major_radius,minor_radius,radial_segments:s,tubular_segments:r}=>{if !positive(*major_radius)||!positive(*minor_radius)||minor_radius>=major_radius||!(3..=256).contains(s)||!(3..=256).contains(r){return Err("ring torus1e-6<=minor<major<=1e6 radial/tubular3..256".into());}let(s,r)=(*s as usize,*r as usize);Ok(((s+1)*(r+1),2*s*r))}
 }}
}
impl Geometry{
 pub fn mesh(&self)->Mesh{Mesh{positions:self.vertices.iter().map(|v|v.position.map(|v|v as f64)).collect(),indices:self.indices.clone()}}
 pub fn verify(&self)->Result<serde_json::Value,String>{let expected=self.primitive.counts()?;if (self.vertices.len(),self.indices.len())!=expected{return Err("buffers disagree with primitive analytic counts".into());}self.mesh().validate()?;let mut degenerate=0;let mut opposed=0;
 for v in &self.vertices{if v.position.iter().chain(v.normal.iter()).chain(v.uv.iter()).chain(v.tangent.iter()).any(|v|!v.is_finite())||v.uv.iter().any(|v|!(0.0..=1.0).contains(v)){return Err("finite vertex and UV0..1 required".into());}let n=v.normal.map(|v|v as f64);let t=[v.tangent[0] as f64,v.tangent[1] as f64,v.tangent[2] as f64];if (dot(n,n)-1.0).abs()>1e-5||(dot(t,t)-1.0).abs()>1e-5||dot(n,t).abs()>1e-5||v.tangent[3].abs()!=1.0{return Err("unit orthogonal normal/tangent and handedness+/-1 required".into());}}
 for ids in &self.indices{let[a,b,c]=ids.map(|i|self.vertices[i as usize].position.map(|v|v as f64));let n=cross(sub(b,a),sub(c,a));if dot(n,n)==0.0{degenerate+=1;}let avg:[f64;3]=std::array::from_fn(|j|ids.iter().map(|i|self.vertices[*i as usize].normal[j] as f64).sum());if dot(n,avg)<=0.0{opposed+=1;}}
 Ok(serde_json::json!({"primitive":self.primitive,"vertex_count":self.vertices.len(),"triangle_count":self.indices.len(),"analytic_counts_match":true,"dimensions":self.mesh().bounds()?,"stride_bytes":std::mem::size_of::<Vertex>(),"offsets_bytes":[0,12,24,32],"layout":["position_vec3","normal_vec3","uv_vec2","tangent_vec4"],"ccw_outward":opposed==0,"zero_area_triangles":degenerate,"opposed_winding_triangles":opposed,"unit_normals":true,"uv_range":[0,1],"tangent_sanity":true,"watertightness_verified":false}))}
}
fn vertex(p:[f64;3],n:[f64;3],uv:[f64;2],t:[f64;3],hand:f32)->Vertex{Vertex{position:p.map(|v|v as f32),normal:n.map(|v|v as f32),uv:uv.map(|v|v as f32),tangent:[t[0] as f32,t[1] as f32,t[2] as f32,hand]}}
fn angle(i:u32,n:u32)->f64{if i==n{0.0}else{std::f64::consts::TAU*i as f64/n as f64}}
pub fn generate(p:&Primitive)->Result<Geometry,String>{let(v,f)=p.counts()?;crate::guard::reserve(v*48+f*12);let mut g=Geometry{primitive:p.clone(),vertices:Vec::with_capacity(v),indices:Vec::with_capacity(f)};match p{
 Primitive::Cube{size,subdivisions:s}=>{let n=*s;for axis in 0..3{for sign in [-1.,1.]{crate::guard::check();let u=(axis+1)%3;let v=(axis+2)%3;let base=g.vertices.len() as u32;for j in 0..=n{for i in 0..=n{let mut pos=[0.;3];pos[axis]=sign*size[axis]*0.5;pos[u]=(i as f64/n as f64-0.5)*size[u];pos[v]=(j as f64/n as f64-0.5)*size[v];let mut normal=[0.;3];normal[axis]=sign;let mut tangent=[0.;3];tangent[u]=1.;g.vertices.push(vertex(pos,normal,[i as f64/n as f64,j as f64/n as f64],tangent,sign as f32));}}for j in 0..n{for i in 0..n{let a=base+j*(n+1)+i;let b=a+1;let c=a+n+1;let d=c+1;for tri in [[a,b,d],[a,d,c]]{g.indices.push(if sign>0.{tri}else{[tri[0],tri[2],tri[1]]});}}}}}},
 Primitive::Sphere{radius,longitude_slices:s,latitude_slices:r}=>{let(s,r)=(*s,*r);for i in 0..s{let th=std::f64::consts::TAU*(i as f64+0.5)/s as f64;g.vertices.push(vertex([0.,*radius,0.],[0.,1.,0.],[(i as f64+0.5)/s as f64,0.],[-th.sin(),0.,th.cos()],1.));}
 for j in 1..r{crate::guard::check();let ph=std::f64::consts::PI*j as f64/r as f64;for i in 0..=s{let th=angle(i,s);let n=[ph.sin()*th.cos(),ph.cos(),ph.sin()*th.sin()];g.vertices.push(vertex(n.map(|v|v*radius),n,[i as f64/s as f64,j as f64/r as f64],[-th.sin(),0.,th.cos()],1.));}}
 let south=g.vertices.len() as u32;for i in 0..s{let th=std::f64::consts::TAU*(i as f64+0.5)/s as f64;g.vertices.push(vertex([0.,-*radius,0.],[0.,-1.,0.],[(i as f64+0.5)/s as f64,1.],[-th.sin(),0.,th.cos()],1.));}
 for i in 0..s{g.indices.push([i,s+i+1,s+i]);for j in 0..r-2{let a=s+j*(s+1)+i;let b=a+1;let c=a+s+1;let d=c+1;g.indices.push([a,b,c]);g.indices.push([b,d,c]);}let a=s+(r-2)*(s+1)+i;g.indices.push([a,a+1,south+i]);}},
 Primitive::Torus{major_radius:a,minor_radius:b,radial_segments:s,tubular_segments:r}=>{let(s,r)=(*s,*r);for i in 0..=s{crate::guard::check();let th=angle(i,s);for j in 0..=r{let ph=angle(j,r);let rad=a+b*ph.cos();g.vertices.push(vertex([rad*th.cos(),b*ph.sin(),rad*th.sin()],[ph.cos()*th.cos(),ph.sin(),ph.cos()*th.sin()],[i as f64/s as f64,j as f64/r as f64],[-th.sin(),0.,th.cos()],-1.));}}for i in 0..s{for j in 0..r{let a=i*(r+1)+j;let b=a+r+1;let c=a+1;let d=b+1;g.indices.push([a,c,b]);g.indices.push([b,c,d]);}}}
 }g.verify()?;Ok(g)}
#[cfg(test)]mod tests{use super::*;
 #[test]fn layout_and_analytic_invariants(){assert_eq!(std::mem::size_of::<Vertex>(),48);for n in [1,2,7]{let p=Primitive::Cube{size:[1.,2.,3.],subdivisions:n};let g=generate(&p).unwrap();assert_eq!((g.vertices.len(),g.indices.len()),(6*(n as usize+1).pow(2),12*(n as usize).pow(2)));check(&g);}for(s,r)in [(3,2),(8,4),(32,16)]{check(&generate(&Primitive::Sphere{radius:1.,longitude_slices:s,latitude_slices:r}).unwrap());check(&generate(&Primitive::Torus{major_radius:2.,minor_radius:0.5,radial_segments:s,tubular_segments:r.max(3)}).unwrap());}}
 fn check(g:&Geometry){let v=g.verify().unwrap();assert_eq!(v["zero_area_triangles"],0);assert_eq!(v["opposed_winding_triangles"],0);assert_eq!(v["ccw_outward"],true);}
 #[test]fn seam_vertices_match_position_normal_tangent_but_uv_abuts(){let g=generate(&Primitive::Sphere{radius:1.,longitude_slices:8,latitude_slices:4}).unwrap();for j in 0..3{let a=g.vertices[8+j*9];let b=g.vertices[8+j*9+8];assert_eq!(a.position,b.position);assert_eq!(a.normal,b.normal);assert_eq!(a.tangent,b.tangent);assert_eq!(a.uv[0],0.);assert_eq!(b.uv[0],1.);}}
 #[test]fn polar_caps_have_no_zero_area_faces(){let g=generate(&Primitive::Sphere{radius:1.,longitude_slices:16,latitude_slices:8}).unwrap();check(&g);assert_eq!(g.indices.len(),2*16*7);for v in g.vertices.iter().take(16){assert_eq!(v.position,[0.,1.,0.]);assert!(v.uv[0]>0.&&v.uv[0]<1.);}}
 #[test]fn invalid_dimension_or_segments_rejected(){assert!(generate(&Primitive::Cube{size:[f64::NAN,1.,1.],subdivisions:1}).is_err());assert!(generate(&Primitive::Sphere{radius:1.,longitude_slices:2,latitude_slices:2}).is_err());assert!(generate(&Primitive::Torus{major_radius:1.,minor_radius:1.,radial_segments:8,tubular_segments:8}).is_err());}
 #[test]fn sphere_pole_tangents_finite_unit_and_ring_uv_monotonic(){let(s,r)=(16u32,8u32);let g=generate(&Primitive::Sphere{radius:1.,longitude_slices:s,latitude_slices:r}).unwrap();for v in g.vertices.iter().take(s as usize).chain(g.vertices.iter().skip(g.vertices.len()-s as usize)){let t=v.tangent;let l=(t[0]*t[0]+t[1]*t[1]+t[2]*t[2]).sqrt();assert!(t.iter().take(3).all(|v|v.is_finite())&&l>0.0&&(l-1.0).abs()<1e-5,"pole tangent finite non-zero unit");}for j in 1..r{for i in 0..=s{let v=&g.vertices[(s+(j-1)*(s+1)+i) as usize];assert!((v.uv[0]-i as f32/s as f32).abs()<1e-6,"u increases 0..1 along ring: no reverse wrap");}}}
 #[test]fn cube_subdivided_shared_edges_no_cracks_or_t_junctions(){let size=[1.,2.,3.];let g=generate(&Primitive::Cube{size,subdivisions:3}).unwrap();let mut counts=std::collections::HashMap::new();for v in &g.vertices{*counts.entry(v.position.map(|v|v.to_bits())).or_insert(0u32)+=1;}for v in &g.vertices{let extreme=v.position.iter().enumerate().filter(|(j,c)|(c.abs()-(size[*j]/2.0)as f32).abs()<1e-6).count();let want=match extreme{3=>3,2=>2,1=>1,_=>panic!("vertex off cube surface")};assert_eq!(counts[&v.position.map(|v|v.to_bits())],want,"shared edge/corner positions coincide across adjacent faces: no cracks or T-junctions");}}
 #[test]fn torus_ccw_consistent_inner_and_outer(){let g=generate(&Primitive::Torus{major_radius:1.,minor_radius:0.4,radial_segments:16,tubular_segments:8}).unwrap();let v=g.verify().unwrap();assert_eq!(v["opposed_winding_triangles"],0);assert_eq!(v["ccw_outward"],true,"inner and outer tube faces wind CCW consistently");}
}
