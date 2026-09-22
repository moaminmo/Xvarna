//! Validated EPW weather, schedules, and high-accuracy solar positions for XVARNA.

#![forbid(unsafe_code)]

mod epw;
mod weather;

pub use epw::{
    EpwDataPeriod, EpwError, EpwLocation, EpwMissingCounts, EpwRecord, EpwWeather, parse_epw,
    with_simulation_year,
};
pub use weather::{
    GapPolicy, LeapDayPolicy, MissingRadiationPolicy, WeaError, WeatherCsvError, WeatherFilter,
    WeatherPolicy, WeatherPolicyError, WeatherPolicyReport, WeatherSelection, WeeklyOccupancy,
    apply_weather_policy, parse_wea, parse_weather_csv, select_weather, write_wea,
    write_weather_csv,
};

use chrono::{DateTime, Utc};
use core::fmt;
use solar_positioning::{RefractionCorrection, spa};
use xvarna_geometry::Vec3;

/// One weighted UTC interval used by annual or custom studies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeSample {
    /// Unix timestamp at the centre of the interval.
    pub unix_seconds_utc: i64,
    /// Interval duration in hours.
    pub duration_hours: f64,
    /// Dimensionless schedule weight.
    pub weight: f64,
}

impl TimeSample {
    /// Creates a validated time sample.
    pub fn try_new(
        unix_seconds_utc: i64,
        duration_hours: f64,
        weight: f64,
    ) -> Result<Self, TimeError> {
        if DateTime::<Utc>::from_timestamp(unix_seconds_utc, 0).is_none() {
            return Err(TimeError::InvalidTimestamp);
        }
        if !duration_hours.is_finite() || duration_hours <= 0.0 {
            return Err(TimeError::InvalidDuration);
        }
        if !weight.is_finite() || weight < 0.0 {
            return Err(TimeError::InvalidWeight);
        }
        Ok(Self {
            unix_seconds_utc,
            duration_hours,
            weight,
        })
    }
}

/// Geographic observer coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarLocation {
    /// WGS84 latitude in degrees, positive north.
    pub latitude_degrees: f64,
    /// WGS84 longitude in degrees, positive east.
    pub longitude_degrees: f64,
    /// Observer elevation above sea level in metres.
    pub elevation_meters: f64,
}

impl SolarLocation {
    /// Creates a validated location.
    pub fn try_new(
        latitude_degrees: f64,
        longitude_degrees: f64,
        elevation_meters: f64,
    ) -> Result<Self, TimeError> {
        if !latitude_degrees.is_finite() || !(-90.0..=90.0).contains(&latitude_degrees) {
            return Err(TimeError::InvalidLatitude);
        }
        if !longitude_degrees.is_finite() || !(-180.0..=180.0).contains(&longitude_degrees) {
            return Err(TimeError::InvalidLongitude);
        }
        if !elevation_meters.is_finite() || !(-500.0..=20_000.0).contains(&elevation_meters) {
            return Err(TimeError::InvalidElevation);
        }
        Ok(Self {
            latitude_degrees,
            longitude_degrees,
            elevation_meters,
        })
    }
}

/// Reproducible NREL SPA calculation options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SolarOptions {
    /// Terrestrial Time minus Universal Time in seconds.
    pub delta_t_seconds: f64,
    /// Atmospheric pressure in millibars; zero disables refraction correction.
    pub pressure_millibars: f64,
    /// Ambient temperature in degrees Celsius.
    pub temperature_celsius: f64,
    /// Counter-clockwise model rotation of true north about +Z.
    pub north_rotation_degrees: f64,
    /// Minimum apparent altitude included in the active sun set.
    pub minimum_altitude_degrees: f64,
}

impl Default for SolarOptions {
    fn default() -> Self {
        Self {
            delta_t_seconds: 69.0,
            pressure_millibars: 1013.25,
            temperature_celsius: 15.0,
            north_rotation_degrees: 0.0,
            minimum_altitude_degrees: 0.0,
        }
    }
}

impl SolarOptions {
    fn validate(self) -> Result<Self, TimeError> {
        if !self.delta_t_seconds.is_finite()
            || !(-10_000.0..=10_000.0).contains(&self.delta_t_seconds)
        {
            return Err(TimeError::InvalidDeltaT);
        }
        if !self.pressure_millibars.is_finite()
            || !(0.0..=2_000.0).contains(&self.pressure_millibars)
        {
            return Err(TimeError::InvalidPressure);
        }
        if !self.temperature_celsius.is_finite()
            || !(-273.0..=100.0).contains(&self.temperature_celsius)
        {
            return Err(TimeError::InvalidTemperature);
        }
        if !self.north_rotation_degrees.is_finite() {
            return Err(TimeError::InvalidNorthRotation);
        }
        if !self.minimum_altitude_degrees.is_finite()
            || !(-90.0..=90.0).contains(&self.minimum_altitude_degrees)
        {
            return Err(TimeError::InvalidMinimumAltitude);
        }
        Ok(self)
    }
}

