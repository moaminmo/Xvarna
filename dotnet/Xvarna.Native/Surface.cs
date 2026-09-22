using System.Buffers.Binary;
using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>Deterministic native longest-edge surface-grid policy.</summary>
public readonly record struct SurfaceGridOptions(
    double TargetEdgeLength,
    double SensorOffset = 0.0001,
    ulong FirstSensorId = 1,
    ulong MaximumCellCount = 1_000_000);

/// <summary>One area-aware surface triangle and its stable oriented sensor.</summary>
public readonly record struct SurfaceCell(
    ulong SensorId,
    uint SourceFaceIndex,
    uint SubdivisionDepth,
    double PositionX,
    double PositionY,
    double PositionZ,
    double NormalX,
    double NormalY,
    double NormalZ,
    double Area,
    double AX,
    double AY,
    double AZ,
    double BX,
    double BY,
    double BZ,
    double CX,
    double CY,
    double CZ);

/// <summary>Complete area-preserving surface sensor grid.</summary>
public sealed record SurfaceGridResult
{
    /// <summary>Validated generation policy in input model units.</summary>
    public required SurfaceGridOptions Options { get; init; }
    /// <summary>Source triangle count.</summary>
    public required ulong SourceFaceCount { get; init; }
    /// <summary>Invalid or degenerate source faces skipped explicitly.</summary>
    public required ulong SkippedFaceCount { get; init; }
    /// <summary>Greatest longest-edge subdivision depth.</summary>
    public required ulong MaximumSubdivisionDepth { get; init; }
    /// <summary>Valid source area in squared model units.</summary>
    public required double SourceArea { get; init; }
    /// <summary>Generated area in squared model units.</summary>
    public required double SampledArea { get; init; }
    /// <summary>Longest generated cell edge in model units.</summary>
    public required double MaximumCellEdgeLength { get; init; }
    /// <summary>Generated analysis cells.</summary>
    public required IReadOnlyList<SurfaceCell> Cells { get; init; }
    /// <summary>Deterministic BLAKE3 source/options/result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Explicit assumptions for a non-bankable photovoltaic potential proxy.</summary>
public readonly record struct PvPotentialOptions(
    double MinimumIrradianceKWhM2 = 800.0,
    double ModuleEfficiency = 0.22,
    double CoverageRatio = 0.85,
    double SystemLossFraction = 0.14);

/// <summary>Area-aware solar/PV values for one surface cell.</summary>
public readonly record struct PvPotentialCell(
    ulong SensorId,
    ulong RegionId,
    bool IsEligible,
    double AreaM2,
    double IrradianceWhM2,
    double IncidentEnergyKWh,
    double ProxyYieldKWh);

/// <summary>One edge-connected eligible solar region.</summary>
public readonly record struct PvPotentialRegion(
    ulong RegionId,
    ulong CellCount,
    double AreaM2,
    double MeanIrradianceWhM2,
    double MinimumIrradianceWhM2,
    double MaximumIrradianceWhM2,
    double IncidentEnergyKWh,
    double ProxyYieldKWh);

/// <summary>Complete area-weighted solar distribution and PV proxy result.</summary>
public sealed record PvPotentialResult
{
    /// <summary>Transparent model assumptions.</summary>
    public required PvPotentialOptions Options { get; init; }
    /// <summary>Total geometric area in m².</summary>
    public required double TotalAreaM2 { get; init; }
    /// <summary>Eligible geometric area in m².</summary>
    public required double EligibleAreaM2 { get; init; }
    /// <summary>Area-weighted mean annual irradiance in Wh/m².</summary>
    public required double MeanIrradianceWhM2 { get; init; }
    /// <summary>Area-weighted tenth percentile in Wh/m².</summary>
    public required double P10IrradianceWhM2 { get; init; }
    /// <summary>Area-weighted median in Wh/m².</summary>
    public required double P50IrradianceWhM2 { get; init; }
    /// <summary>Area-weighted ninetieth percentile in Wh/m².</summary>
    public required double P90IrradianceWhM2 { get; init; }
    /// <summary>Incident energy on all cells in kWh.</summary>
    public required double TotalIncidentEnergyKWh { get; init; }
    /// <summary>Incident energy on eligible cells in kWh.</summary>
    public required double EligibleIncidentEnergyKWh { get; init; }
    /// <summary>Proxy installed DC capacity in kWp.</summary>
    public required double CapacityKwp { get; init; }
    /// <summary>Annual output proxy after declared assumptions in kWh.</summary>
    public required double ProxyYieldKWh { get; init; }
    /// <summary>Annual proxy output divided by capacity in kWh/kWp.</summary>
    public required double SpecificYieldKWhKwp { get; init; }
    /// <summary>Per-cell results.</summary>
    public required IReadOnlyList<PvPotentialCell> Cells { get; init; }
    /// <summary>Edge-connected eligible regions.</summary>
    public required IReadOnlyList<PvPotentialRegion> Regions { get; init; }
    /// <summary>Deterministic BLAKE3 result identity.</summary>
    public required string ContentHash { get; init; }
}

/// <summary>Builds deterministic area-aware native surface grids.</summary>
public static class SurfaceGridGenerator
{
    private const int ExpectedOptionsSize = 40;
    private const int ExpectedCellSize = 152;
    private const int ExpectedMetadataSize = 96;

