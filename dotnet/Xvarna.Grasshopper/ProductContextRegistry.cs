using System.Runtime.CompilerServices;
using Grasshopper.Kernel;
using Rhino;
using Xvarna.Native;

namespace Xvarna.Grasshopper;

/// <summary>Immutable snapshot of document-wide product controls and recent execution events.</summary>
internal sealed record ProductContextSnapshot(
    AnalysisQualityProfile Quality,
    XvarnaUnitContext Units,
    bool QualityConfigured,
    bool UnitsConfigured,
    IReadOnlyList<string> RecentEvents);

/// <summary>
/// Document-scoped product policy. The weak table cannot keep closed Grasshopper documents alive.
/// </summary>
internal static class ProductContextRegistry
{
    private sealed class State
    {
        internal readonly object Gate = new();
        internal AnalysisQualityProfile Quality = DefaultQuality;
        internal XvarnaUnitContext? Units;
        internal bool QualityConfigured;
        internal bool UnitsConfigured;
        internal Guid QualityOwner;
        internal Guid UnitsOwner;
        internal readonly Queue<string> Events = new();
    }

    private static readonly ConditionalWeakTable<GH_Document, State> Documents = new();
    internal static AnalysisQualityProfile DefaultQuality { get; } =
        AnalysisQualityProfile.Create(XvarnaQualityPreset.Balanced);

    internal static AnalysisQualityProfile GetQuality(GH_Document? document)
    {
        if (document is null) return DefaultQuality;
        State state = Documents.GetOrCreateValue(document);
        lock (state.Gate) return state.Quality;
    }

    internal static void SetQuality(GH_Document? document, Guid owner, AnalysisQualityProfile quality)
    {
        if (document is null) return;
        State state = Documents.GetOrCreateValue(document);
        lock (state.Gate)
        {
            state.Quality = quality;
            state.QualityConfigured = true;
            state.QualityOwner = owner;
            AddEvent(state, $"Quality policy: {quality.Preset}/{quality.Execution}; generation {quality.RunGeneration}");
        }
    }

    internal static XvarnaUnitContext GetUnits(GH_Document? document)
    {
        if (document is not null)
        {
            State state = Documents.GetOrCreateValue(document);
            lock (state.Gate)
            {
                if (state.Units is not null) return state.Units;
            }
        }
        RhinoDoc? active = RhinoDoc.ActiveDoc;
        UnitSystem system = active?.ModelUnitSystem ?? UnitSystem.Meters;
        double scale = RhinoMath.UnitScale(system, UnitSystem.Meters);
        double tolerance = active?.ModelAbsoluteTolerance ?? 1.0e-6;
        return XvarnaUnitContext.Create(system.ToString(), scale, tolerance);
    }

    internal static void SetUnits(GH_Document? document, Guid owner, XvarnaUnitContext units)
    {
        if (document is null) return;
        State state = Documents.GetOrCreateValue(document);
        lock (state.Gate)
        {
            state.Units = units;
            state.UnitsConfigured = true;
            state.UnitsOwner = owner;
            AddEvent(state, $"Units: {units.ModelUnitName}; scale {units.MetersPerModelUnit:G8} m/unit");
        }
    }

    internal static int ScaleSamples(GH_Document? document, int baseline, int minimum, int maximum) =>
        GetQuality(document).ScaleSamples(baseline, minimum, maximum);

    internal static void Record(GH_Document? document, string component, string status)
    {
        if (document is null) return;
        State state = Documents.GetOrCreateValue(document);
        lock (state.Gate) AddEvent(state, $"{component}: {status}");
    }

    internal static ProductContextSnapshot Snapshot(GH_Document? document)
    {
        if (document is null)
        {
            return new(DefaultQuality, GetUnits(null), false, false, []);
        }
        State state = Documents.GetOrCreateValue(document);
        lock (state.Gate)
        {
            return new(state.Quality, state.Units ?? GetUnits(null), state.QualityConfigured,
                state.UnitsConfigured, state.Events.Reverse().ToArray());
        }
    }

    private static void AddEvent(State state, string value)
    {
        state.Events.Enqueue($"{DateTimeOffset.UtcNow:O} | {value}");
        while (state.Events.Count > 32) state.Events.Dequeue();
    }
}
