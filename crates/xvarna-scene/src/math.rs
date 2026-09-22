//! Affine transforms used by canonical scene instances.

use xvarna_geometry::{Aabb, Vec3};

const AFFINE_EPSILON: f64 = 1.0e-15;

/// Row-major affine 4×4 transform using f64 values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    values: [f64; 16],
}

impl Transform {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        values: [
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ],
    };

    /// Validates a row-major affine matrix.
    pub fn try_from_row_major(values: [f64; 16]) -> Result<Self, TransformError> {
        if values.iter().any(|value| !value.is_finite()) {
            return Err(TransformError::NonFinite);
        }
        if values[12].abs() > AFFINE_EPSILON
            || values[13].abs() > AFFINE_EPSILON
            || values[14].abs() > AFFINE_EPSILON
            || (values[15] - 1.0).abs() > AFFINE_EPSILON
        {
            return Err(TransformError::NotAffine);
        }
        let transform = Self { values };
        if transform.linear_determinant().abs() <= 1.0e-15 {
            return Err(TransformError::Singular);
        }
        Ok(transform)
    }

    /// Returns the row-major values.
    #[must_use]
    pub const fn to_row_major(self) -> [f64; 16] {
        self.values
    }

    /// Transforms a position including translation.
    #[must_use]
    pub const fn transform_point(self, point: Vec3) -> Vec3 {
        Vec3::new(
            self.values[0].mul_add(
                point.x,
                self.values[1].mul_add(point.y, self.values[2].mul_add(point.z, self.values[3])),
            ),
            self.values[4].mul_add(
                point.x,
                self.values[5].mul_add(point.y, self.values[6].mul_add(point.z, self.values[7])),
            ),
            self.values[8].mul_add(
                point.x,
                self.values[9].mul_add(point.y, self.values[10].mul_add(point.z, self.values[11])),
            ),
        )
    }

    /// Transforms a direction without translation.
    #[must_use]
    pub fn transform_direction(self, direction: Vec3) -> Vec3 {
        Vec3::new(
            self.values[0].mul_add(
                direction.x,
                self.values[1].mul_add(direction.y, self.values[2] * direction.z),
            ),
            self.values[4].mul_add(
                direction.x,
                self.values[5].mul_add(direction.y, self.values[6] * direction.z),
            ),
            self.values[8].mul_add(
                direction.x,
                self.values[9].mul_add(direction.y, self.values[10] * direction.z),
            ),
        )
    }

    /// Returns the inverse affine transform.
    #[allow(clippy::suboptimal_flops)]
    pub fn inverse(self) -> Result<Self, TransformError> {
        let m = self.values;
        let determinant = self.linear_determinant();
        if determinant.abs() <= 1.0e-15 {
            return Err(TransformError::Singular);
        }
        let inverse_determinant = determinant.recip();
        let i00 = (m[5] * m[10] - m[6] * m[9]) * inverse_determinant;
        let i01 = (m[2] * m[9] - m[1] * m[10]) * inverse_determinant;
        let i02 = (m[1] * m[6] - m[2] * m[5]) * inverse_determinant;
        let i10 = (m[6] * m[8] - m[4] * m[10]) * inverse_determinant;
        let i11 = (m[0] * m[10] - m[2] * m[8]) * inverse_determinant;
        let i12 = (m[2] * m[4] - m[0] * m[6]) * inverse_determinant;
        let i20 = (m[4] * m[9] - m[5] * m[8]) * inverse_determinant;
        let i21 = (m[1] * m[8] - m[0] * m[9]) * inverse_determinant;
        let i22 = (m[0] * m[5] - m[1] * m[4]) * inverse_determinant;
        let tx = -i00.mul_add(m[3], i01.mul_add(m[7], i02 * m[11]));
        let ty = -i10.mul_add(m[3], i11.mul_add(m[7], i12 * m[11]));
        let tz = -i20.mul_add(m[3], i21.mul_add(m[7], i22 * m[11]));
        Ok(Self {
            values: [
                i00, i01, i02, tx, i10, i11, i12, ty, i20, i21, i22, tz, 0.0, 0.0, 0.0, 1.0,
            ],
        })
    }

    /// Returns the determinant of the linear 3×3 block.
    #[must_use]
    #[allow(clippy::suboptimal_flops)]
    pub fn linear_determinant(self) -> f64 {
        let m = self.values;
        m[0] * (m[5] * m[10] - m[6] * m[9]) - m[1] * (m[4] * m[10] - m[6] * m[8])
            + m[2] * (m[4] * m[9] - m[5] * m[8])
    }

    /// Returns true when the transform reverses orientation.
    #[must_use]
    pub fn is_mirrored(self) -> bool {
        self.linear_determinant() < 0.0
    }

    /// Scales the translation from model units into canonical metres.
    #[must_use]
    pub fn with_translation_scale(mut self, scale: f64) -> Self {
        self.values[3] *= scale;
        self.values[7] *= scale;
        self.values[11] *= scale;
        self
    }

    /// Subtracts a canonical origin from the translation.
    #[must_use]
    pub fn rebased(mut self, origin: Vec3) -> Self {
        self.values[3] -= origin.x;
        self.values[7] -= origin.y;
        self.values[11] -= origin.z;
        self
    }

    /// Transforms all eight corners and returns conservative world bounds.
    #[must_use]
    pub fn transform_aabb(self, bounds: Aabb) -> Aabb {
        let mut transformed = Aabb::from_point(self.transform_point(bounds.min));
        for x in [bounds.min.x, bounds.max.x] {
            for y in [bounds.min.y, bounds.max.y] {
                for z in [bounds.min.z, bounds.max.z] {
                    transformed.include(self.transform_point(Vec3::new(x, y, z)));
                }
            }
        }
        transformed
    }
}

/// Invalid affine transform.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransformError {
    /// A matrix value is NaN or infinity.
    NonFinite,
    /// The last row does not represent an affine transform.
    NotAffine,
    /// The linear block cannot be inverted.
    Singular,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_round_trips_non_uniform_transform() {
        let transform = Transform::try_from_row_major([
            2.0, 0.0, 0.0, 4.0, 0.0, 3.0, 0.0, -5.0, 0.0, 0.0, 0.5, 6.0, 0.0, 0.0, 0.0, 1.0,
        ])
        .expect("transform is invertible");
        let point = Vec3::new(1.0, 2.0, 3.0);
        let world = transform.transform_point(point);
        let round_trip = transform
            .inverse()
            .expect("inverse exists")
            .transform_point(world);
        assert!((round_trip.x - point.x).abs() < 1.0e-12);
        assert!((round_trip.y - point.y).abs() < 1.0e-12);
        assert!((round_trip.z - point.z).abs() < 1.0e-12);
    }

    #[test]
    fn singular_and_projective_transforms_are_rejected() {
        let mut singular = Transform::IDENTITY.to_row_major();
        singular[0] = 0.0;
        assert_eq!(
            Transform::try_from_row_major(singular),
            Err(TransformError::Singular)
        );

        let mut projective = Transform::IDENTITY.to_row_major();
        projective[12] = 1.0;
        assert_eq!(
            Transform::try_from_row_major(projective),
            Err(TransformError::NotAffine)
        );
    }
}
