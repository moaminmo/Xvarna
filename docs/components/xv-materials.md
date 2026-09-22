# XV Materials

Builds one immutable optical catalog shared by DAENA and HVARE. Define unique non-zero Material IDs with visible transmittance, direct-solar transmittance, and diffuse reflectance; then assign them to exact `XV Scene` Object IDs. All fractions are in `[0,1]`, and each channel plus reflectance must not exceed one.

Unassigned objects are opaque. Coefficients apply per geometric interaction, so a closed glazing mesh can be crossed more than once. The result preserves a deterministic BLAKE3 identity for scenario provenance.
