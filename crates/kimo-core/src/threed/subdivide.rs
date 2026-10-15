//! Uniform Catmull-Clark subdivision through opensubdiv-rs 0.1.3 (MIT; faithful pure-Rust port of
//! Pixar's OpenSubdiv). No copied tables/constants: mask oracles in the tests come from the published
//! Catmull-Clark limit formula applied to hand-built fixtures. The crate is never its own oracle.
//! Any panic inside the foreign crate is caught and returned as Err so callers can fall back.
use super::mesh::Mesh;
use opensubdiv_rs::far::{PrimvarRefiner,TopologyDescriptor,TopologyRefinerFactory,UniformOptions};
use opensubdiv_rs::sdc;

pub struct Subdivided{pub mesh:Mesh,pub base_vertices:usize,pub final_vertices:usize,pub final_triangles:usize,pub interior_edges_two_manifold:bool,pub boundary_edges:usize,pub euler_characteristic:i64,pub zero_area_triangles:usize}

fn weld(mesh:&Mesh)->Result<(Vec<[f64;3]>,Vec<[u32;3]>),String>{mesh.validate()?;let mut ids=std::collections::HashMap::new();let mut pos=Vec::new();let mut remap=Vec::with_capacity(mesh.positions.len());for p in &mesh.positions{let n=ids.len();let id=*ids.entry(p.map(|v|v.to_bits())).or_insert(n);if id==pos.len(){pos.push(*p);}remap.push(id as u32);}let mut tris=Vec::with_capacity(mesh.indices.len());for t in &mesh.indices{let r=t.map(|i|remap[i as usize]);if r[0]==r[1]||r[1]==r[2]||r[2]==r[0]{return Err("degenerate triangle after position welding".into());}tris.push(r);}Ok((pos,tris))}

fn edge_stats(tris:&[[u32;3]])->(usize,usize,bool){let mut e=std::collections::HashMap::new();for t in tris{for k in 0..3{let(a,b)=(t[k],t[(k+1)%3]);*e.entry(if a<b{(a,b)}else{(b,a)}).or_insert(0usize)+=1;}}let boundary=e.values().filter(|c|**c==1).count();let two_manifold=e.values().all(|c|*c<=2)&&(boundary==0||e.values().all(|c|*c==1||*c==2));(e.len(),boundary,two_manifold)}

/// Uniformly subdivide a raw triangle mesh with Catmull-Clark. Input is welded by exact position
/// (seam-duplicated vertices would tear the surface). `creases`: endpoint position pairs of base
/// edges (welded lookup), each carrying `crease_weight` (0..=10; 10 = fully sharp).
pub fn catmull_clark(mesh:&Mesh,levels:u32,creases:&[[[f64;3];2]],crease_weight:f32)->Result<Subdivided,String>{
 if !(1..=6).contains(&levels){return Err("subdivision levels 1..6".into());}
 if !crease_weight.is_finite()||!(0.0..=10.0).contains(&crease_weight){return Err("crease weight finite 0..10".into());}
 if creases.len()>4096{return Err("max 4096 crease edges".into());}
 for c in creases{if !c.iter().flatten().all(|v|v.is_finite()&&v.abs()<=1e12){return Err("crease endpoints finite".into());}}
 let(pos,tris)=weld(mesh)?;
 let(base_edges,base_boundary,_)=edge_stats(&tris);
 let vpf:Vec<usize>=vec![3;tris.len()];let flat:Vec<u32>=tris.iter().flatten().copied().collect();
 let mut pairs=Vec::with_capacity(creases.len());
 for c in creases{let find=|p:[f64;3]|pos.iter().position(|q|q.map(|v|v.to_bits())==p.map(|v|v.to_bits()));let(a,b)=(find(c[0]),find(c[1]));match(a,b){(Some(a),Some(b))=>{let(a,b)=(a as u32,b as u32);if !tris.iter().any(|t|{let s:std::collections::HashSet<u32>=t.iter().copied().collect();s.contains(&a)&&s.contains(&b)}){return Err("crease endpoints do not share a triangle edge".into());}pairs.push([a,b]);},_=>return Err("crease endpoint not on a welded mesh vertex".into())}}
 let weights=vec![crease_weight;pairs.len()];
 let positions:Vec<[f32;3]>=pos.iter().map(|p|p.map(|v|v as f32)).collect();
 let lifted=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||->Result<(Vec<[f32;3]>,Vec<[u32;3]>,i64,usize),String>{
  let desc=TopologyDescriptor::new(positions.len(),&vpf,&flat).with_creases(&pairs,&weights);
  let mut refiner=TopologyRefinerFactory::create(desc,sdc::SchemeType::Catmark,sdc::Options::default()).map_err(|e|format!("topology rejected: {e:?}"))?;
  refiner.refine_uniform(UniformOptions::new(levels as usize));
  let pr=PrimvarRefiner::new(&refiner);let mut src=positions.clone();
  for lvl in 1..=refiner.max_level(){let mut dst=vec![[0f32;3];refiner.level(lvl).num_vertices()];pr.interpolate(lvl,&src,&mut dst);src=dst;}
  let top=refiner.level(refiner.max_level());let mut out=Vec::with_capacity(top.num_faces()*2);
  for f in 0..top.num_faces(){let fv=top.face_vertices(f);if fv.len()<3{return Err("refined face with <3 vertices".into());}for k in 1..fv.len()-1{out.push([fv[0],fv[k],fv[k+1]]);}}
  for v in &src{if !v.iter().all(|c|c.is_finite()){return Err("non-finite refined position (instability)".into());}}
  Ok((src,out,0i64,0usize))}));
 let(rpos,rtris,_,_)=match lifted{Ok(r)=>r?,Err(_)=>return Err("opensubdiv-rs panicked during refinement; fall back to native CC rules".into())};
 let(edges,boundary,two_manifold)=edge_stats(&rtris);
 if !two_manifold{return Err("refined mesh is not 2-manifold (edge with >2 faces)".into());}
 if boundary!=base_boundary*(1usize<<levels){return Err(format!("boundary edge count {boundary} != base {base_boundary} * 2^levels")); }
 let euler=rpos.len() as i64-edges as i64+rtris.len() as i64;
 let base_euler=pos.len() as i64-base_edges as i64+tris.len() as i64;
 if euler!=base_euler{return Err(format!("Euler characteristic changed {base_euler} -> {euler}"));}
 let mut zero_area=0;let positions64:Vec<[f64;3]>=rpos.iter().map(|p|p.map(|v|v as f64)).collect();
 for t in &rtris{let[a,b,c]=t.map(|i|positions64[i as usize]);let ab=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];let ac=[c[0]-a[0],c[1]-a[1],c[2]-a[2]];let n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];if n[0]*n[0]+n[1]*n[1]+n[2]*n[2]<1e-24{zero_area+=1;}}
 let final_triangles=rtris.len();
 Ok(Subdivided{mesh:Mesh{positions:positions64,indices:rtris},base_vertices:pos.len(),final_vertices:rpos.len(),final_triangles,interior_edges_two_manifold:two_manifold,boundary_edges:boundary,euler_characteristic:euler,zero_area_triangles:zero_area})
}

