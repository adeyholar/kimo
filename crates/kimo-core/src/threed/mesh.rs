use serde::{Serialize,Deserialize};
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Mesh {pub positions:Vec<[f64;3]>,pub indices:Vec<[u32;3]>}
impl Mesh {
 pub fn validate(&self)->Result<(),String>{
  if self.positions.is_empty()||self.positions.len()>1_000_000||self.indices.is_empty()||self.indices.len()>1_000_000{return Err("mesh requires1..1000000 positions and triangles".into());}
  if self.positions.iter().flatten().any(|v|!v.is_finite()||v.abs()>1e12){return Err("mesh positions finite and abs<=1e12 required".into());}
  if self.indices.iter().flatten().any(|i|*i as usize>=self.positions.len()){return Err("mesh index outside position buffer".into());}Ok(())
 }
 pub fn bounds(&self)->Result<[[f64;3];2],String>{self.validate()?;let mut lo=[f64::INFINITY;3];let mut hi=[f64::NEG_INFINITY;3];for p in &self.positions{for j in 0..3{lo[j]=lo[j].min(p[j]);hi[j]=hi[j].max(p[j]);}}Ok([lo,hi])}
}
pub type Matrix=[[f64;4];4];
pub const IDENTITY:Matrix=[[1.,0.,0.,0.],[0.,1.,0.,0.],[0.,0.,1.,0.],[0.,0.,0.,1.]];
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(deny_unknown_fields)]
pub struct Node {pub parent:Option<usize>,pub local:Matrix,pub mesh:Option<usize>}
fn multiply(a:Matrix,b:Matrix)->Result<Matrix,String>{let mut o=[[0.;4];4];for i in 0..4{for j in 0..4{for k in 0..4{o[i][j]+=a[i][k]*b[k][j];}if !o[i][j].is_finite(){return Err("world matrix overflow".into());}}}Ok(o)}
pub fn world_matrices(nodes:&[Node],mesh_count:usize)->Result<Vec<Matrix>,String>{
 if nodes.len()>256{return Err("max256 scene nodes".into());}
 for n in nodes{if n.local.iter().flatten().any(|v|!v.is_finite()||v.abs()>1e12)||n.local[3]!=[0.,0.,0.,1.]{return Err("finite bounded affine4x4 required".into());}if n.mesh.map(|i|i>=mesh_count).unwrap_or(false){return Err("node mesh missing".into());}}
 fn visit(i:usize,n:&[Node],state:&mut[u8],cache:&mut[Matrix])->Result<Matrix,String>{if state[i]==1{return Err("scene parent cycle".into());}if state[i]==2{return Ok(cache[i]);}state[i]=1;let m=if let Some(p)=n[i].parent{if p>=n.len(){return Err("scene parent missing".into());}multiply(visit(p,n,state,cache)?,n[i].local)?}else{n[i].local};cache[i]=m;state[i]=2;Ok(m)}
 let mut state=vec![0;nodes.len()];let mut out=vec![IDENTITY;nodes.len()];for i in 0..nodes.len(){visit(i,nodes,&mut state,&mut out)?;}Ok(out)
}
pub fn transform_mesh(mesh:&Mesh,m:Matrix)->Result<Mesh,String>{mesh.validate()?;if m[3]!=[0.,0.,0.,1.]||m.iter().flatten().any(|v|!v.is_finite()){return Err("finite affine transform required".into());}let mut out=mesh.clone();for p in &mut out.positions{let q=*p;for i in 0..3{p[i]=m[i][0]*q[0]+m[i][1]*q[1]+m[i][2]*q[2]+m[i][3];}}out.validate()?;Ok(out)}
