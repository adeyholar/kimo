# Native OCIO CPU API (review-only implementation)

This is an opt-in Rust/C++ CPU adapter to installed OpenColorIO 2. It does not
replace Kino's defaults, output transform, Python ACES reference, renderer, or
project model. Configs are not called ACES-certified or production-validated.

## Build

Supply compatible OCIO 2 headers and a shared library, then build with:

```
OCIO_INCLUDE_DIR=/opt/ocio/include OCIO_LIB_DIR=/opt/ocio/lib \
LD_LIBRARY_PATH=/opt/ocio/lib cargo test -p kino-core --features native-ocio \
  --test color_ocio_config
```

`OCIO_INCLUDE_DIR` contains `OpenColorIO/`. The headers and shared library must
match. Tested with OCIO 2.5.1 headers and the 2.5.1 wheel shared library on
Linux x86_64. No OCIO dependency is downloaded by the build. Default builds need no
OCIO headers/library, keep legacy effects unchanged, and fail closed on explicit
OCIO effects. The C++ adapter uses exception containment and RAII. `cc` is the
only new build dependency. Existing extension discovery stays intact.

## Configs, transforms and tools

`color::ocio_config::Config::from_text/from_file` loads a UTF-8 config of at most
4 MiB using the real OCIO parser. `colorspaces()` includes inactive spaces.
`views()` lists display-defined views. Shared views and virtual displays are not
listed in this version. The library version is inspectable. No global current
config is set. Search paths/environment variable definitions are cleared in the
local config. Environment-independent display listing uses the all-displays API.

Self-contained configs may contain matrices, log transforms, embedded LUTs,
groups and built-ins supported by the installed OCIO version. External
`FileTransform`s are rejected throughout colorspaces, looks, named transforms,
and view transforms (including nested groups). This is deliberately narrower
than *every arbitrary config*: configs needing external LUT/CLF/CDL files or
context variables need a later asset allowlist/resolver. Native parser resource
use is not a process sandbox. The input cap and catalog/depth limits do not
prove safety against every malicious YAML/OCIO parser input.

`Config::processor(source, destination)` creates a persistent CPU processor.
It retains its own lifetime independently of the config. `Processor::apply`
works on straight/unassociated RGBA float frames, permits signed and HDR RGB,
checks shape/finiteness/alpha, preserves alpha, and is transactional on errors.
A processor marked no-op performs no pixel writes (including signed zero).
Optimized CPU approximations are disabled. Repeated results are deterministic
on the tested build, not promised bit-identical across OCIO versions, compiler
flags or CPU architectures. The RGB-only operation intentionally ignores alpha
terms in OCIO transforms.

Auto-registered tools: `inspect_ocio_config`, `transform_ocio_samples`. No files
are written by these tools. They do not select a project output display view.

## Per-effect working-domain declarations

The enum `WorkingDomain` has `Current`, `SceneLinear`, `DisplayReferred`.
`DomainProcessor::new(config, current, linear, display, domain)` resolves the
required space and compiles both transforms **before** effect evaluation.
`apply` automatically sandwiches the effect and returns to the current space.
Unknown effect kinds fail; mixing the legacy encoding wrapper fails.

The existing `effects::apply` dispatch recognizes an explicit effect string
contract, without model/schema changes:

```
"ocio_domain": "display-referred"   # or scene-linear/current
"ocio_config": "/path/to/config.ocio"
"ocio_working_space": "ACEScg"
"ocio_linear_space": "ACEScg"
"ocio_display_space": "My encoded display space"
```

These strings are declarations supplied by the caller, not inferred metadata.
The caller must ensure the actual incoming frame is in `ocio_working_space`.
There is no renderer-wide propagation or stale-project-domain detection.
Without declarations, all existing effects retain their current behavior.
No automatic classification of existing effect kinds is made. A display-space
sandwich is an invertible colorspace transform, **not** an ACES display-view
rendering operator or a guaranteed invertible tone map. Configured display
spaces that clip/gamut-map may lose information on the round trip.

For realtime/export integration, cache a `DomainProcessor` in the caller.
The string-dispatch convenience path reloads/recompiles per call and is not
performance-qualified. No production display-view processor or MCP effect
setter is included; existing generic effect string configuration may be used.

## Proven tests

The small test config is authored here, not an Academy shipping config or a
complete ACES output config. Its reference domain is AP1 scene-linear. The
ACEScct space uses the installed OCIO built-in encoding curve. DisplayGamma2 is
synthetic and explicitly not an ACES ODT.

Published ACEScct Equation 1 from
https://docs.acescentral.com/encodings/acescct/ supplies independent expectations
for signed/toe/breakpoint/gray/white/HDR samples. Maximum allowed encoded error
is 2e-6; round-trip tolerance is 1e-5. Tests cover listing, repeated transforms,
processor lifetime, bit-exact no-op and alpha, malformed/oversize/NUL/nested
external-file rejection, bad inputs, effect sandwiches, error rollback, string
dispatch, missing native feature and tool registration. They do not prove an
ACES output view, the full ACES 2 suite, hostile-parser sandboxing, GPU output,
render-wide integration, throughput or industry parity.

An optional integration test loads the official built-in ACES2 CG config when
`KINO_TEST_NATIVE_ACES_CONFIG` points to its serialized .ocio file. It checks
space/view discovery and ACEScg-to-ACEScct gray, not its output display views.
