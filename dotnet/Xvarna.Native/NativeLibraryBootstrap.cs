using System.Reflection;
using System.Runtime.InteropServices;
using System.Threading;

namespace Xvarna.Native;

internal static class NativeLibraryBootstrap
{
    private static int resolverInstalled;

    internal static void Initialize()
    {
        if (Interlocked.Exchange(ref resolverInstalled, 1) != 0)
        {
            return;
        }

        NativeLibrary.SetDllImportResolver(
            typeof(NativeLibraryBootstrap).Assembly,
            ResolveNativeLibrary);
    }

    private static nint ResolveNativeLibrary(
        string libraryName,
        Assembly assembly,
        DllImportSearchPath? searchPath)
    {
        _ = searchPath;
        if (!string.Equals(libraryName, NativeMethods.LibraryName, StringComparison.Ordinal))
        {
            return nint.Zero;
        }

        string? assemblyDirectory = Path.GetDirectoryName(assembly.Location);
        if (string.IsNullOrWhiteSpace(assemblyDirectory))
        {
            return nint.Zero;
        }

        string fileName = RuntimeInformation.IsOSPlatform(OSPlatform.Windows)
            ? $"{NativeMethods.LibraryName}.dll"
            : RuntimeInformation.IsOSPlatform(OSPlatform.OSX)
                ? $"lib{NativeMethods.LibraryName}.dylib"
                : $"lib{NativeMethods.LibraryName}.so";

        string[] candidates =
        [
            Path.Combine(assemblyDirectory, fileName),
            Path.Combine(assemblyDirectory, "runtimes", "win-x64", "native", fileName),
        ];

        foreach (string candidate in candidates)
        {
            if (File.Exists(candidate) && NativeLibrary.TryLoad(candidate, out nint handle))
            {
                return handle;
            }
        }

        return nint.Zero;
    }
}
