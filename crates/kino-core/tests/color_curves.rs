use kino_core::color::curves::Curve;
#[test]fn nonuniform_identity_extended(){let c=Curve::new(&[[0.,0.],[0.01,0.01],[0.6,0.6],[1.,1.]]).unwrap();for i in -100..1000{let x=i as f32/200.;assert!((c.at(x)-x).abs()<1e-6)}}
#[test]fn no_overshoot_monotone(){let c=Curve::new(&[[0.,0.],[0.1,0.5],[0.9,0.6],[1.,1.]]).unwrap();let mut prev=0.;for i in 0..10001{let v=c.at(i as f32/10000.);assert!(v>=prev-1e-7&&v<=1.00001);prev=v}}
#[test]fn hostile_points(){for p in [vec![],vec![[0.,0.]],vec![[0.,0.],[0.,1.]],vec![[1.,0.],[0.,1.]],vec![[0.,f32::NAN],[1.,1.]],vec![[0.,0.];33]]{assert!(Curve::new(&p).is_err())}}
