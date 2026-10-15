//! Watertight mesh booleans via vendored Manifold 3.5.3 (Apache-2.0, vendor/manifold-3.5.3;
//! Sun fdlibm notice retained) through the narrow C ABI in manifold_shim.cpp. No copied tables
//! or constants: volume oracles in the tests are analytic (axis-aligned boxes) plus the
//! inclusion-exclusion identity. Manifold's kernel is f32 internally with exact predicates;
//! inputs are welded by exact position first and must be closed 2-manifold shells.
use super::mesh::Mesh;
use super::subdivide::{edge_stats,weld};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum BoolOp{Union,Difference,Intersection}

pub struct BooleanResult{pub mesh:Mesh,pub empty:bool,pub volume:f64,pub vertices:usize,pub triangles:usize,pub interior_edges_two_manifold:bool,pub boundary_edges:usize,pub euler_characteristic:i64,pub min_triangle_area:f64}

unsafe extern "C"{fn kimo_manifold_boolean(a_pos:*const f64,a_nv:usize,a_tri:*const u32,a_nt:usize,b_pos:*const f64,b_nv:usize,b_tri:*const u32,b_nt:usize,op:i32,out_pos:*mut *mut f64,out_nv:*mut usize,out_tri:*mut *mut u32,out_nt:*mut usize,out_volume:*mut f64,err:*mut i8,err_len:usize)->i32;fn kimo_manifold_free(p:*mut f64,t:*mut u32);}

fn signed_volume(pos:&[[f64;3]],tris:&[[u32;3]])->f64{let mut v=0.0;for t in tris{let[a,b,c]=t.map(|i|pos[i as usize]);v+=(a[0]*(b[1]*c[2]-b[2]*c[1])-a[1]*(b[0]*c[2]-b[2]*c[0])+a[2]*(b[0]*c[1]-b[1]*c[0]))/6.0;}v}

/// Boolean of two closed 2-manifold shells. `Empty` is a result, not an error (disjoint
/// intersection, identical difference). Output is re-validated: 2-manifold edges, outward CCW
/// winding (positive signed volume), no zero-area slivers at a scale-aware threshold.
pub fn boolean(a:&Mesh,b:&Mesh,op:BoolOp)->Result<BooleanResult,String>{
 let(pa,ta)=weld(a)?;let(pb,tb)=weld(b)?;
 let(ea,ba,ma)=edge_stats(&ta);if !ma||ba!=0{return Err(format!("input A must be a closed 2-manifold shell (boundary edges {ba}, manifold {ma})"));}
 let(eb,bb,mb)=edge_stats(&tb);if !mb||bb!=0{return Err(format!("input B must be a closed 2-manifold shell (boundary edges {bb}, manifold {mb})"));}
 let _=(ea,eb);
 let mut lo=[f64::INFINITY;3];let mut hi=[f64::NEG_INFINITY;3];for p in pa.iter().chain(pb.iter()){for j in 0..3{lo[j]=lo[j].min(p[j]);hi[j]=hi[j].max(p[j]);}}
 let extent=(hi[0]-lo[0]).max(hi[1]-lo[1]).max(hi[2]-lo[2]).max(1e-30);
 let code_op=match op{BoolOp::Union=>0,BoolOp::Difference=>1,BoolOp::Intersection=>2};
 let(mut op_pos,mut op_tri)=(std::ptr::null_mut::<f64>(),std::ptr::null_mut::<u32>());
 let(mut onv,mut ont,mut vol)=(0usize,0usize,0f64);let mut ebuf=[0i8;256];
 let code=unsafe{kimo_manifold_boolean(pa.as_ptr() as *const f64,pa.len(),ta.as_ptr() as *const u32,ta.len(),pb.as_ptr() as *const f64,pb.len(),tb.as_ptr() as *const u32,tb.len(),code_op,&mut op_pos,&mut onv,&mut op_tri,&mut ont,&mut vol,ebuf.as_mut_ptr(),ebuf.len())};
 if code!=0{let msg=ebuf.iter().take_while(|&&c|c!=0).map(|&c|c as u8 as char).collect::<String>();return Err(format!("manifold shim error {code}: {msg}"));}
 if onv==0{return Ok(BooleanResult{mesh:Mesh{positions:Vec::new(),indices:Vec::new()},empty:true,volume:0.0,vertices:0,triangles:0,interior_edges_two_manifold:true,boundary_edges:0,euler_characteristic:0,min_triangle_area:0.0});}
 let positions:Vec<[f64;3]>=unsafe{std::slice::from_raw_parts(op_pos,onv*3)}.chunks_exact(3).map(|c|[c[0],c[1],c[2]]).collect();
 let indices:Vec<[u32;3]>=unsafe{std::slice::from_raw_parts(op_tri,ont*3)}.chunks_exact(3).map(|c|[c[0],c[1],c[2]]).collect();
 unsafe{kimo_manifold_free(op_pos,op_tri)};
 let mesh=Mesh{positions,indices};mesh.validate()?;
 let(_,boundary,two)=edge_stats(&mesh.indices);
 if !two||boundary!=0{return Err(format!("boolean output not closed 2-manifold (boundary {boundary})"));}
 let sv=signed_volume(&mesh.positions,&mesh.indices);
 if sv<=0.0{return Err(format!("boolean output winding not outward CCW (signed volume {sv})"));}
 let mut min_area=f64::INFINITY;
 for t in &mesh.indices{let[a,b,c]=t.map(|i|mesh.positions[i as usize]);let ab=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];let ac=[c[0]-a[0],c[1]-a[1],c[2]-a[2]];let n=[ab[1]*ac[2]-ab[2]*ac[1],ab[2]*ac[0]-ab[0]*ac[2],ab[0]*ac[1]-ab[1]*ac[0]];let area=0.5*(n[0]*n[0]+n[1]*n[1]+n[2]*n[2]).sqrt();min_area=min_area.min(area);}
 let sliver=extent*extent*1e-14;
 if min_area<=sliver{return Err(format!("zero-area sliver in boolean output (min area {min_area:e} <= {sliver:e} at extent {extent})"));}
 let euler=mesh.positions.len() as i64-edge_stats(&mesh.indices).0 as i64+mesh.indices.len() as i64;
 Ok(BooleanResult{mesh,empty:false,volume:vol,vertices:onv,triangles:ont,interior_edges_two_manifold:two,boundary_edges:boundary,euler_characteristic:euler,min_triangle_area:min_area})
}

