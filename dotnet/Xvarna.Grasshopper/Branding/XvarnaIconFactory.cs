using System.Collections.Concurrent;
using System.Drawing;

namespace Xvarna.Grasshopper.Branding;

/// <summary>Loads the operation-specific, embedded 24-pixel component icons.</summary>
internal static class XvarnaIconFactory
{
    private static readonly ConcurrentDictionary<string, Bitmap> Cache = new(StringComparer.Ordinal);
    internal static Bitmap For(Type componentType) => Cache.GetOrAdd(componentType.Name, Load);
    private static Bitmap Load(string typeName)
    {
        string name = typeName.EndsWith("Component", StringComparison.Ordinal) ? typeName[..^9] : typeName;
        using Stream stream = typeof(XvarnaIconFactory).Assembly.GetManifestResourceStream($"Xvarna.Icons.{name}.png")
            ?? throw new InvalidOperationException($"Missing component icon: {name}");
        using Bitmap image = new(stream);
        return new Bitmap(image); // Detach from the resource stream; cache owns the bitmap.
    }
}
