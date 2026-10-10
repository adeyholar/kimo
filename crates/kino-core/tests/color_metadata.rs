use kino_core::color::metadata::Mastering;
fn m()->Mastering{Mastering{red:[0.68,0.32],green:[0.265,0.69],blue:[0.15,0.06],white:[0.3127,0.329],min_nits:0.005,max_nits:1000.,max_cll:1000,max_fall:400}}
#[test]fn units(){assert_eq!(m().x265().unwrap(),"hdr10=1:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,50):max-cll=1000,400")}
#[test]fn hostile_metadata(){let mut x=m();x.max_fall=1001;assert!(x.validate().is_err());x=m();x.min_nits=f64::NAN;assert!(x.validate().is_err());x=m();x.red=[1.,1.];assert!(x.validate().is_err());x=m();x.max_nits=20000.;assert!(x.validate().is_err());x=m();x.blue=x.red;assert!(x.validate().is_err());}
