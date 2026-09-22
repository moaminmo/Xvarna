namespace Xvarna.Native;

/// <summary>Semantic version of the stable native ABI.</summary>
public readonly record struct AbiVersion(uint Major, uint Minor, uint Patch)
{
    /// <summary>The ABI expected by this managed connector.</summary>
    public static AbiVersion Expected { get; } = new(0, 19, 0);

    /// <summary>Returns whether a native provider satisfies this required ABI.</summary>
    public bool IsSatisfiedBy(AbiVersion native) =>
        Major == native.Major && native.Minor >= Minor;

    /// <inheritdoc />
    public override string ToString() => $"{Major}.{Minor}.{Patch}";
}
