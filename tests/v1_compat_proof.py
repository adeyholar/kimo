"""Upgrade compat: tests/data/v1_project.kimo was saved by the v1 main binary (425a2e8). The current binary must load it and re-save it BYTE-IDENTICALLY
(no new keys written for features the project does not use). Regenerate the fixture only with a v1 binary: python3 tests/v1_compat_proof.py --make-fixture PATH_TO_V1_BINARY. /tmp only except the fixture."""
import sys, os, json; sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'examples')); from mcp_client import Kimo
FIX = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'data', 'v1_project.kimo')
if len(sys.argv) > 2 and sys.argv[1] == '--make-fixture':
    k = Kimo(exe=sys.argv[2]); c = k.call; c('project_set', width=320, height=180, fps=25); tr = c('add_track')['track_id']; t2 = c('add_track')['track_id']
    a = c('add_shape', track_id=tr, shape='rect', width=120, height=70, fill=[.9, .3, .2, 1], start=0, duration=2)['clip_id']
    b = c('add_shape', track_id=t2, shape='ellipse', width=90, height=90, fill=[.2, .8, .4, .8], start=0.5, duration=2)['clip_id']
    c('add_text', track_id=t2, text='KIMO', start=0, duration=1, size=60)
    c('add_effect', clip_id=a, kind='glow'); c('add_effect', clip_id=a, kind='blur', params={'radius': 3}); c('add_effect', clip_id=b, kind='vignette')
    c('set_keyframe', clip_id=a, property='scale', time=0.5, value=1.4); c('set_property', clip_id=b, property='opacity', value=0.7)
    c('set_mask', clip_id=a, shape='ellipse', cx=.5, cy=.5, w=.5, h=.6, feather=.2)
    c('add_particles', track_id=t2, kind='sparks', start=0, duration=1, count=50, seed=5)
    c('add_shard_burst', track_id=t2, start=0, duration=1, config={'count': 20, 'size_min': 10, 'size_max': 30, 'origin': [160, 90, 0], 'speed': 100, 'seed': 3, 'material': 'glass', 'refract': 30})
    c('group_clips', clip_ids=[a, b]); c('save_project', path=FIX); print('fixture written', os.path.getsize(FIX), 'bytes'); sys.exit(0)
k = Kimo(); c = k.call; r = c('load_project', path=FIX); print('loaded:', r)
c('save_project', path='/tmp/v1_resave.kimo')
a, b = open(FIX, 'rb').read(), open('/tmp/v1_resave.kimo', 'rb').read()
assert a == b, 'v1 project did not re-save identically (%d vs %d bytes)' % (len(a), len(b))
print('v1 project loads and re-saves byte-identically (%d bytes)' % len(a)); print('v1 compat proof OK')
