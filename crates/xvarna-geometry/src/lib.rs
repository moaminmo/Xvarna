//! Reference geometry primitives and validation for XVARNA.

#![forbid(unsafe_code)]

use core::ops::{Add, Mul, Sub};
use xvarna_types::ObjectId;

mod audit;
mod processing;
mod surface;

pub use audit::{
    Aabb, MeshAuditError, MeshAuditOptions, MeshAuditReport, MeshFaceFlags, MeshRepairReport,
    MeshVertexFlags, audit_mesh, hash_mesh, repair_mesh_conservative,
};
pub use processing::{
    MeshClipError, MeshClipReport, MeshHealError, MeshHealOptions, MeshHealReport,
    clip_mesh_to_aabb, heal_mesh,
};
pub use surface::{
    SurfaceCell, SurfaceGridError, SurfaceGridOptions, SurfaceGridResult, generate_surface_grid,
};

/// A finite three-dimensional vector using double precision.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    /// X component.
    pub x: f64,
    /// Y component.
    pub y: f64,
    /// Z component.
    pub z: f64,
}

impl Vec3 {
    /// The zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

    /// Creates a vector.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Returns true when all components are finite.
    #[must_use]
    pub const fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }

    /// Computes the dot product.
    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x
            .mul_add(other.x, self.y.mul_add(other.y, self.z * other.z))
    }

    /// Computes the cross product.
    #[must_use]
    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y.mul_add(other.z, -(self.z * other.y)),
            self.z.mul_add(other.x, -(self.x * other.z)),
            self.x.mul_add(other.y, -(self.y * other.x)),
        )
    }

    /// Returns the squared length.
    #[must_use]
    pub fn length_squared(self) -> f64 {
        self.dot(self)
    }

    /// Returns a normalized vector, or `None` for a non-finite or zero vector.
    #[must_use]
    pub fn normalized(self) -> Option<Self> {
        let length_squared = self.length_squared();
        if !length_squared.is_finite() || length_squared <= 0.0 {
            return None;
        }
        Some(self * length_squared.sqrt().recip())
    }
}

impl Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f64) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

/// A validated ray segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Ray origin.
    pub origin: Vec3,
    /// Unit-length ray direction.
    pub direction: Vec3,
    /// Inclusive lower distance bound.
    pub t_min: f64,
    /// Inclusive upper distance bound.
    pub t_max: f64,
}

impl Ray {
    /// Creates a ray after validating and normalizing its direction.
    pub fn try_new(
        origin: Vec3,
        direction: Vec3,
        t_min: f64,
        t_max: f64,
    ) -> Result<Self, RayError> {
        if !origin.is_finite() {
            return Err(RayError::NonFiniteOrigin);
        }
        let direction = direction.normalized().ok_or(RayError::InvalidDirection)?;
        if !t_min.is_finite() || t_min < 0.0 {
            return Err(RayError::InvalidMinimumDistance);
        }
        if t_max.is_nan() || t_max < t_min {
            return Err(RayError::InvalidMaximumDistance);
        }
        Ok(Self {
            origin,
            direction,
            t_min,
            t_max,
        })
    }
}

/// Validation errors for [`Ray`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RayError {
    /// The origin contains NaN or infinity.
    NonFiniteOrigin,
    /// The direction is zero or non-finite.
    InvalidDirection,
    /// The minimum distance is negative or non-finite.
    InvalidMinimumDistance,
    /// The maximum distance is NaN or smaller than the minimum.
    InvalidMaximumDistance,
}

/// A triangle with a stable source object identifier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Triangle {
    /// First vertex.
    pub a: Vec3,
    /// Second vertex.
    pub b: Vec3,
    /// Third vertex.
    pub c: Vec3,
    /// Stable source object identifier.
    pub object_id: ObjectId,
}

/// A reference intersection result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriangleHit {
    /// Distance along the normalized ray.
    pub distance: f64,
    /// First barycentric coordinate.
    pub barycentric_u: f64,
    /// Second barycentric coordinate.
    pub barycentric_v: f64,
    /// Stable source object identifier.
    pub object_id: ObjectId,
}