/// One calculated solar direction and its temporal provenance.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunSample {
    /// Source UTC timestamp.
    pub unix_seconds_utc: i64,
    /// Model-space unit vector from an observer toward the sun.
    pub direction: Vec3,
    /// Apparent altitude in degrees above the horizon.
    pub altitude_degrees: f64,
    /// Azimuth in degrees clockwise from true north.
    pub azimuth_degrees: f64,
    /// Source interval duration in hours.
    pub duration_hours: f64,
    /// Dimensionless source schedule weight.
    pub weight: f64,
    /// True when altitude satisfies the configured threshold and weight is positive.
    pub is_active: bool,
}

/// Immutable calculated sun set with a deterministic identity.
#[derive(Clone, Debug, PartialEq)]
pub struct SunSet {
    /// Geographic calculation location.
    pub location: SolarLocation,
    /// Calculation options.
    pub options: SolarOptions,
    /// Ordered source-aligned samples.
    pub samples: Vec<SunSample>,
    /// BLAKE3 identity of all inputs and outputs.
    pub content_hash: [u8; 32],
}

/// Generates source-aligned solar positions using the Reda–Andreas NREL SPA.
pub fn calculate_sun_set(
    location: SolarLocation,
    samples: &[TimeSample],
    options: SolarOptions,
) -> Result<SunSet, TimeError> {
    if samples.is_empty() {
        return Err(TimeError::EmptySchedule);
    }
    let options = options.validate()?;
    let refraction = if options.pressure_millibars > 0.0 {
        Some(
            RefractionCorrection::new(options.pressure_millibars, options.temperature_celsius)
                .map_err(|_| TimeError::SolarCalculation)?,
        )
    } else {
        None
    };
    let rotation = options.north_rotation_degrees.to_radians();
    let (rotation_sine, rotation_cosine) = rotation.sin_cos();
    let mut sun_samples = Vec::with_capacity(samples.len());
    for sample in samples {
        let timestamp = DateTime::<Utc>::from_timestamp(sample.unix_seconds_utc, 0)
            .ok_or(TimeError::InvalidTimestamp)?;
        let position = spa::solar_position(
            timestamp,
            location.latitude_degrees,
            location.longitude_degrees,
            location.elevation_meters,
            options.delta_t_seconds,
            refraction,
        )
        .map_err(|_| TimeError::SolarCalculation)?;
        let altitude_degrees = position.elevation_angle();
        let azimuth_degrees = position.azimuth();
        let altitude = altitude_degrees.to_radians();
        let azimuth = azimuth_degrees.to_radians();
        let horizontal = altitude.cos();
        let true_east = azimuth.sin() * horizontal;
        let true_north = azimuth.cos() * horizontal;
        let direction = Vec3::new(
            true_east.mul_add(rotation_cosine, -(true_north * rotation_sine)),
            true_east.mul_add(rotation_sine, true_north * rotation_cosine),
            altitude.sin(),
        )
        .normalized()
        .expect("trigonometric unit vector is finite and non-zero");
        sun_samples.push(SunSample {
            unix_seconds_utc: sample.unix_seconds_utc,
            direction,
            altitude_degrees,
            azimuth_degrees,
            duration_hours: sample.duration_hours,
            weight: sample.weight,
            is_active: altitude_degrees >= options.minimum_altitude_degrees && sample.weight > 0.0,
        });
    }
    let content_hash = sun_set_hash(location, options, &sun_samples);
    Ok(SunSet {
        location,
        options,
        samples: sun_samples,
        content_hash,
    })
}

/// Schedule and solar-position validation errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeError {
    /// UTC timestamp is outside the supported calendar range.
    InvalidTimestamp,
    /// Duration must be finite and positive.
    InvalidDuration,
    /// Weight must be finite and non-negative.
    InvalidWeight,
    /// Latitude is outside -90..=90 degrees.
    InvalidLatitude,
    /// Longitude is outside -180..=180 degrees.
    InvalidLongitude,
    /// Observer elevation is outside the supported range.
    InvalidElevation,
    /// Delta T is non-finite or outside the supported range.
    InvalidDeltaT,
    /// Atmospheric pressure is invalid.
    InvalidPressure,
    /// Ambient temperature is invalid.
    InvalidTemperature,
    /// True-north model rotation is non-finite.
    InvalidNorthRotation,
    /// Minimum altitude is invalid.
    InvalidMinimumAltitude,
    /// A schedule must contain at least one interval.
    EmptySchedule,
    /// The underlying SPA calculation rejected the inputs.
    SolarCalculation,
}

