namespace Xvarna.Grasshopper;

/// <summary>Keeps retained results paired with the input that produced them, per solve iteration.</summary>
internal sealed class AnalysisResultCache<TWork, TResult> where TResult : class
{
    internal static string SchedulingScope(Guid component, int iteration) => $"grasshopper:{component:D}:{iteration}";
    internal sealed record Entry(TWork Work, TResult Result, long RunGeneration);
    private readonly Dictionary<int, Entry> entries = new();
    internal bool TryGet(int iteration, out Entry? entry) => entries.TryGetValue(iteration, out entry);
    internal void Store(int iteration, TWork work, TResult result, long generation) =>
        entries[iteration] = new(work, result, generation);
}
