using System.Globalization;
using Grasshopper.Kernel.Types;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Visibility;

internal static class IntelligenceHelpers
{
    internal static T? Unwrap<T>(object? value) where T : class => value switch
    {
        T typed => typed,
        GH_ObjectWrapper { Value: T typed } => typed,
        _ => null,
    };

    internal static ulong Parse(string text, string name, bool allowAll = false)
    {
        if (allowAll && text == "-1") return ulong.MaxValue;
        return ulong.TryParse(text, NumberStyles.None, CultureInfo.InvariantCulture, out ulong value)
            ? value : throw new ArgumentException($"{name} '{text}' is not a valid unsigned 64-bit integer.");
    }

    internal static T Select<T>(IReadOnlyList<T> values, int index, T fallback) =>
        values.Count == 0 ? fallback : values[values.Count == 1 ? 0 : index];

    internal static bool Compatible(int targetCount, int count) => count is 0 or 1 || count == targetCount;
}
