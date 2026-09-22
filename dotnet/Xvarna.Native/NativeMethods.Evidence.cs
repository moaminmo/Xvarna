using System.Runtime.InteropServices;

namespace Xvarna.Native;

internal static unsafe partial class NativeMethods
{
    [DllImport(LibraryName, EntryPoint = "xv_evidence_passport_seal_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus EvidencePassportSealJson(
        byte* inputUtf8, nuint inputLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_sensitivity_design_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SensitivityDesignJson(
        byte* inputUtf8, nuint inputLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_sensitivity_analyze_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus SensitivityAnalyzeJson(
        byte* inputUtf8, nuint inputLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_robust_scenarios_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus RobustScenariosJson(
        byte* inputUtf8, nuint inputLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_uncertainty_rank_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus UncertaintyRankJson(
        byte* inputUtf8, nuint inputLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);

    [DllImport(LibraryName, EntryPoint = "xv_multifidelity_recommend_json", CallingConvention = CallingConvention.Cdecl, ExactSpelling = true)]
    internal static extern NativeStatus MultiFidelityRecommendJson(
        byte* inputUtf8, nuint inputLength, byte* outputUtf8, nuint outputCapacity, nuint* requiredBytes);
}
