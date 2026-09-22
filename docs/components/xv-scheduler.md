# XV Scheduler

`XV Scheduler` configures and observes the one bounded worker pool shared by XVARNA's compute-heavy Grasshopper analyses. The default concurrency leaves one logical CPU available to Rhino and the default queue capacity is 256 jobs.

Each job has a stable ID, component name, scope generation, queued/running state, phase, completed/total units, fraction, and cancellation state. A newer solve for the same component scope makes the prior generation stale; stale output is suppressed instead of overwriting the current result. Queue cancellation is immediate. A native or GPU dispatch already submitted completes to a safe boundary, after which cancellation or stale-generation checks discard its unpublished result.

Limits may be changed only while the scheduler is idle. Outputs expose live jobs and cumulative completed, cancelled, stale, failed, and average queue-latency telemetry. The scheduler coordinates execution and publication; it does not claim that every third-party/native kernel can be pre-empted in the middle of an individual call.
