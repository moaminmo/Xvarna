# XV Workspace

Creates, opens, or begins/resumes a persistent VAHMAN workspace. Action `0` validates and reports state, `1` creates from Manifest JSON, and `2` returns the current pending batch or checkpoints the next batch. Candidate IDs and parameter values are emitted as aligned lists/Data Tree branches.

Ledger and checkpoint hashes are first-class outputs. If Rhino closes after a batch is created, reopen the same directory and use Action `2`; the identical candidates are returned. Keep a single writer for each directory.
