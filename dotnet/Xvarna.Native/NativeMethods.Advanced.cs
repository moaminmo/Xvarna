using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    [DllImport(LibraryName, EntryPoint = "xv_scene_target_view", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneTargetView(
        SafeSceneHandle sceneHandle, NativeViewObserver* observers, nuint observerCount,
        NativeViewTargetPatch* patches, nuint patchCount, NativeTargetViewOptions* options,
        NativeTargetViewMetadata* outputMetadata, NativeTargetViewSummary* outputSummaries,
        nuint summaryCapacity, NativeTargetViewEntry* outputEntries, nuint entryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_view_corridor", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneViewCorridor(
        SafeSceneHandle sceneHandle, NativeViewCorridor* corridors, nuint corridorCount,
        NativeViewCorridorOptions* options, NativeViewCorridorMetadata* outputMetadata,
        NativeViewCorridorSummary* outputSummaries, nuint summaryCapacity,
        NativeViewCorridorSample* outputSamples, nuint sampleCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_scene_observer_path", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SceneObserverPath(
        SafeSceneHandle sceneHandle, NativeObserverPath* paths, nuint pathCount,
        NativePoint3* vertices, nuint vertexCount, NativeViewTargetPatch* patches, nuint patchCount,
        NativeObserverPathOptions* options, NativeObserverPathMetadata* outputMetadata,
        NativeObserverPathSummary* outputSummaries, nuint summaryCapacity,
        NativeObserverPathSample* outputSamples, nuint sampleCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_compute_target_view", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeTargetView(
        SafeComputeHandle computeHandle, NativeViewObserver* observers, nuint observerCount,
        NativeViewTargetPatch* patches, nuint patchCount, NativeTargetViewOptions* options,
        NativeDaenaExecutionInfo* outputExecution, NativeTargetViewMetadata* outputMetadata,
        NativeTargetViewSummary* outputSummaries, nuint summaryCapacity,
        NativeTargetViewEntry* outputEntries, nuint entryCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_compute_view_corridor", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeViewCorridor(
        SafeComputeHandle computeHandle, NativeViewCorridor* corridors, nuint corridorCount,
        NativeViewCorridorOptions* options, NativeDaenaExecutionInfo* outputExecution,
        NativeViewCorridorMetadata* outputMetadata, NativeViewCorridorSummary* outputSummaries,
        nuint summaryCapacity, NativeViewCorridorSample* outputSamples, nuint sampleCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_compute_observer_path", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus ComputeObserverPath(
        SafeComputeHandle computeHandle, NativeObserverPath* paths, nuint pathCount,
        NativePoint3* vertices, nuint vertexCount, NativeViewTargetPatch* patches, nuint patchCount,
        NativeObserverPathOptions* options, NativeDaenaExecutionInfo* outputExecution,
        NativeObserverPathMetadata* outputMetadata, NativeObserverPathSummary* outputSummaries,
        nuint summaryCapacity, NativeObserverPathSample* outputSamples, nuint sampleCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_study_rank", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyRank(
        NativeVariableSpec* variables, nuint variableCount, uint* directions, nuint objectiveCount,
        nuint constraintCount, ulong* candidateIds, ulong* generations, double* parameters,
        double* objectives, double* constraints, nuint candidateCount,
        NativeStudyMetadata* outputMetadata, NativeRankedSolution* outputSolutions,
        nuint solutionCapacity, ulong* outputParetoIds, nuint paretoCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_study_spearman", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudySpearman(
        NativeVariableSpec* variables, nuint variableCount, uint* directions, nuint objectiveCount,
        nuint constraintCount, ulong* candidateIds, double* parameters, double* objectives,
        double* constraints, nuint candidateCount, NativeSensitivityCoefficient* outputCoefficients,
        nuint coefficientCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_study_hypervolume_2d", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyHypervolume2d(
        NativeVariableSpec* variables, nuint variableCount, uint* directions, nuint constraintCount,
        ulong* candidateIds, double* parameters, double* objectives, double* constraints,
        nuint candidateCount, double referenceFirst, double referenceSecond, double* outputHypervolume);

    [DllImport(LibraryName, EntryPoint = "xv_optimizer_create", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus OptimizerCreate(
        NativeVariableSpec* variables, nuint variableCount, uint* directions, nuint objectiveCount,
        nuint constraintCount, NativeOptimizerConfig* config, ulong* outputHandle);

    [DllImport(LibraryName, EntryPoint = "xv_optimizer_ask", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus OptimizerAsk(
        SafeOptimizerHandle handle, nuint variableCount, NativeOptimizerMetadata* outputMetadata,
        ulong* outputCandidateIds, ulong* outputGenerations, double* outputParameters,
        nuint candidateCapacity, nuint parameterCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_optimizer_tell", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus OptimizerTell(
        SafeOptimizerHandle handle, nuint objectiveCount, nuint constraintCount,
        ulong* candidateIds, double* objectives, double* constraints, nuint candidateCount,
        NativeOptimizerMetadata* outputMetadata, NativeRankedSolution* outputPopulation,
        nuint populationCapacity, ulong* outputArchiveIds, nuint archiveCapacity);

    [DllImport(LibraryName, EntryPoint = "xv_optimizer_release", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus OptimizerRelease(ulong handle);
}

[StructLayout(LayoutKind.Sequential)]
internal unsafe struct NativeDaenaExecutionInfo
{
    internal uint StructureSize, Backend, Flags, Reserved;
    internal ulong RuntimeFallbackBatchCount, BatchCount, RayCount, DispatchCount;
    internal ulong UploadMicroseconds, ExecutionMicroseconds, ReadbackMicroseconds;
    internal double MaximumPrecisionErrorMeters;
    internal fixed byte AdapterName[128];
    internal fixed byte FallbackReason[256];
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewObserver
{
    internal ulong ObserverId;
    internal double PositionX, PositionY, PositionZ;
    internal double ForwardX, ForwardY, ForwardZ;
    internal double UpX, UpY, UpZ;
    internal double Weight;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewTargetPatch
{
    internal ulong TargetId;
    internal double FirstX, FirstY, FirstZ;
    internal double SecondX, SecondY, SecondZ;
    internal double ThirdX, ThirdY, ThirdZ;
    internal ulong CategoryMask;
    internal double CategoryWeight;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeTargetViewOptions
{
    internal uint StructureSize, Flags, SamplesPerPatch, Reserved;
    internal double HorizontalFovRadians, VerticalFovRadians, MaximumDistanceMeters;
    internal double EndpointClearanceMeters, DistanceReferenceMeters, DistanceExponent, DirectionExponent;
    internal ulong OccluderCategoryMask, TargetCategoryMask, GreenCategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeTargetViewEntry
{
    internal uint StructureSize, Reserved;
    internal ulong ObserverId, TargetId, CategoryMask;
    internal double PotentialSolidAngleSteradians, VisibleSolidAngleSteradians, VisibilityFraction;
    internal double FovFraction, WeightedFovScore, ConvergenceDeltaSteradians;
    internal ulong EligibleSampleCount, VisibleSampleCount, DominantBlockerObjectId;
    internal double DominantBlockedSolidAngleSteradians;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeTargetViewSummary
{
    internal uint StructureSize, Reserved;
    internal ulong ObserverId;
    internal double FovSolidAngleSteradians, PotentialTargetSolidAngleSteradians;
    internal double VisibleTargetSolidAngleSteradians, TargetViewFraction;
    internal double TargetUniverseVisibilityFraction, WeightedViewScore, GreenViewIndex;
    internal double GreenShareOfVisibleTargets;
    internal ulong DominantTargetId;
    internal double ConvergenceDeltaSteradians;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeTargetViewMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong ObserverCount, TargetCount, EntryCount, AnalysisTimeMicroseconds;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewCorridor
{
    internal ulong CorridorId;
    internal double OriginX, OriginY, OriginZ;
    internal double TargetX, TargetY, TargetZ;
    internal double UpX, UpY, UpZ, TargetRadiusMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewCorridorOptions
{
    internal uint StructureSize, SampleCount;
    internal double EndpointClearanceMeters;
    internal ulong CategoryMask;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewCorridorSummary
{
    internal uint StructureSize, Reserved;
    internal ulong CorridorId;
    internal double ApertureSolidAngleSteradians;
    internal ulong OpenSampleCount, BlockedSampleCount;
    internal double OpenFraction, OpenSolidAngleSteradians, ConvergenceDelta;
    internal ulong DominantBlockerObjectId;
    internal double DominantBlockerFraction, NearestBlockerDistanceMeters;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewCorridorSample
{
    internal uint StructureSize, State;
    internal ulong CorridorId;
    internal double ApertureX, ApertureY, ApertureZ;
    internal double DirectionX, DirectionY, DirectionZ;
    internal double ApertureDistanceMeters, FirstHitDistanceMeters;
    internal ulong BlockerObjectId, BlockerInstanceId, BlockerMeshId;
    internal uint BlockerTriangleId, Reserved;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeViewCorridorMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong CorridorCount, SamplesPerCorridor, SampleCount, AnalysisTimeMicroseconds;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativePoint3 { internal double X, Y, Z; }

[StructLayout(LayoutKind.Sequential)]
internal struct NativeObserverPath
{
    internal ulong PathId, VertexOffset, VertexCount;
    internal double UpX, UpY, UpZ, Weight;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeObserverPathOptions
{
    internal uint StructureSize, Reserved;
    internal double SpacingMeters;
    internal NativeTargetViewOptions View;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeObserverPathSample
{
    internal uint StructureSize, Reserved;
    internal ulong PathId, SampleIndex;
    internal double DistanceAlongPathMeters, PositionX, PositionY, PositionZ;
    internal double ForwardX, ForwardY, ForwardZ;
    internal double TargetViewFraction, WeightedViewScore, GreenViewIndex;
    internal ulong DominantTargetId;
    internal double ConvergenceDeltaSteradians;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeObserverPathSummary
{
    internal uint StructureSize, Reserved;
    internal ulong PathId;
    internal double PathLengthMeters;
    internal ulong SampleCount;
    internal double MeanTargetViewFraction, MeanWeightedViewScore, MeanGreenViewIndex;
    internal double MinimumWeightedViewScore, MaximumWeightedViewScore;
    internal ulong WorstSampleIndex, BestSampleIndex;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeObserverPathMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong PathCount, SampleCount, AnalysisTimeMicroseconds;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeVariableSpec
{
    internal uint StructureSize, Kind;
    internal ulong VariableId;
    internal double LowerBound, UpperBound;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeRankedSolution
{
    internal uint StructureSize, Flags;
    internal ulong CandidateId, ParetoRank;
    internal double TotalConstraintViolation, CrowdingDistance;
    internal ulong DominatedSolutionCount;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeStudyMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong CandidateCount, ObjectiveCount, ConstraintCount, FrontCount, ParetoCount;
    internal ulong AnalysisTimeMicroseconds, ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeSensitivityCoefficient
{
    internal uint StructureSize, Reserved;
    internal ulong VariableIndex, ObjectiveIndex;
    internal double SpearmanRho;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeOptimizerConfig
{
    internal uint StructureSize, Reserved;
    internal ulong PopulationSize, OffspringSize, ArchiveCapacity, Seed;
    internal double CrossoverProbability, MutationProbability;
    internal double CrossoverDistributionIndex, MutationDistributionIndex;
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeOptimizerMetadata
{
    internal uint StructureSize, Reserved;
    internal ulong Generation, EvaluationCount, PendingCandidateCount, PopulationCount, ArchiveCount;
    internal double MeanFiniteCrowdingDistance;
    internal ulong ContentHash0, ContentHash1, ContentHash2, ContentHash3;
}
