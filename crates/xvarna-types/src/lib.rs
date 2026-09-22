//! Canonical identifiers, versions, and shared data contracts for XVARNA.

#![forbid(unsafe_code)]

use core::fmt;

/// The current native ABI version.
pub const ABI_VERSION: AbiVersion = AbiVersion::new(0, 19, 0);

/// A semantic version used at the native ABI boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(C)]
pub struct AbiVersion {
    /// Major ABI version. A mismatch is not compatible.
    pub major: u32,
    /// Minor ABI version. Additive changes increment this value.
    pub minor: u32,
    /// Patch ABI version.
    pub patch: u32,
}

impl AbiVersion {
    /// Creates an ABI version.
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Returns true when a native provider satisfies this required ABI.
    #[must_use]
    pub const fn is_satisfied_by(self, provider: Self) -> bool {
        self.major == provider.major && provider.minor >= self.minor
    }
}

impl fmt::Display for AbiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Returns the Cargo package version for the engine build.
#[must_use]
pub const fn engine_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(pub u64);

        impl $name {
            /// Creates an identifier from its stable integer representation.
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            /// Returns the stable integer representation.
            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

define_id!(ObjectId, "A stable identifier for a source scene object.");
define_id!(InstanceId, "A stable identifier for a geometry instance.");
define_id!(MeshId, "A stable identifier for a reusable mesh resource.");
define_id!(SensorId, "A stable identifier for an analysis sensor.");
define_id!(TargetId, "A stable identifier for a visibility target.");

/// The validity state common to all result envelopes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum ValidityState {
    /// The result passed all required checks.
    Valid = 0,
    /// The result is usable, but warnings must be inspected.
    ValidWithWarnings = 1,
    /// The result uses an explicitly labeled approximation.
    Approximate = 2,
    /// The result belongs to an older input revision.
    Stale = 3,
    /// The job was cancelled before a final result was produced.
    Cancelled = 4,
    /// The result contains a valid but incomplete subset.
    Partial = 5,
    /// The result failed validation and must not be used.
    Invalid = 6,
    /// The result was validated against a declared reference workflow.
    ReferenceValidated = 7,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_requirement_needs_matching_major_and_sufficient_provider_minor() {
        let required = AbiVersion::new(1, 3, 0);
        assert!(!required.is_satisfied_by(AbiVersion::new(1, 2, 7)));
        assert!(required.is_satisfied_by(AbiVersion::new(1, 3, 0)));
        assert!(required.is_satisfied_by(AbiVersion::new(1, 4, 0)));
        assert!(!required.is_satisfied_by(AbiVersion::new(2, 3, 0)));
    }

    #[test]
    fn stable_ids_round_trip() {
        let id = ObjectId::new(42);
        assert_eq!(id.get(), 42);
        assert_eq!(id.to_string(), "42");
    }
}
