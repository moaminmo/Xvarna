# XV Devices

**Purpose.** Enumerates portable compute adapters visible to VAYU.

**Inputs.** Refresh. **Outputs.** adapter records, names, type, backend, memory limits and a report. Memory is reported in bytes.

**Method and assumptions.** Uses wgpu adapter enumeration; listing a device does not prove parity or adequate VRAM for a workload. Driver and OS determine availability.

**Example.** Use after XV Info and before XV Backend; preserve the selected adapter name in evidence.
