# .NET Study platform API — 0.16.0

`StudyPlatformWorkspace` owns no native handle; it is a validated path-based façade over the Rust transactional workspace. Every operation reopens and verifies hashes, so a process or Rhino document may stop after `BeginBatch()` and later continue from the same directory.

## Manifest and workspace

```csharp
StudyPlatformManifest manifest = new()
{
    StudyId = "courtyard-search",
    Title = "Daylight, view and energy",
    Parameters =
    [
        new() { ParameterId = 1, Name = "depth", Kind = DesignVariableKind.Continuous,
            LowerBound = 0.2, UpperBound = 1.0, Unit = "ratio" },
        new() { ParameterId = 2, Name = "floors", Kind = DesignVariableKind.Integer,
            LowerBound = 2, UpperBound = 8, Unit = "floors" },
    ],
    Objectives =
    [
        new() { ObjectiveId = 1, Name = "visual", Direction = ObjectiveDirection.Maximize,
            Unit = "%", Components = ["sDA", "greenView"] },
        new() { ObjectiveId = 2, Name = "energy", Direction = ObjectiveDirection.Minimize,
            Unit = "kWh/m2" },
    ],
    Constraints = [new() { ConstraintId = 1, Name = "depthLimit", Unit = "ratio" }],
    BaselineVariantId = 1,
    Optimizer = MultiObjectiveOptimizerOptions.Default with
    {
        PopulationSize = 64, OffspringSize = 64, Seed = 1701,
    },
};

string schema = StudyPlatformWorkspace.ManifestSchemaJson;
StudyPlatformWorkspace workspace = StudyPlatformWorkspace.Create("study", manifest);
StudyPlatformBatch batch = workspace.BeginBatch();
```

An interrupted caller uses `StudyPlatformWorkspace.Open("study")`; `State.ActiveBatchId` and `State.PendingCandidates` identify unfinished work. Calling `BeginBatch()` returns that exact envelope rather than advancing the random stream.

## Commit and report

Evaluate every candidate outside VAHMAN, preserve its ID, and group values once per manifest objective:

```csharp
StudyPlatformEvaluation[] rows = batch.Candidates.Select(candidate =>
    new StudyPlatformEvaluation
    {
        VariantId = candidate.CandidateId,
        ObjectiveValues =
        [
            new[] { ComputeSda(candidate.Values), ComputeGreenView(candidate.Values) },
            new[] { ComputeEnergy(candidate.Values) },
        ],
        ConstraintResiduals = [candidate.Values[0] - 0.9],
        MetricFields =
        [
            new() { FieldId = "daylight.totalLux", Unit = "lux",
                Positions = sensorPositions, Values = sensorLux },
        ],
        ProvenanceHash = upstreamResultHash,
    }).ToArray();

StudyWorkspaceState committed = workspace.CommitBatch(batch.BatchId, rows);
StudyReportArtifacts artifacts = workspace.GenerateReport("study/report",
    new StudyReportOptions(1000, 1000, 0.95, 0.05, 1701));
```

`artifacts.Report` and `artifacts.Viewer` are network-free HTML. `artifacts.Parquet`, `artifacts.Gltf`, and `artifacts.Data` are the standards-based portable table, objective-space asset, and complete JSON record. Keep the entire report directory together when sharing it.

## Native C ABI

The C surface uses caller-owned UTF-8 and a sizing pass. Call each function first with `output_utf8 = NULL`, `output_capacity = 0`, allocate `required_bytes`, then call again:

- `xv_study_manifest_schema_json`
- `xv_study_workspace_create`
- `xv_study_workspace_status`
- `xv_study_workspace_begin_batch`
- `xv_study_workspace_commit_batch`
- `xv_study_workspace_report`

Workspace paths, manifest JSON, and commit JSON are borrowed for the duration of a call. Report policy is the fixed 48-byte `xv_study_report_options`. Every returned JSON string includes its terminating NUL in the required capacity.

## Safety rules

- Treat manifest registry order and IDs as immutable after creation.
- Commit exactly one row for every candidate in the active batch.
- Preserve objective grouping; vector components flatten only inside the platform.
- Use non-positive residuals for feasible constraints.
- Give aligned metric fields the same `fieldId`, unit, value count, and positions as the baseline.
- Use one writer per workspace. Readers may reopen only after the writer operation completes.