    /// <summary>Subdivides packed triangle geometry into an area-preserving sensor grid.</summary>
    public static unsafe SurfaceGridResult Generate(
        double[] positions,
        uint[] triangles,
        SurfaceGridOptions options)
    {
        ArgumentNullException.ThrowIfNull(positions);
        ArgumentNullException.ThrowIfNull(triangles);
        if (positions.Length % 3 != 0)
        {
            throw new ArgumentException("Position buffer length must be divisible by three.", nameof(positions));
        }
        if (triangles.Length % 3 != 0)
        {
            throw new ArgumentException("Triangle buffer length must be divisible by three.", nameof(triangles));
        }
        if (!double.IsFinite(options.TargetEdgeLength) || options.TargetEdgeLength <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Target edge length must be finite and positive.");
        }
        if (!double.IsFinite(options.SensorOffset) || options.SensorOffset < 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Sensor offset must be finite and non-negative.");
        }
        if (options.FirstSensorId == 0 || options.MaximumCellCount == 0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "First sensor ID and maximum cell count must be non-zero.");
        }

        NativeLibraryBootstrap.Initialize();
        EnsureLayout<NativeSurfaceGridOptions>(ExpectedOptionsSize);
        EnsureLayout<NativeSurfaceCell>(ExpectedCellSize);
        EnsureLayout<NativeSurfaceGridMetadata>(ExpectedMetadataSize);
        NativeSurfaceGridOptions nativeOptions = new()
        {
            StructureSize = ExpectedOptionsSize,
            TargetEdgeLength = options.TargetEdgeLength,
            SensorOffset = options.SensorOffset,
            FirstSensorId = options.FirstSensorId,
            MaximumCellCount = options.MaximumCellCount,
        };
        NativeSurfaceGridMetadata metadata = default;
        fixed (double* positionPointer = positions)
        fixed (uint* trianglePointer = triangles)
        {
            NativeStatus status = NativeMethods.SurfaceGrid(
                positionPointer,
                (nuint)(positions.Length / 3),
                trianglePointer,
                (nuint)(triangles.Length / 3),
                &nativeOptions,
                &metadata,
                null,
                0);
            ThrowOnFailure(status);
        }
        ValidateMetadata(metadata.StructureSize, ExpectedMetadataSize, nameof(NativeSurfaceGridMetadata));
        NativeSurfaceCell[] nativeCells = new NativeSurfaceCell[checked((int)metadata.CellCount)];
        fixed (double* positionPointer = positions)
        fixed (uint* trianglePointer = triangles)
        fixed (NativeSurfaceCell* cellPointer = nativeCells)
        {
            NativeStatus status = NativeMethods.SurfaceGrid(
                positionPointer,
                (nuint)(positions.Length / 3),
                trianglePointer,
                (nuint)(triangles.Length / 3),
                &nativeOptions,
                &metadata,
                cellPointer,
                (nuint)nativeCells.Length);
            ThrowOnFailure(status);
        }
        ValidateMetadata(metadata.StructureSize, ExpectedMetadataSize, nameof(NativeSurfaceGridMetadata));
        SurfaceCell[] cells = Array.ConvertAll(nativeCells, ConvertCell);
        return new()
        {
            Options = options,
            SourceFaceCount = metadata.SourceFaceCount,
            SkippedFaceCount = metadata.SkippedFaceCount,
            MaximumSubdivisionDepth = metadata.MaximumSubdivisionDepth,
            SourceArea = metadata.SourceArea,
            SampledArea = metadata.SampledArea,
            MaximumCellEdgeLength = metadata.MaximumCellEdgeLength,
            Cells = cells,
            ContentHash = FormatHash(metadata.ContentHash0, metadata.ContentHash1, metadata.ContentHash2, metadata.ContentHash3),
        };
    }

