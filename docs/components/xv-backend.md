# XV Devices, XV Backend, XV Runtime, and backend-aware XV Ray Query

`XV Devices` lists every adapter visible to VAYU with its driver, API code, device class, vendor ID, and device ID. Duplicate physical devices can appear through different graphics APIs; this is expected and remains visible rather than silently merged.

`XV Backend` accepts an immutable `XV Scene` and creates a revision-aware cached compute session:

- `Auto`: prefer portable GPU, retain explicit CPU fallback;
- `GPU`: require portable GPU and surface failure;
- `CPU`: use canonical f64 traversal without opening a device.

`Adapter Filter` is an optional case-insensitive substring copied from `XV Devices`; it makes selection reproducible when a machine exposes several GPUs or the same physical device through several graphics APIs. An unmatched filter is a reported fallback in Auto and an error in GPU mode; CPU rejects a meaningless filter. `Maximum Error mm` controls the permitted f64-to-f32 conversion error. Triangle, rays-per-dispatch, geometry-chunk, and GPU-memory inputs are enforced resource caps, not performance hints. `Scene Cache` controls validated portable-plan reuse. Low Power changes adapter preference only when the filter is empty. Outputs include the reusable session, active backend, adapter, fallback flag, portable payload, initialization time, chunk count, cache hit, streamed state, and a complete provenance report.

Connect the session to `XV Ray Query`, `XV Target View`, `XV Corridor`, or `XV View Path`. These components accept either `XV Scene` or `XV Backend`; existing scientific outputs and stable component GUIDs remain unchanged. Ray Query reports each raw batch, while advanced DAENA result/report objects aggregate all domain batches, dispatches, transfer stages, maximum precision error, and fallback provenance.

`XV Runtime` reads the retained session without synchronizing or resetting it. It reports cumulative requests, rays, dispatches, reusable batches, arena generation/capacity/memory, geometry upload count/bytes, host snapshot memory, cache hit, streaming, transfer bytes, runtime fallback, device health, loss/error counts, and the latest exact diagnostic. A fixed-size repeated definition should show one arena generation; growth is expected only when a later request needs a larger slot.

VAYU owns two lazy scratch slots. Each retains ray, hit, readback, parameter, bind-group, and host staging resources after warm-up. Capacity grows by powers of two and is bounded at 262,144 rays per slot as well as by the declared and hardware dispatch ceilings. This is dynamic scratch memory in addition to the static scene payload shown by `XV Backend`.

The portable plan retains BLAS resources once and stores affine instances plus a TLAS rather than expanding every occurrence. If a plan exceeds a declared working-set budget, deterministic chunks are uploaded and traversed in stable order; closest-hit tie breaking remains identical to CPU. Auto mode contains device creation/loss/error and reports canonical CPU fallback, while GPU mode fails explicitly.

For small batches, use CPU unless measuring proves otherwise. For strict reproducibility, record XVARNA version, adapter/driver, preference, error budget, geometry/VRAM budgets, cache state, chunk count, and the trace report.
