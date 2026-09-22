using System.Drawing;
using Xvarna.Native;

namespace Xvarna.Grasshopper.Components.Solar;

internal static class SkyPalette
{
    internal static Color Viridis(double value)
    {
        XvarnaColor color = XvarnaLegend.Color(value, XvarnaPalette.Viridis);
        return Color.FromArgb(color.Alpha, color.Red, color.Green, color.Blue);
    }
}
