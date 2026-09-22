using Grasshopper2.Components;
using Grasshopper2.Data.Meta;

namespace Xvarna.Grasshopper2.Interop;

internal static class Gh2Data
{
    internal static void SetTwig<T>(IDataAccess access, int index, IReadOnlyList<T> values)
    {
        T[] items = values.ToArray();
        access.SetTwig(index, items, new MetaData[items.Length], new bool[items.Length]);
    }
}
