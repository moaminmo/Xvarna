using System.Drawing;
using Grasshopper.Kernel;

namespace Xvarna.Grasshopper;

/// <summary>Grasshopper assembly metadata for XVARNA.</summary>
public sealed class XvarnaAssemblyInfo : GH_AssemblyInfo
{
    /// <inheritdoc />
    public override string Name => "XVARNA";

    /// <inheritdoc />
    public override Bitmap? Icon => null;

    /// <inheritdoc />
    public override string Description =>
        "High-performance environmental and spatial intelligence for computational design.";

    /// <inheritdoc />
    public override Guid Id => new("e6b1c42a-0e93-4ef8-a833-5c12dc2e2d11");

    /// <inheritdoc />
    public override string AuthorName => "XVARNA contributors";

    /// <inheritdoc />
    public override string AuthorContact => "https://github.com/moaminmo/Xvarna";
}
