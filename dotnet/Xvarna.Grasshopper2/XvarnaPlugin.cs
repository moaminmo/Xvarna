using Grasshopper2.Framework;
using Rhino.PlugIns;

namespace Xvarna.Grasshopper2;

/// <summary>Rhino host entry point for the XVARNA Grasshopper 2 connector.</summary>
public sealed class XvarnaRhinoPlugin : PlugIn
{
    /// <summary>Gets the host-created plug-in instance.</summary>
    public static XvarnaRhinoPlugin? Instance { get; private set; }

    /// <summary>Initializes the Rhino host entry point.</summary>
    public XvarnaRhinoPlugin() => Instance = this;
}

/// <summary>Registers XVARNA with the Grasshopper 2 framework.</summary>
public sealed class XvarnaGh2Plugin : Plugin
{
    /// <summary>Initializes XVARNA's Grasshopper 2 registration.</summary>
    public XvarnaGh2Plugin() { }
}
