using System.Runtime.InteropServices;

namespace Xvarna.Native;

/// <summary>Information returned after probing the native XVARNA engine.</summary>
public sealed record EngineProbeResult(
    bool IsAvailable,
    Version? EngineVersion,
    AbiVersion? NativeAbi,
    string Runtime,
    string Status);

/// <summary>Loads the native engine and validates its ABI contract.</summary>
public static class EngineProbe
{
    /// <summary>Probes the native engine without allowing loader errors to escape into Rhino.</summary>
    public static EngineProbeResult TryLoad()
    {
        NativeLibraryBootstrap.Initialize();

        try
        {
            AbiVersion nativeAbi = new(
                NativeMethods.AbiVersionMajor(),
                NativeMethods.AbiVersionMinor(),
                NativeMethods.AbiVersionPatch());
            Version engineVersion = new(
                checked((int)NativeMethods.EngineVersionMajor()),
                checked((int)NativeMethods.EngineVersionMinor()),
                checked((int)NativeMethods.EngineVersionPatch()));

            bool compatible = AbiVersion.Expected.IsSatisfiedBy(nativeAbi);
            string status = compatible
                ? "Ready"
                : $"ABI mismatch: managed {AbiVersion.Expected}, native {nativeAbi}";

            return new(
                compatible,
                engineVersion,
                nativeAbi,
                RuntimeInformation.FrameworkDescription,
                status);
        }
        catch (Exception exception) when (exception is DllNotFoundException
            or EntryPointNotFoundException
            or BadImageFormatException
            or OverflowException)
        {
            return new(
                false,
                null,
                null,
                RuntimeInformation.FrameworkDescription,
                $"Native engine unavailable: {exception.Message}");
        }
    }

}