    internal static NativeSurfaceCell ConvertCell(SurfaceCell cell) => new()
    {
        StructureSize = ExpectedCellSize,
        SourceFaceIndex = cell.SourceFaceIndex,
        SensorId = cell.SensorId,
        PositionX = cell.PositionX,
        PositionY = cell.PositionY,
        PositionZ = cell.PositionZ,
        NormalX = cell.NormalX,
        NormalY = cell.NormalY,
        NormalZ = cell.NormalZ,
        Area = cell.Area,
        AX = cell.AX,
        AY = cell.AY,
        AZ = cell.AZ,
        BX = cell.BX,
        BY = cell.BY,
        BZ = cell.BZ,
        CX = cell.CX,
        CY = cell.CY,
        CZ = cell.CZ,
        SubdivisionDepth = cell.SubdivisionDepth,
    };

    internal static void ThrowOnFailure(NativeStatus status)
    {
        if (status != NativeStatus.Success)
        {
            throw new XvarnaNativeException(status);
        }
    }

    internal static void EnsureLayout<T>(int expected) where T : struct
    {
        if (Marshal.SizeOf<T>() != expected)
        {
            throw new InvalidOperationException($"Managed {typeof(T).Name} layout is {Marshal.SizeOf<T>()} bytes; expected {expected}.");
        }
    }

    internal static void ValidateMetadata(uint actual, int expected, string name)
    {
        if (actual != expected)
        {
            throw new InvalidOperationException($"Native {name} layout is {actual} bytes; expected {expected}.");
        }
    }

    internal static string FormatHash(ulong first, ulong second, ulong third, ulong fourth)
    {
        Span<byte> hash = stackalloc byte[32];
        BinaryPrimitives.WriteUInt64LittleEndian(hash[0..8], first);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[8..16], second);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[16..24], third);
        BinaryPrimitives.WriteUInt64LittleEndian(hash[24..32], fourth);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    private static SurfaceCell ConvertCell(NativeSurfaceCell cell)
    {
        ValidateMetadata(cell.StructureSize, ExpectedCellSize, nameof(NativeSurfaceCell));
        return new(
            cell.SensorId,
            cell.SourceFaceIndex,
            cell.SubdivisionDepth,
            cell.PositionX,
            cell.PositionY,
            cell.PositionZ,
            cell.NormalX,
            cell.NormalY,
            cell.NormalZ,
            cell.Area,
            cell.AX,
            cell.AY,
            cell.AZ,
            cell.BX,
            cell.BY,
            cell.BZ,
            cell.CX,
            cell.CY,
            cell.CZ);
    }
}

/// <summary>Computes area-weighted solar hotspots, connected regions, and PV proxy output.</summary>
public static class SurfacePotentialAnalyzer
{
    private const int ExpectedOptionsSize = 48;
    private const int ExpectedCellSize = 56;
    private const int ExpectedRegionSize = 72;
    private const int ExpectedMetadataSize = 152;