/// Intersects a ray with a triangle using a double-precision reference path.
///
/// This implementation is intentionally simple and auditable. A watertight
/// production kernel will be differential-tested against it and the analytic
/// corpus defined in the master specification.
#[must_use]
pub fn intersect_triangle_reference(ray: Ray, triangle: Triangle) -> Option<TriangleHit> {
    const EPSILON: f64 = 1.0e-12;

    let first_edge = triangle.b - triangle.a;
    let second_edge = triangle.c - triangle.a;
    let p = ray.direction.cross(second_edge);
    let determinant = first_edge.dot(p);
    if determinant.abs() <= EPSILON {
        return None;
    }

    let inverse_determinant = determinant.recip();
    let origin_delta = ray.origin - triangle.a;
    let u = origin_delta.dot(p) * inverse_determinant;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = origin_delta.cross(first_edge);
    let v = ray.direction.dot(q) * inverse_determinant;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let distance = second_edge.dot(q) * inverse_determinant;
    if distance < ray.t_min || distance > ray.t_max {
        return None;
    }

    Some(TriangleHit {
        distance,
        barycentric_u: u,
        barycentric_v: v,
        object_id: triangle.object_id,
    })
}

/// A triangle mesh in canonical double-precision coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Vertex positions.
    pub positions: Vec<Vec3>,
    /// Triangle vertex indices.
    pub triangles: Vec<[u32; 3]>,
}

impl Mesh {
    /// Returns all structural issues found without mutating the mesh.
    #[must_use]
    pub fn validate(&self, absolute_tolerance: f64) -> Vec<MeshIssue> {
        let mut issues = Vec::new();
        let tolerance_squared = absolute_tolerance.max(0.0).powi(2);

        for (vertex_index, position) in self.positions.iter().copied().enumerate() {
            if !position.is_finite() {
                issues.push(MeshIssue::NonFiniteVertex { vertex_index });
            }
        }

        for (triangle_index, indices) in self.triangles.iter().copied().enumerate() {
            let Some(vertices) = indices
                .map(|index| self.positions.get(index as usize).copied())
                .into_iter()
                .collect::<Option<Vec<_>>>()
            else {
                issues.push(MeshIssue::IndexOutOfBounds {
                    triangle_index,
                    indices,
                    vertex_count: self.positions.len(),
                });
                continue;
            };

            let doubled_area_squared = (vertices[1] - vertices[0])
                .cross(vertices[2] - vertices[0])
                .length_squared();
            if doubled_area_squared <= 4.0 * tolerance_squared * tolerance_squared {
                issues.push(MeshIssue::DegenerateTriangle { triangle_index });
            }
        }

        issues
    }
}

/// A non-mutating mesh validation finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MeshIssue {
    /// A vertex contains NaN or infinity.
    NonFiniteVertex {
        /// Index of the invalid vertex.
        vertex_index: usize,
    },
    /// A triangle references a vertex outside the position buffer.
    IndexOutOfBounds {
        /// Index of the invalid triangle.
        triangle_index: usize,
        /// Invalid triangle indices.
        indices: [u32; 3],
        /// Number of available vertices.
        vertex_count: usize,
    },
    /// A triangle has negligible area under the selected tolerance.
    DegenerateTriangle {
        /// Index of the degenerate triangle.
        triangle_index: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_triangle() -> Triangle {
        Triangle {
            a: Vec3::new(0.0, 0.0, 0.0),
            b: Vec3::new(1.0, 0.0, 0.0),
            c: Vec3::new(0.0, 1.0, 0.0),
            object_id: ObjectId::new(7),
        }
    }

    #[test]
    fn reference_intersection_hits_unit_triangle() {
        let ray = Ray::try_new(
            Vec3::new(0.25, 0.25, 1.0),
            Vec3::new(0.0, 0.0, -2.0),
            0.0,
            10.0,
        )
        .expect("test ray is valid");

        let hit = intersect_triangle_reference(ray, unit_triangle()).expect("triangle must be hit");
        assert!((hit.distance - 1.0).abs() < 1.0e-12);
        assert_eq!(hit.object_id, ObjectId::new(7));
    }

    #[test]
    fn reference_intersection_rejects_miss() {
        let ray = Ray::try_new(
            Vec3::new(2.0, 2.0, 1.0),
            Vec3::new(0.0, 0.0, -1.0),
            0.0,
            10.0,
        )
        .expect("test ray is valid");
        assert!(intersect_triangle_reference(ray, unit_triangle()).is_none());
    }

    #[test]
    fn mesh_validation_reports_invalid_index_and_degenerate_triangle() {
        let mesh = Mesh {
            positions: vec![Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)],
            triangles: vec![[0, 1, 1], [0, 1, 2]],
        };
        let issues = mesh.validate(1.0e-6);
        assert_eq!(issues.len(), 2);
        assert!(matches!(issues[0], MeshIssue::DegenerateTriangle { .. }));
        assert!(matches!(issues[1], MeshIssue::IndexOutOfBounds { .. }));
    }
}
