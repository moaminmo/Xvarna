using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    [DllImport(LibraryName, EntryPoint = "xv_study_manifest_schema_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyManifestSchemaJson(
        byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_study_workspace_create", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyWorkspaceCreate(
        byte* rootUtf8, nuint rootLength, byte* manifestUtf8, nuint manifestLength,
        byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_study_workspace_status", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyWorkspaceStatus(
        byte* rootUtf8, nuint rootLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_study_workspace_begin_batch", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyWorkspaceBeginBatch(
        byte* rootUtf8, nuint rootLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_study_workspace_commit_batch", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyWorkspaceCommitBatch(
        byte* rootUtf8, nuint rootLength, byte* evaluationsUtf8, nuint evaluationsLength,
        byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_study_workspace_report", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus StudyWorkspaceReport(
        byte* rootUtf8, nuint rootLength, byte* outputDirectoryUtf8, nuint outputDirectoryLength,
        NativeStudyReportOptions* options,
        byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);
}

[StructLayout(LayoutKind.Sequential)]
internal struct NativeStudyReportOptions
{
    internal uint StructureSize, Reserved;
    internal ulong BootstrapSamples, PermutationSamples;
    internal double ConfidenceLevel, Alpha;
    internal ulong Seed;
}
