using System.Drawing;
using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;
using Grasshopper;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Data;
using Grasshopper.Kernel.Types;
using Xvarna.Native;

// Component overrides and ports are self-describing in Grasshopper metadata.
#pragma warning disable CS1591

namespace Xvarna.Grasshopper.Components.Study;

/// <summary>Creates a hash-protected RASHNU scientific Evidence Passport.</summary>
public sealed class EvidencePassportComponent : GH_Component
{
    public EvidencePassportComponent() : base("XVARNA Evidence Passport", "XV Evidence",
        "Seals result/scene identity, method, assumptions, limitations, citations, device and validation into portable scientific provenance.",
        "XVARNA", "05 Study")
    { }
    public override Guid ComponentGuid => new("a34cb04c-94b7-4672-81cb-92f710ad1554");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Result Hash", "RH", "Stable hash emitted by the analysis.", GH_ParamAccess.item);
        input.AddTextParameter("Scene Hash", "SH", "Stable scene hash.", GH_ParamAccess.item);
        input.AddTextParameter("Method", "M", "Human-readable scientific method.", GH_ParamAccess.item);
        input.AddIntegerParameter("Fidelity", "F", "0 Fast interactive, 1 Reference solver.", GH_ParamAccess.item, 0);
        input.AddTextParameter("Backend", "B", "CPU, VAYU GPU, Radiance, or other explicit backend.", GH_ParamAccess.item, "VAYU CPU");
        input.AddTextParameter("Device", "Dev", "Compute device or external tool.", GH_ParamAccess.item, string.Empty);
        input.AddTextParameter("Driver", "Drv", "Driver or external-tool version.", GH_ParamAccess.item, string.Empty);
        input.AddTextParameter("Study ID", "SID", "Optional Study identity.", GH_ParamAccess.item, string.Empty);
        input.AddTextParameter("Variant ID", "VID", "Optional unsigned variant identity.", GH_ParamAccess.item, string.Empty);
        input.AddTextParameter("Seed", "Seed", "Optional unsigned deterministic seed.", GH_ParamAccess.item, string.Empty);
        input.AddTextParameter("Sample Count", "N", "Effective sample/ray/time-step count.", GH_ParamAccess.item, "0");
        input.AddTextParameter("Elapsed µs", "µs", "Wall time in microseconds.", GH_ParamAccess.item, "0");
        input.AddTextParameter("Assumptions", "A", "Explicit modelling assumptions.", GH_ParamAccess.list);
        input.AddTextParameter("Limitations", "L", "Explicit limitations and non-claims.", GH_ParamAccess.list);
        input.AddTextParameter("Citations", "C", "Method citations, URLs, or stable identifiers.", GH_ParamAccess.list);
        input.AddNumberParameter("Parity Error", "PE", "Optional maximum CPU/GPU parity delta.", GH_ParamAccess.item);
        input.AddTextParameter("Reference Method", "RM", "Optional external reference tool/method.", GH_ParamAccess.item, string.Empty);
        input.AddNumberParameter("Reference Error", "RE", "Optional maximum reference error.", GH_ParamAccess.item);
        input.AddBooleanParameter("Run", "Run", "Seal the Evidence Passport.", GH_ParamAccess.item, true);
        foreach (int index in new[] { 5, 6, 7, 8, 9, 12, 13, 14, 15, 16, 17 }) input[index].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Passport", "P", "Strongly typed Evidence Passport.", GH_ParamAccess.item);
        output.AddTextParameter("JSON", "JSON", "Portable, hash-protected passport JSON.", GH_ParamAccess.item);
        output.AddTextParameter("Content Hash", "Hash", "BLAKE3 identity of the complete passport.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Human-readable evidence boundary.", GH_ParamAccess.item);
    }

    protected override void SolveInstance(IGH_DataAccess data)
    {
        string resultHash = string.Empty, sceneHash = string.Empty, method = string.Empty;
        string backend = "VAYU CPU", device = string.Empty, driver = string.Empty, study = string.Empty;
        string variant = string.Empty, seed = string.Empty, sampleCount = "0", elapsed = "0", referenceMethod = string.Empty;
        int fidelity = 0; bool run = true; double parityError = 0, referenceError = 0;
        List<string> assumptions = [], limitations = [], citations = [];
        if (!data.GetData(0, ref resultHash) || !data.GetData(1, ref sceneHash)
            || !data.GetData(2, ref method)) return;
        data.GetData(3, ref fidelity); data.GetData(4, ref backend); data.GetData(5, ref device);
        data.GetData(6, ref driver); data.GetData(7, ref study); data.GetData(8, ref variant);
        data.GetData(9, ref seed); data.GetData(10, ref sampleCount); data.GetData(11, ref elapsed);
        data.GetDataList(12, assumptions); data.GetDataList(13, limitations); data.GetDataList(14, citations);
        bool parity = data.GetData(15, ref parityError); data.GetData(16, ref referenceMethod);
        bool reference = data.GetData(17, ref referenceError); data.GetData(18, ref run);
        if (!run) { Message = "EVIDENCE\nPaused"; return; }
        try
        {
            if (fidelity is < 0 or > 1) throw new ArgumentException("Fidelity must be 0 Fast or 1 Reference.");
            EvidencePassport passport = EvidenceEngine.Seal(new()
            {
                ResultHash = resultHash,
                SceneHash = sceneHash,
                Method = method,
                Fidelity = (EvidenceFidelity)fidelity,
                Backend = backend,
                Device = device,
                Driver = driver,
                StudyId = study,
                VariantId = EvidenceComponentData.OptionalId(variant, "Variant ID"),
                Seed = EvidenceComponentData.OptionalId(seed, "Seed"),
                SampleCount = EvidenceComponentData.Id(sampleCount, "Sample Count"),
                ElapsedMicroseconds = EvidenceComponentData.Id(elapsed, "Elapsed µs"),
                Assumptions = assumptions,
                Limitations = limitations,
                Citations = citations,
                Validation = new()
                {
                    ParityTested = parity,
                    MaximumParityDelta = parity ? parityError : null,
                    ReferenceTested = reference,
                    ReferenceMethod = reference ? referenceMethod : string.Empty,
                    MaximumReferenceError = reference ? referenceError : null,
                },
            });
            data.SetData(0, passport); data.SetData(1, EvidenceComponentData.Json(passport));
            data.SetData(2, passport.ContentHash);
            data.SetData(3, $"RASHNU Evidence Passport 1{Environment.NewLine}{method} · {passport.Fidelity} · {backend}{Environment.NewLine}" +
                $"{passport.SampleCount:N0} samples · {passport.ElapsedMicroseconds:N0} µs{Environment.NewLine}" +
                $"Assumptions {assumptions.Count}; limitations {limitations.Count}; citations {citations.Count}{Environment.NewLine}{passport.ContentHash}");
            Message = $"EVIDENCE\n{passport.Fidelity} · SEALED";
        }
        catch (Exception exception) when (exception is ArgumentException or FormatException or OverflowException
            or InvalidOperationException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

/// <summary>Generates and analyzes estimator-valid Saltelli/Jansen or Morris designs.</summary>
public sealed class GlobalSensitivityComponent : GH_Component
{
    public GlobalSensitivityComponent() : base("XVARNA Global Sensitivity", "XV Sensitivity+",
        "Generates externally evaluable Saltelli/Jansen or Morris rows and analyzes only outputs aligned to the hash-protected design.",
        "XVARNA", "05 Study")
    { }
    public override Guid ComponentGuid => new("b2500814-feca-4168-bae6-825ce98fcafd");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddIntegerParameter("Kinds", "K", "0 continuous, 1 integer, 2 categorical.", GH_ParamAccess.list, 0);
        input.AddNumberParameter("Lower Bounds", "L", "Inclusive lower bounds.", GH_ParamAccess.list);
        input.AddNumberParameter("Upper Bounds", "U", "Inclusive upper bounds.", GH_ParamAccess.list);
        input.AddIntegerParameter("Method", "M", "0 Saltelli/Jansen, 1 Morris.", GH_ParamAccess.item, 0);
        input.AddIntegerParameter("Base Samples", "N", "Saltelli base rows or Morris trajectory count.", GH_ParamAccess.item, 1024);
        input.AddIntegerParameter("Morris Levels", "P", "Even grid level count ≥ 4.", GH_ParamAccess.item, 6);
        input.AddTextParameter("Seed", "Seed", "Unsigned deterministic seed.", GH_ParamAccess.item, "42");
        input.AddNumberParameter("Outputs", "Y", "Optional evaluated outputs, one branch per generated row.", GH_ParamAccess.tree);
        input.AddBooleanParameter("Run", "Run", "Generate and, when Y is connected, analyze.", GH_ParamAccess.item, true);
        input[7].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddGenericParameter("Design", "D", "Reusable hash-protected design.", GH_ParamAccess.item);
        output.AddNumberParameter("Parameters", "X", "External-evaluation rows, one branch per row.", GH_ParamAccess.tree);
        output.AddTextParameter("Design JSON", "JSON", "Portable design JSON.", GH_ParamAccess.item);
        output.AddNumberParameter("First Order", "S1", "Saltelli first-order values, one branch per output.", GH_ParamAccess.tree);
        output.AddNumberParameter("Total Order", "ST", "Jansen total-order values, one branch per output.", GH_ParamAccess.tree);
        output.AddNumberParameter("Morris Mu Star", "µ*", "Morris absolute elementary effects, one branch per output.", GH_ParamAccess.tree);
        output.AddGenericParameter("Analysis", "A", "Typed result when output rows are supplied.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Design dimensions, estimator, and content hashes.", GH_ParamAccess.item);
    }

    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<int> kinds = []; List<double> lower = [], upper = []; int method = 0, samples = 1024, levels = 6;
        string seed = "42"; bool run = true; GH_Structure<GH_Number> outputs = [];
        if (!data.GetDataList(1, lower) || !data.GetDataList(2, upper)) return;
        data.GetDataList(0, kinds); data.GetData(3, ref method); data.GetData(4, ref samples);
        data.GetData(5, ref levels); data.GetData(6, ref seed); bool hasOutputs = data.GetDataTree(7, out outputs);
        data.GetData(8, ref run); if (!run) { Message = "SENSITIVITY\nPaused"; return; }
        try
        {
            if (lower.Count == 0 || lower.Count != upper.Count) throw new ArgumentException("Lower and upper bounds must have the same non-zero count.");
            if (samples <= 0) throw new ArgumentException("Base Samples must be positive.");
            DesignVariable[] variables = lower.Select((value, index) => new DesignVariable(
                checked((ulong)index + 1), (DesignVariableKind)EvidenceComponentData.At(kinds, index, 0), value, upper[index])).ToArray();
            GlobalSensitivityMethod selected = method switch
            {
                0 => GlobalSensitivityMethod.SaltelliJansen,
                1 => GlobalSensitivityMethod.Morris,
                _ => throw new ArgumentException("Method must be 0 or 1.")
            };
            GlobalSensitivityDesign design = EvidenceEngine.CreateSensitivityDesign(variables, selected, samples, levels,
                EvidenceComponentData.Id(seed, "Seed"));
            DataTree<double> parameters = new();
            foreach (SensitivityDesignRow row in design.Rows) parameters.AddRange(row.Values, new GH_Path(row.RowIndex));
            GlobalSensitivityResult? result = null; DataTree<double> first = new(), total = new(), morris = new();
            if (hasOutputs && outputs.DataCount > 0)
            {
                if (outputs.Branches.Count != design.Rows.Count) throw new ArgumentException($"Outputs require exactly {design.Rows.Count:N0} branches, one per row.");
                IReadOnlyList<IReadOnlyList<double>> rows = outputs.Branches
                    .Select(branch => (IReadOnlyList<double>)branch.Select(value => value.Value).ToArray()).ToArray();
                result = EvidenceEngine.AnalyzeSensitivity(design, rows);
                foreach (IGrouping<int, SobolIndex> group in result.Sobol.GroupBy(value => value.OutputIndex))
                {
                    first.AddRange(group.OrderBy(value => value.ParameterIndex).Select(value => value.FirstOrder), new GH_Path(group.Key));
                    total.AddRange(group.OrderBy(value => value.ParameterIndex).Select(value => value.TotalOrder), new GH_Path(group.Key));
                }
                foreach (IGrouping<int, MorrisEffect> group in result.Morris.GroupBy(value => value.OutputIndex))
                    morris.AddRange(group.OrderBy(value => value.ParameterIndex).Select(value => value.MeanAbsolute), new GH_Path(group.Key));
            }
            data.SetData(0, design); data.SetDataTree(1, parameters); data.SetData(2, EvidenceComponentData.Json(design));
            data.SetDataTree(3, first); data.SetDataTree(4, total); data.SetDataTree(5, morris); data.SetData(6, result);
            data.SetData(7, $"RASHNU Global Sensitivity 0.18{Environment.NewLine}{selected}: {variables.Length} parameters; {design.Rows.Count:N0} external rows; seed {design.Seed}{Environment.NewLine}" +
                $"Design {design.ContentHash}{(result is null ? "" : Environment.NewLine + "Analysis " + result.ContentHash)}");
            Message = $"{selected.ToString().ToUpperInvariant()}\n{design.Rows.Count:N0} rows{(result is null ? "" : " · ANALYZED")}";
        }
        catch (Exception exception) when (exception is ArgumentException or FormatException or OverflowException
            or InvalidOperationException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

/// <summary>Chooses the most informative expensive reference evaluations per unit cost.</summary>
public sealed class MultiFidelityComponent : GH_Component
{
    public MultiFidelityComponent() : base("XVARNA Multi-Fidelity", "XV Fidelity",
        "Calibrates aligned Fast/Reference results, propagates discrepancy and observation noise, and recommends costly reference runs by expected improvement per cost.",
        "XVARNA", "05 Study")
    { }
    public override Guid ComponentGuid => new("45e96e48-72ec-43bb-a7d4-293b3be0416c");
    public override GH_Exposure Exposure => GH_Exposure.primary;
    protected override Bitmap? Icon => Xvarna.Grasshopper.Branding.XvarnaIconFactory.For(GetType());

    protected override void RegisterInputParams(GH_InputParamManager input)
    {
        input.AddTextParameter("Candidate IDs", "ID", "One stable unsigned ID per branch.", GH_ParamAccess.list);
        input.AddNumberParameter("Parameters", "X", "One parameter branch per candidate.", GH_ParamAccess.tree);
        input.AddIntegerParameter("Directions", "Dir", "0 minimize, 1 maximize; one per objective.", GH_ParamAccess.list);
        input.AddNumberParameter("Fast Objectives", "Fast", "One complete objective branch per candidate.", GH_ParamAccess.tree);
        input.AddNumberParameter("Reference Objectives", "Ref", "Aligned branches; empty branch means not evaluated.", GH_ParamAccess.tree);
        input.AddTextParameter("Fast Evidence", "FE", "Evidence/result hash per candidate.", GH_ParamAccess.list);
        input.AddTextParameter("Reference Evidence", "RE", "Evidence hash per non-empty reference branch.", GH_ParamAccess.list);
        input.AddNumberParameter("Reference Point", "RP", "Worst direction-aware point for hypervolume.", GH_ParamAccess.list);
        input.AddNumberParameter("Fast Cost", "FC", "Cost per Fast run.", GH_ParamAccess.item, 1.0);
        input.AddNumberParameter("Reference Cost", "RC", "Cost per Reference run.", GH_ParamAccess.item, 50.0);
        input.AddNumberParameter("Fast Noise", "σ", "One scalar noise floor for all Fast objectives.", GH_ParamAccess.item, 0.0);
        input.AddIntegerParameter("Batch", "Q", "Maximum reference jobs to recommend.", GH_ParamAccess.item, 4);
        input.AddIntegerParameter("MC Samples", "MC", "Monte-Carlo EHVI samples for two objectives.", GH_ParamAccess.item, 2048);
        input.AddTextParameter("Seed", "Seed", "Unsigned deterministic seed.", GH_ParamAccess.item, "42");
        input.AddBooleanParameter("Run", "Run", "Calibrate and select reference jobs.", GH_ParamAccess.item, true);
        input[4].Optional = true; input[6].Optional = true;
    }

    protected override void RegisterOutputParams(GH_OutputParamManager output)
    {
        output.AddTextParameter("Recommended IDs", "ID", "Ordered candidate IDs for Reference evaluation.", GH_ParamAccess.list);
        output.AddNumberParameter("Predicted Reference", "µ", "Predicted Reference means per selected candidate.", GH_ParamAccess.tree);
        output.AddNumberParameter("Uncertainty", "σ", "Predicted Reference standard deviations.", GH_ParamAccess.tree);
        output.AddNumberParameter("Value Per Cost", "V/C", "Acquisition score per selected candidate.", GH_ParamAccess.list);
        output.AddGenericParameter("Calibration", "Cal", "Per-objective linear discrepancy diagnostics.", GH_ParamAccess.list);
        output.AddTextParameter("Selection JSON", "JSON", "Portable complete selection result.", GH_ParamAccess.item);
        output.AddTextParameter("Content Hash", "Hash", "BLAKE3 identity of calibration and selection.", GH_ParamAccess.item);
        output.AddTextParameter("Report", "Rep", "Paired sample count, fit quality, cost and method boundary.", GH_ParamAccess.item);
    }

    protected override void SolveInstance(IGH_DataAccess data)
    {
        List<string> ids = [], fastEvidence = [], referenceEvidence = []; List<int> directionCodes = [];
        List<double> referencePoint = []; GH_Structure<GH_Number> parameters = [], fast = [], reference = [];
        double fastCost = 1, referenceCost = 50, noise = 0; int batch = 4, monteCarlo = 2048;
        string seed = "42"; bool run = true;
        if (!data.GetDataList(0, ids) || !data.GetDataTree(1, out parameters)
            || !data.GetDataList(2, directionCodes) || !data.GetDataTree(3, out fast)
            || !data.GetDataList(5, fastEvidence) || !data.GetDataList(7, referencePoint)) return;
        data.GetDataTree(4, out reference); data.GetDataList(6, referenceEvidence);
        data.GetData(8, ref fastCost); data.GetData(9, ref referenceCost); data.GetData(10, ref noise);
        data.GetData(11, ref batch); data.GetData(12, ref monteCarlo); data.GetData(13, ref seed); data.GetData(14, ref run);
        if (!run) { Message = "FIDELITY\nPaused"; return; }
        try
        {
            int count = ids.Count, objectives = directionCodes.Count;
            if (count == 0 || parameters.Branches.Count != count || fast.Branches.Count != count
                || fastEvidence.Count != count || objectives < 2 || referencePoint.Count != objectives)
                throw new ArgumentException("IDs, parameter/Fast branches, Fast evidence, directions, and reference point are not aligned.");
            ObjectiveDirection[] directions = directionCodes.Select(value => value switch
                {
                    0 => ObjectiveDirection.Minimize,
                    1 => ObjectiveDirection.Maximize,
                    _ => throw new ArgumentException("Directions must contain only 0 or 1.")
                }).ToArray();
            ScientificCandidate[] candidates = ids.Select((text, index) => new ScientificCandidate
            {
                CandidateId = EvidenceComponentData.Id(text, $"Candidate ID {index}"),
                Values = parameters.Branches[index].Select(value => value.Value).ToArray(),
            }).ToArray();
            List<FidelityObservation> observations = [];
            int referenceHashIndex = 0;
            for (int index = 0; index < count; index++)
            {
                double[] fastRow = fast.Branches[index].Select(value => value.Value).ToArray();
                if (fastRow.Length != objectives) throw new ArgumentException($"Fast branch {index} requires {objectives} objectives.");
                observations.Add(new()
                {
                    CandidateId = candidates[index].CandidateId,
                    Fidelity = EvidenceFidelity.Fast,
                    Objectives = fastRow,
                    Cost = fastCost,
                    NoiseStandardDeviations = Enumerable.Repeat(noise, objectives).ToArray(),
                    EvidenceHash = fastEvidence[index]
                });
                if (reference.Branches.Count <= index || reference.Branches[index].Count == 0) continue;
                double[] referenceRow = reference.Branches[index].Select(value => value.Value).ToArray();
                if (referenceRow.Length != objectives) throw new ArgumentException($"Reference branch {index} requires {objectives} objectives.");
                if (referenceHashIndex >= referenceEvidence.Count) throw new ArgumentException("Reference Evidence requires one hash per non-empty reference branch.");
                observations.Add(new()
                {
                    CandidateId = candidates[index].CandidateId,
                    Fidelity = EvidenceFidelity.Reference,
                    Objectives = referenceRow,
                    Cost = referenceCost,
                    NoiseStandardDeviations = new double[objectives],
                    EvidenceHash = referenceEvidence[referenceHashIndex++]
                });
            }
            if (referenceHashIndex != referenceEvidence.Count) throw new ArgumentException("Unused Reference Evidence hashes were supplied.");
            MultiFidelitySelection selection = EvidenceEngine.RecommendReferenceEvaluations(directions, candidates, observations,
                new()
                {
                    BatchSize = batch,
                    MonteCarloSamples = monteCarlo,
                    Seed = EvidenceComponentData.Id(seed, "Seed"),
                    ReferencePoint = referencePoint,
                    MinimumStandardDeviation = Math.Max(1e-9, noise)
                });
            DataTree<double> means = new(), deviations = new();
            foreach ((FidelityRecommendation item, int index) in selection.Recommendations.Select((value, index) => (value, index)))
            { means.AddRange(item.PredictedReferenceMean, new GH_Path(index)); deviations.AddRange(item.PredictedReferenceStandardDeviation, new GH_Path(index)); }
            data.SetDataList(0, selection.Recommendations.Select(value => value.CandidateId.ToString(CultureInfo.InvariantCulture)));
            data.SetDataTree(1, means); data.SetDataTree(2, deviations);
            data.SetDataList(3, selection.Recommendations.Select(value => value.AcquisitionPerCost));
            data.SetDataList(4, selection.Calibrations); data.SetData(5, EvidenceComponentData.Json(selection));
            data.SetData(6, selection.ContentHash);
            data.SetData(7, $"VAHMAN Multi-Fidelity 0.18{Environment.NewLine}{selection.PairedCandidateIds.Count:N0} aligned Fast/Reference pairs; {selection.Recommendations.Count:N0} recommended jobs{Environment.NewLine}" +
                $"Cost-aware Monte-Carlo EHVI for two objectives; transparent uncertainty proxy above two (not qNEHVI/MF-HVKG){Environment.NewLine}" +
                string.Join(Environment.NewLine, selection.Calibrations.Select(value => $"F{value.ObjectiveIndex}: slope {value.Slope:G5}; R² {value.RSquared:G4}; residual σ {value.ResidualStandardDeviation:G4}")));
            Message = $"FIDELITY\n{selection.PairedCandidateIds.Count} paired → {selection.Recommendations.Count} jobs";
        }
        catch (Exception exception) when (exception is ArgumentException or FormatException or OverflowException
            or InvalidOperationException or XvarnaNativeException)
        { AddRuntimeMessage(GH_RuntimeMessageLevel.Error, exception.Message); }
    }
}

internal static class EvidenceComponentData
{
    private static readonly JsonSerializerOptions Options = CreateOptions();
    internal static string Json<T>(T value) => JsonSerializer.Serialize(value, Options);
    internal static int At(IReadOnlyList<int> values, int index, int fallback) => values.Count switch
    {
        0 => fallback,
        1 => values[0],
        _ when values.Count > index => values[index],
        _ => throw new ArgumentException("Kinds require zero, one, or one value per variable.")
    };
    internal static ulong Id(string value, string label) => ulong.TryParse(value, NumberStyles.None,
        CultureInfo.InvariantCulture, out ulong result) ? result : throw new FormatException($"{label} must be an unsigned integer.");
    internal static ulong? OptionalId(string value, string label) => string.IsNullOrWhiteSpace(value) ? null : Id(value, label);
    private static JsonSerializerOptions CreateOptions()
    {
        JsonSerializerOptions options = new(JsonSerializerDefaults.Web) { WriteIndented = true };
        options.Converters.Add(new JsonStringEnumConverter(JsonNamingPolicy.CamelCase));
        return options;
    }
}
