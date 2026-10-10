use kino_core::color::lut::Lut;
fn cube()->String{let mut s="LUT_3D_SIZE 2\nDOMAIN_MIN -1 -1 -1\nDOMAIN_MAX 2 2 2\n".to_string();for b in [-1,2]{for g in [-1,2]{for r in [-1,2]{s+=&format!("{r} {g} {b}\n")}}}s}
#[test]fn domain_identity_all_tetrahedra(){let l=Lut::parse(&cube()).unwrap();for i in 0..1000{let c=[(i%17) as f32/16.,(i%31) as f32/30.,(i%43) as f32/42.];let o=l.sample(c);for j in 0..3{assert!((o[j]-c[j]).abs()<3e-7)}}}
#[test]fn exact_vertices(){let l=Lut::parse(&cube()).unwrap();assert_eq!(l.sample([-1.;3]),[-1.;3]);assert_eq!(l.sample([2.;3]),[2.;3]);}
#[test]fn hostile_cube(){for x in ["LUT_3D_SIZE 0","LUT_3D_SIZE 1\n0 0 0", "LUT_3D_SIZE 999999999999", "LUT_3D_SIZE 2\nNaN 0 0", "LUT_3D_SIZE 2\ninf 0 0", "LUT_3D_SIZE 2\nDOMAIN_MIN 1 1 1\nDOMAIN_MAX 0 0 0", "LUT_3D_SIZE 2\nLUT_3D_SIZE 2"]{assert!(Lut::parse(x).is_err())}}