    /// <summary>Analyzes annual irradiance values aligned one-to-one with a surface grid.</summary>
    public static unsafe PvPotentialResult Analyze(
        SurfaceGridResult grid,
        IReadOnlyList<double> irradianceWhM2,
        PvPotentialOptions options,
        double unitScaleToMeters)
    {
        ArgumentNullException.ThrowIfNull(grid);
        ArgumentNullException.ThrowIfNull(irradianceWhM2);
        if (irradianceWhM2.Count != grid.Cells.Count)
        {
            throw new ArgumentException("Irradiance count must match surface-grid cell count.", nameof(irradianceWhM2));
        }
        if (!double.IsFinite(unitScaleToMeters) || unitScaleToMeters <= 0.0)
        {
            throw new ArgumentOutOfRangeException(nameof(unitScaleToMeters));
        }
        if (!double.IsFinite(options.MinimumIrradianceKWhM2) || options.MinimumIrradianceKWhM2 < 0.0
            || !double.IsFinite(options.ModuleEfficiency) || options.ModuleEfficiency is < 0.0 or > 1.0
            || !double.IsFinite(options.CoverageRatio) || options.CoverageRatio is < 0.0 or > 1.0
            || !double.IsFinite(options.SystemLossFraction) || options.SystemLossFraction is < 0.0 or > 1.0)
        {
            throw new ArgumentOutOfRangeException(nameof(options), "Threshold must be non-negative and all ratios must be between zero and one.");
        }

        NativeLibraryBootstrap.Initialize();
        SurfaceGridGenerator.EnsureLayout<NativePvPotentialOptions>(ExpectedOptionsSize);
        SurfaceGridGenerator.EnsureLayout<NativePvPotentialCell>(ExpectedCellSize);
        SurfaceGridGenerator.EnsureLayout<NativePvPotentialRegion>(ExpectedRegionSize);
        SurfaceGridGenerator.EnsureLayout<NativePvPotentialMetadata>(ExpectedMetadataSize);
        NativeSurfaceCell[] nativeCells = grid.Cells.Select(SurfaceGridGenerator.ConvertCell).ToArray();
        double[] values = irradianceWhM2.ToArray();
        NativePvPotentialOptions nativeOptions = new()
        {
            StructureSize = ExpectedOptionsSize,
            UnitScaleToMeters = unitScaleToMeters,
            MinimumIrradianceWhM2 = options.MinimumIrradianceKWhM2 * 1000.0,
            ModuleEfficiency = options.ModuleEfficiency,
            CoverageRatio = options.CoverageRatio,
            SystemLossFraction = options.SystemLossFraction,
        };
        NativePvPotentialMetadata metadata = default;
        fixed (NativeSurfaceCell* cellPointer = nativeCells)
        fixed (double* valuePointer = values)
        {
            NativeStatus status = NativeMethods.SurfacePvPotential(
                cellPointer,
                valuePointer,
                (nuint)nativeCells.Length,
                &nativeOptions,
                &metadata,
                null,
                0,
                null,
                0);
            SurfaceGridGenerator.ThrowOnFailure(status);
        }
        SurfaceGridGenerator.ValidateMetadata(metadata.StructureSize, ExpectedMetadataSize, nameof(NativePvPotentialMetadata));
        NativePvPotentialCell[] outputCells = new NativePvPotentialCell[checked((int)metadata.CellCount)];
        NativePvPotentialRegion[] outputRegions = new NativePvPotentialRegion[checked((int)metadata.RegionCount)];
        fixed (NativeSurfaceCell* cellPointer = nativeCells)
        fixed (double* valuePointer = values)
        fixed (NativePvPotentialCell* outputCellPointer = outputCells)
        fixed (NativePvPotentialRegion* outputRegionPointer = outputRegions)
        {
            NativeStatus status = NativeMethods.SurfacePvPotential(
                cellPointer,
                valuePointer,
                (nuint)nativeCells.Length,
                &nativeOptions,
                &metadata,
                outputCellPointer,
                (nuint)outputCells.Length,
                outputRegionPointer,
                (nuint)outputRegions.Length);
            SurfaceGridGenerator.ThrowOnFailure(status);
        }
        return new()
        {
            Options = options,
            TotalAreaM2 = metadata.TotalAreaM2,
            EligibleAreaM2 = metadata.EligibleAreaM2,
            MeanIrradianceWhM2 = metadata.MeanIrradianceWhM2,
            P10IrradianceWhM2 = metadata.P10IrradianceWhM2,
            P50IrradianceWhM2 = metadata.P50IrradianceWhM2,
            P90IrradianceWhM2 = metadata.P90IrradianceWhM2,
            TotalIncidentEnergyKWh = metadata.TotalIncidentEnergyKWh,
            EligibleIncidentEnergyKWh = metadata.EligibleIncidentEnergyKWh,
            CapacityKwp = metadata.CapacityKwp,
            ProxyYieldKWh = metadata.ProxyYieldKWh,
            SpecificYieldKWhKwp = metadata.SpecificYieldKWhKwp,
            Cells = Array.ConvertAll(outputCells, cell => new PvPotentialCell(
                cell.SensorId,
                cell.RegionId,
                (cell.Flags & 1) != 0,
                cell.AreaM2,
                cell.IrradianceWhM2,
                cell.IncidentEnergyKWh,
                cell.ProxyYieldKWh)),
            Regions = Array.ConvertAll(outputRegions, region => new PvPotentialRegion(
                region.RegionId,
                region.CellCount,
                region.AreaM2,
                region.MeanIrradianceWhM2,
                region.MinimumIrradianceWhM2,
                region.MaximumIrradianceWhM2,
                region.IncidentEnergyKWh,
                region.ProxyYieldKWh)),
            ContentHash = SurfaceGridGenerator.FormatHash(
                metadata.ContentHash0,
                metadata.ContentHash1,
                metadata.ContentHash2,
                metadata.ContentHash3),
        };
    }
}
