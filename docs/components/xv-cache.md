# XV Cache

`XV Cache` configures and observes the process-wide, two-tier VAYU portable-scene cache. The default directory is `%LOCALAPPDATA%\XVARNA\cache\v1`, with a 256 MiB process-memory LRU and a 4 GiB persistent-disk budget.

Every disk entry contains a format magic, schema version, cache key, payload length, and BLAKE3 checksum. Writes use a temporary file in the destination directory followed by atomic rename. A truncated, mismatched, or corrupt entry is never returned: it is counted, removed, and rebuilt from the immutable scene. Both tiers evict entries to remain inside their declared byte budgets.

`Apply` changes the canonical directory and budgets when values change. `Clear` is rising-edge triggered and removes only `.xvc` cache entries inside that exact directory; it does not recursively remove the directory or unrelated files. Outputs expose memory/disk hits, misses, writes, evictions, corruption recoveries, occupancy, canonical directory, schema, policy, and last event. Connect `XV Backend` and inspect its `Cache Hit` output to attribute reuse to a particular scene session.
