using System.Collections.Concurrent;
using Grasshopper2.Components;
using Grasshopper2.UI;
using Grasshopper2.UI.Icon;
using Grasshopper2.UI.Icon.Vector;
using GrasshopperIO;

namespace Xvarna.Grasshopper2.Interop;

/// <summary>Shared GH2 product shell for branding and solution cancellation.</summary>
public abstract class XvarnaComponent : Component
{
    private static readonly AsyncLocal<CancellationToken> CurrentCancellation = new();

    /// <summary>Initializes a branded XVARNA component.</summary>
    protected XvarnaComponent(Nomen nomen) : base(nomen) => ApplyProductIcon();

    /// <summary>Restores a branded XVARNA component.</summary>
    protected XvarnaComponent(IReader reader) : base(reader) => ApplyProductIcon();

    /// <summary>The cancellation token owned by the current GH2 solution.</summary>
    protected static CancellationToken SolutionCancellationToken => CurrentCancellation.Value;

    /// <inheritdoc />
    protected sealed override void Process(IDataAccess[] accesses, CancellationToken cancellationToken)
    {
        CancellationToken previous = CurrentCancellation.Value;
        CurrentCancellation.Value = cancellationToken;
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            base.Process(accesses, cancellationToken);
            cancellationToken.ThrowIfCancellationRequested();
        }
        finally
        {
            CurrentCancellation.Value = previous;
        }
    }

    private void ApplyProductIcon() => ModifyIcon(XvarnaGh2Icons.For(GetType()), true);
}

/// <summary>Loads operation-specific vector icons shared with GH1.</summary>
internal static class XvarnaGh2Icons
{
    private static readonly ConcurrentDictionary<string, IIcon> Cache = new(StringComparer.Ordinal);
    internal static IIcon For(Type componentType) => Cache.GetOrAdd(componentType.Name, Load);
    private static IIcon Load(string typeName)
    {
        string name = typeName.EndsWith("Component", StringComparison.Ordinal) ? typeName[..^9] : typeName;
        using Stream stream = typeof(XvarnaGh2Icons).Assembly.GetManifestResourceStream($"Xvarna.Icons.{name}.svg")
            ?? throw new InvalidOperationException($"Missing component icon: {name}");
        using StreamReader reader = new(stream);
        return SvgIcon.FromXml(reader.ReadToEnd());
    }
}