impl fmt::Display for TimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidTimestamp => "UTC timestamp is outside the supported range",
            Self::InvalidDuration => "sample duration must be finite and positive",
            Self::InvalidWeight => "sample weight must be finite and non-negative",
            Self::InvalidLatitude => "latitude must be between -90 and 90 degrees",
            Self::InvalidLongitude => "longitude must be between -180 and 180 degrees",
            Self::InvalidElevation => "observer elevation must be between -500 and 20000 metres",
            Self::InvalidDeltaT => "delta T is outside the supported range",
            Self::InvalidPressure => "pressure must be between 0 and 2000 millibars",
            Self::InvalidTemperature => "temperature must be between -273 and 100 Celsius",
            Self::InvalidNorthRotation => "north rotation must be finite",
            Self::InvalidMinimumAltitude => "minimum altitude must be between -90 and 90 degrees",
            Self::EmptySchedule => "schedule contains no time samples",
            Self::SolarCalculation => "NREL SPA solar-position calculation failed",
        })
    }
}

impl std::error::Error for TimeError {}

fn sun_set_hash(location: SolarLocation, options: SolarOptions, samples: &[SunSample]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_ZURVAN_SUN_SET_V1\0");
    for value in [
        location.latitude_degrees,
        location.longitude_degrees,
        location.elevation_meters,
        options.delta_t_seconds,
        options.pressure_millibars,
        options.temperature_celsius,
        options.north_rotation_degrees,
        options.minimum_altitude_degrees,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&(samples.len() as u64).to_le_bytes());
    for sample in samples {
        hasher.update(&sample.unix_seconds_utc.to_le_bytes());
        for value in [
            sample.direction.x,
            sample.direction.y,
            sample.direction.z,
            sample.altitude_degrees,
            sample.azimuth_degrees,
            sample.duration_hours,
            sample.weight,
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&[u8::from(sample.is_active)]);
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nrel_spa_golden_case_matches_published_angles() {
        let timestamp = "2003-10-17T19:30:30Z"
            .parse::<DateTime<Utc>>()
            .expect("golden timestamp")
            .timestamp();
        let schedule = [TimeSample::try_new(timestamp, 1.0, 1.0).expect("valid sample")];
        let result = calculate_sun_set(
            SolarLocation::try_new(39.742_476, -105.178_6, 1_830.14).expect("valid location"),
            &schedule,
            SolarOptions {
                delta_t_seconds: 67.0,
                pressure_millibars: 820.0,
                temperature_celsius: 11.0,
                ..SolarOptions::default()
            },
        )
        .expect("SPA succeeds");
        let sun = result.samples[0];
        assert!((sun.azimuth_degrees - 194.340_24).abs() < 0.000_3);
        assert!((sun.altitude_degrees - 39.888_38).abs() < 0.000_3);
    }

    #[test]
    fn north_rotation_rotates_xy_without_changing_altitude() {
        let sample = TimeSample::try_new(1_687_348_800, 1.0, 1.0).expect("valid sample");
        let location = SolarLocation::try_new(35.0, 51.0, 1_200.0).expect("valid location");
        let first = calculate_sun_set(location, &[sample], SolarOptions::default())
            .expect("first succeeds");
        let rotated = calculate_sun_set(
            location,
            &[sample],
            SolarOptions {
                north_rotation_degrees: 90.0,
                ..SolarOptions::default()
            },
        )
        .expect("rotation succeeds");
        assert!((rotated.samples[0].direction.x + first.samples[0].direction.y).abs() < 1.0e-12);
        assert!((rotated.samples[0].direction.y - first.samples[0].direction.x).abs() < 1.0e-12);
        assert!((rotated.samples[0].direction.z - first.samples[0].direction.z).abs() < 1.0e-12);
    }

    #[test]
    fn invalid_schedule_values_are_rejected() {
        assert_eq!(
            TimeSample::try_new(0, 0.0, 1.0),
            Err(TimeError::InvalidDuration)
        );
        assert_eq!(
            TimeSample::try_new(0, 1.0, -1.0),
            Err(TimeError::InvalidWeight)
        );
    }
}
