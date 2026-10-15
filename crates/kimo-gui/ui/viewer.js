// Viewport: three.js preview of the engine scene. Dirty-flag rendering:
// the GPU is touched only when the scene changed (or the intro spin is
// running), capped at 30fps; when clean, the rAF loop stops entirely.
import * as THREE from './vendor/three.module.min.js';

const canvas = document.getElementById('gl');
const renderer = new THREE.WebGLRenderer({ canvas, antialias:true });
renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
const scene = new THREE.Scene();
scene.background = new THREE.Color(0x15151b);
const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 100);
camera.position.set(3.2, 2.4, 4.2);
camera.lookAt(0, 0.2, 0);

const cube = new THREE.Mesh(
  new THREE.BoxGeometry(1.4, 1.4, 1.4),
  new THREE.MeshStandardMaterial({ color:0xdd5227, roughness:.55, metalness:.05 }));
cube.position.y = 0.2;
scene.add(cube);
scene.add(new THREE.AmbientLight(0xf4f0e8, 0.55));
const key = new THREE.DirectionalLight(0xfff2df, 1.6); key.position.set(3, 5, 2); scene.add(key);

// dot-grid floor, matching the graph backdrop
const pts = [];
for (let x=-6; x<=6; x++) for (let z=-6; z<=6; z++) pts.push(x*0.55, -0.55, z*0.55);
const dots = new THREE.Points(
  new THREE.BufferGeometry().setAttribute('position', new THREE.Float32BufferAttribute(pts, 3)),
  new THREE.PointsMaterial({ color:0x2a2a34, size:0.03 }));
scene.add(dots);
const lamp = new THREE.Mesh(new THREE.SphereGeometry(0.07, 16, 16),
  new THREE.MeshBasicMaterial({ color:0xf4e0a8 }));
lamp.position.set(1.8, 1.6, 1.2); scene.add(lamp);

let dirty = true, raf = 0, last = 0, spinUntil = performance.now() + 4000;
function resize() {
  const w = canvas.clientWidth, h = canvas.clientHeight;
  if (!w || !h) return;
  if (canvas.width !== w*renderer.getPixelRatio() || canvas.height !== h*renderer.getPixelRatio()) {
    renderer.setSize(w, h, false); camera.aspect = w/h; camera.updateProjectionMatrix(); markDirty();
  }
}
function frame(t) {
  raf = 0;
  resize();
  const spinning = t < spinUntil;
  if ((dirty || spinning) && t - last >= 33) {   // 30fps cap
    if (spinning) { const a = (t/1000)*0.5; camera.position.set(Math.sin(a)*4.6, 2.4, Math.cos(a)*4.6); camera.lookAt(0,0.2,0); }
    renderer.render(scene, camera); dirty = false; last = t;
  }
  if (dirty || spinning) raf = requestAnimationFrame(frame);  // zero idle GPU when clean
}
export function markDirty() { dirty = true; if (!raf) raf = requestAnimationFrame(frame); }
export function setCubeColor(hex) { cube.material.color.set(hex); markDirty(); }
new ResizeObserver(() => markDirty()).observe(canvas);
markDirty();
