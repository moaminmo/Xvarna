//! WEA/CSV interoperability, explicit repair policy, and occupancy-aware filtering.

// These conversions are confined to validated civil-time ranges and integral
// file-format resolutions. Keeping the checks beside each conversion would hide
// the weather policy; the parsers reject invalid ranges before constructing data.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use super::{
    EpwDataPeriod, EpwLocation, EpwMissingCounts, EpwRecord, EpwWeather, SolarLocation,
    SolarOptions, TimeSample, calculate_sun_set, epw::weather_hash,
};
use chrono::{Datelike, NaiveDate, TimeDelta, Timelike, Utc};
use core::fmt;

/// Missing radiation handling selected explicitly by a caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MissingRadiationPolicy {
    /// Fail if any requested radiation channel is missing.
    Reject,
    /// Retain the parser's transparent zero and missing flag.
    Zero,
    /// Fill from the nearest bracketing valid samples in timestamp space.
    LinearInterpolation,
}

/// Leap-day handling for multi-year comparability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeapDayPolicy {
    /// Preserve February 29 intervals.
    Keep,
    /// Remove February 29 intervals and report the exact count.
    Drop,
}

/// Timeline gap handling after monotonicity validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GapPolicy {
    /// Reject any interval spacing larger than the declared resolution.
    Reject,
    /// Preserve gaps and expose their count in provenance.
    Preserve,
}

/// Complete deterministic weather normalization policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeatherPolicy {
    /// Missing global/direct/diffuse radiation policy.
    pub missing_radiation: MissingRadiationPolicy,
    /// Leap-day policy.
    pub leap_day: LeapDayPolicy,
    /// Missing-interval policy.
    pub gaps: GapPolicy,
}

impl Default for WeatherPolicy {
    fn default() -> Self {
        Self {
            missing_radiation: MissingRadiationPolicy::Reject,
            leap_day: LeapDayPolicy::Keep,
            gaps: GapPolicy::Reject,
        }
    }
}

/// Normalized weather plus complete policy diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherPolicyReport {
    /// Policy-normalized weather ready for analysis.
    pub weather: EpwWeather,
    /// February 29 records removed.
    pub dropped_leap_day_count: usize,
    /// Gaps preserved or observed before rejection.
    pub gap_count: usize,
    /// Global-horizontal samples interpolated.
    pub interpolated_global_count: usize,
    /// Direct-normal samples interpolated.
    pub interpolated_direct_count: usize,
    /// Diffuse-horizontal samples interpolated.
    pub interpolated_diffuse_count: usize,
}

/// Weather repair cannot satisfy the selected scientific policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WeatherPolicyError {
    /// Records are duplicated or not strictly increasing in UTC.
    NonMonotonicTimeline,
    /// One or more intervals are absent under a reject policy.
    TimelineGap,
    /// Missing radiation is present under a reject policy.
    MissingRadiation,
    /// No valid value exists from which to interpolate a channel.
    CannotInterpolate,
    /// A policy removed every interval.
    EmptySelection,
}

impl fmt::Display for WeatherPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonMonotonicTimeline => "weather timeline must be strictly increasing in UTC",
            Self::TimelineGap => "weather timeline contains a missing interval",
            Self::MissingRadiation => "weather contains missing radiation under reject policy",
            Self::CannotInterpolate => "weather channel has no valid sample for interpolation",
            Self::EmptySelection => "weather policy or filter selected no intervals",
        })
    }
}

impl std::error::Error for WeatherPolicyError {}

/// Applies leap, gap, and missing-data rules without hiding provenance.
pub fn apply_weather_policy(
    weather: &EpwWeather,
    policy: WeatherPolicy,
) -> Result<WeatherPolicyReport, WeatherPolicyError> {
    // Validate missing source intervals before deliberate calendar filtering.
    let gap_count = validate_timeline(&weather.records)?;
    let mut records = weather.records.clone();
    let before_leap = records.len();
    if policy.leap_day == LeapDayPolicy::Drop {
        records.retain(|record| record.month != 2 || record.day != 29);
    }
    let dropped_leap_day_count = before_leap - records.len();
    if records.is_empty() {
        return Err(WeatherPolicyError::EmptySelection);
    }

    if gap_count > 0 && policy.gaps == GapPolicy::Reject {
        return Err(WeatherPolicyError::TimelineGap);
    }

    let missing = count_missing(&records);
    let mut interpolated = EpwMissingCounts::default();
    match policy.missing_radiation {
        MissingRadiationPolicy::Reject
            if missing.global_horizontal > 0
                || missing.direct_normal > 0
                || missing.diffuse_horizontal > 0 =>
        {
            return Err(WeatherPolicyError::MissingRadiation);
        }
        MissingRadiationPolicy::LinearInterpolation => {
            interpolated.global_horizontal = interpolate_channel(
                &mut records,
                |record| {
                    (
                        record.global_horizontal_wh_m2,
                        record.global_horizontal_missing,
                    )
                },
                |record, value| {
                    record.global_horizontal_wh_m2 = value;
                    record.global_horizontal_missing = false;
                },
            )?;
            interpolated.direct_normal = interpolate_channel(
                &mut records,
                |record| (record.direct_normal_wh_m2, record.direct_normal_missing),
                |record, value| {
                    record.direct_normal_wh_m2 = value;
                    record.direct_normal_missing = false;
                },
            )?;
            interpolated.diffuse_horizontal = interpolate_channel(
                &mut records,
                |record| {
                    (
                        record.diffuse_horizontal_wh_m2,
                        record.diffuse_horizontal_missing,
                    )
                },
                |record, value| {
                    record.diffuse_horizontal_wh_m2 = value;
                    record.diffuse_horizontal_missing = false;
                },
            )?;
        }
        MissingRadiationPolicy::Reject | MissingRadiationPolicy::Zero => {}
    }
    let missing_counts = count_missing(&records);
    let content_hash = weather_hash(
        &weather.location,
        &weather.data_period,
        &records,
        missing_counts,
    );
    Ok(WeatherPolicyReport {
        weather: EpwWeather {
            location: weather.location.clone(),
            data_period: weather.data_period.clone(),
            records,
            missing_counts,
            content_hash,
        },
        dropped_leap_day_count,
        gap_count,
        interpolated_global_count: interpolated.global_horizontal,
        interpolated_direct_count: interpolated.direct_normal,
        interpolated_diffuse_count: interpolated.diffuse_horizontal,
    })
}

fn count_missing(records: &[EpwRecord]) -> EpwMissingCounts {
    records
        .iter()
        .fold(EpwMissingCounts::default(), |mut count, record| {
            count.global_horizontal += usize::from(record.global_horizontal_missing);
            count.direct_normal += usize::from(record.direct_normal_missing);
            count.diffuse_horizontal += usize::from(record.diffuse_horizontal_missing);
            count
        })
}

fn validate_timeline(records: &[EpwRecord]) -> Result<usize, WeatherPolicyError> {
    let mut gaps = 0_usize;
    for pair in records.windows(2) {
        let delta = pair[1].midpoint_unix_seconds_utc - pair[0].midpoint_unix_seconds_utc;
        if delta <= 0 {
            return Err(WeatherPolicyError::NonMonotonicTimeline);
        }
        let expected = (pair[0].duration_hours * 3_600.0).round() as i64;
        if delta > expected + 1 {
            gaps += 1;
        }
    }
    Ok(gaps)
}

fn interpolate_channel(
    records: &mut [EpwRecord],
    read: impl Fn(&EpwRecord) -> (f64, bool),
    write: impl Fn(&mut EpwRecord, f64),
) -> Result<usize, WeatherPolicyError> {
    let valid = records
        .iter()
        .enumerate()
        .filter_map(|(index, record)| (!read(record).1).then_some(index))
        .collect::<Vec<_>>();
    if valid.is_empty() {
        return Err(WeatherPolicyError::CannotInterpolate);
    }
    let mut count = 0_usize;
    for index in 0..records.len() {
        if !read(&records[index]).1 {
            continue;
        }
        let insertion = valid.partition_point(|&candidate| candidate < index);
        let previous = insertion.checked_sub(1).map(|i| valid[i]);
        let next = valid.get(insertion).copied();
        let value = match (previous, next) {
            (Some(previous), Some(next)) => {
                let first_time = records[previous].midpoint_unix_seconds_utc as f64;
                let last_time = records[next].midpoint_unix_seconds_utc as f64;
                let target = records[index].midpoint_unix_seconds_utc as f64;
                let fraction = (target - first_time) / (last_time - first_time);
                let first = read(&records[previous]).0;
                let last = read(&records[next]).0;
                (last - first).mul_add(fraction, first)
            }
            (Some(previous), None) => read(&records[previous]).0,
            (None, Some(next)) => read(&records[next]).0,
            (None, None) => return Err(WeatherPolicyError::CannotInterpolate),
        };
        write(&mut records[index], value);
        count += 1;
    }
    Ok(count)
}

/// Reusable month/weekday/hour/radiation selection contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeatherFilter {
    /// Twelve low bits select January through December; zero means all months.
    pub month_mask: u16,
    /// Seven low bits select Monday through Sunday; zero means all weekdays.
    pub weekday_mask: u8,
    /// Inclusive local-standard midpoint hour from 0 through 23.
    pub start_hour: u8,
    /// Exclusive local-standard midpoint hour from 1 through 24.
    pub end_hour: u8,
    /// Minimum direct-normal interval energy in Wh/m².
    pub minimum_direct_normal_wh_m2: f64,
    /// Minimum global-horizontal interval energy in Wh/m².
    pub minimum_global_horizontal_wh_m2: f64,
}

impl Default for WeatherFilter {
    fn default() -> Self {
        Self {
            month_mask: 0,
            weekday_mask: 0,
            start_hour: 0,
            end_hour: 24,
            minimum_direct_normal_wh_m2: 0.0,
            minimum_global_horizontal_wh_m2: 0.0,
        }
    }
}

/// Weekly local-standard occupancy weights plus an explicit holiday override.
#[derive(Clone, Debug, PartialEq)]
pub struct WeeklyOccupancy {
    /// Monday 00:00 through Sunday 23:00, one weight per local hour.
    pub hourly_weights: [f64; 168],
    /// Month/day pairs forced to zero occupancy.
    pub holidays: Vec<(u8, u8)>,
}

impl WeeklyOccupancy {
    /// Fully occupied schedule.
    #[must_use]
    pub const fn always() -> Self {
        Self {
            hourly_weights: [1.0; 168],
            holidays: Vec::new(),
        }
    }

    /// Monday–Friday occupancy for local midpoint hours in `[start, end)`.
    #[must_use]
    pub fn weekday_hours(start: u8, end: u8) -> Self {
        let mut weights = [0.0; 168];
        for weekday in 0..5_usize {
            for hour in start.min(24)..end.min(24) {
                weights[weekday * 24 + usize::from(hour)] = 1.0;
            }
        }
        Self {
            hourly_weights: weights,
            holidays: Vec::new(),
        }
    }
}

/// Source-aligned filtered schedule with deterministic record indices.
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherSelection {
    /// Indices into the source weather record array.
    pub record_indices: Vec<usize>,
    /// Weighted solar/time samples aligned one-to-one with the indices.
    pub samples: Vec<TimeSample>,
    /// Sum of duration multiplied by occupancy weight.
    pub weighted_hours: f64,
    /// BLAKE3 identity of source, filter, occupancy, and selected indices.
    pub content_hash: [u8; 32],
}

/// Applies reusable weather and occupancy masks without copying weather records.
pub fn select_weather(
    weather: &EpwWeather,
    filter: WeatherFilter,
    occupancy: &WeeklyOccupancy,
) -> Result<WeatherSelection, WeatherPolicyError> {
    if filter.start_hour > 23
        || filter.end_hour == 0
        || filter.end_hour > 24
        || filter.start_hour >= filter.end_hour
        || !filter.minimum_direct_normal_wh_m2.is_finite()
        || !filter.minimum_global_horizontal_wh_m2.is_finite()
        || occupancy
            .hourly_weights
            .iter()
            .any(|weight| !weight.is_finite() || *weight < 0.0)
    {
        return Err(WeatherPolicyError::EmptySelection);
    }
    let mut record_indices = Vec::new();
    let mut samples = Vec::new();
    let mut weighted_hours = 0.0;
    let offset_seconds = (weather.location.time_zone_hours * 3_600.0).round() as i64;
    for (index, record) in weather.records.iter().enumerate() {
        let Some(local) = chrono::DateTime::<Utc>::from_timestamp(
            record.midpoint_unix_seconds_utc + offset_seconds,
            0,
        ) else {
            continue;
        };
        let month_bit = 1_u16 << (record.month - 1);
        let weekday =
            usize::try_from(local.weekday().num_days_from_monday()).expect("weekday fits");
        let weekday_bit = 1_u8 << weekday;
        let hour = usize::try_from(local.hour()).expect("hour fits");
        let holiday = occupancy.holidays.contains(&(record.month, record.day));
        let weight = if holiday {
            0.0
        } else {
            occupancy.hourly_weights[weekday * 24 + hour]
        };
        let included = (filter.month_mask == 0 || filter.month_mask & month_bit != 0)
            && (filter.weekday_mask == 0 || filter.weekday_mask & weekday_bit != 0)
            && hour >= usize::from(filter.start_hour)
            && hour < usize::from(filter.end_hour)
            && record.direct_normal_wh_m2 >= filter.minimum_direct_normal_wh_m2
            && record.global_horizontal_wh_m2 >= filter.minimum_global_horizontal_wh_m2
            && weight > 0.0;
        if included {
            record_indices.push(index);
            samples.push(TimeSample {
                unix_seconds_utc: record.midpoint_unix_seconds_utc,
                duration_hours: record.duration_hours,
                weight,
            });
            weighted_hours = (record.duration_hours * weight).mul_add(1.0, weighted_hours);
        }
    }
    if samples.is_empty() {
        return Err(WeatherPolicyError::EmptySelection);
    }
    let content_hash = selection_hash(weather, filter, occupancy, &record_indices, &samples);
    Ok(WeatherSelection {
        record_indices,
        samples,
        weighted_hours,
        content_hash,
    })
}

fn selection_hash(
    weather: &EpwWeather,
    filter: WeatherFilter,
    occupancy: &WeeklyOccupancy,
    indices: &[usize],
    samples: &[TimeSample],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_WEATHER_SELECTION_V1\0");
    hasher.update(&weather.content_hash);
    hasher.update(&filter.month_mask.to_le_bytes());
    hasher.update(&[filter.weekday_mask, filter.start_hour, filter.end_hour]);
    hasher.update(&filter.minimum_direct_normal_wh_m2.to_bits().to_le_bytes());
    hasher.update(
        &filter
            .minimum_global_horizontal_wh_m2
            .to_bits()
            .to_le_bytes(),
    );
    for weight in occupancy.hourly_weights {
        hasher.update(&weight.to_bits().to_le_bytes());
    }
    for holiday in &occupancy.holidays {
        hasher.update(&[holiday.0, holiday.1]);
    }
    for (&index, sample) in indices.iter().zip(samples) {
        hasher.update(&(index as u64).to_le_bytes());
        hasher.update(&sample.weight.to_bits().to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

/// Invalid WEA header, timeline, or irradiance row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WeaError {
    /// Mandatory header field is absent or malformed.
    InvalidHeader(&'static str),
    /// A weather row is malformed.
    InvalidRecord(usize),
    /// No weather rows exist.
    Empty,
    /// Temporal resolution is not a divisor of one hour.
    UnsupportedResolution,
    /// Solar reconstruction failed.
    Solar,
}

impl fmt::Display for WeaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader(name) => write!(formatter, "invalid WEA header {name}"),
            Self::InvalidRecord(line) => write!(formatter, "invalid WEA record on line {line}"),
            Self::Empty => formatter.write_str("WEA contains no weather records"),
            Self::UnsupportedResolution => {
                formatter.write_str("WEA resolution must divide one hour")
            }
            Self::Solar => formatter.write_str("WEA global-horizontal reconstruction failed"),
        }
    }
}

impl std::error::Error for WeaError {}

#[derive(Clone, Copy, Debug)]
struct WeaRow {
    month: u8,
    day: u8,
    hour: f64,
    direct: f64,
    diffuse: f64,
}

/// Parses Radiance WEA and reconstructs GHI using the same NREL-SPA direction engine.
#[allow(clippy::too_many_lines)]
pub fn parse_wea(text: &str) -> Result<EpwWeather, WeaError> {
    let mut place = None;
    let mut latitude = None;
    let mut longitude_west = None;
    let mut time_zone_west = None;
    let mut elevation = None;
    let mut rows = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields = line.split_whitespace().collect::<Vec<_>>();
        match fields.as_slice() {
            ["place", tail @ ..] => place = Some(tail.join(" ")),
            ["latitude", value] => latitude = value.parse::<f64>().ok(),
            ["longitude", value] => longitude_west = value.parse::<f64>().ok(),
            ["time_zone", value] => time_zone_west = value.parse::<f64>().ok(),
            ["site_elevation", value] => elevation = value.parse::<f64>().ok(),
            ["weather_data_file_units", _] => {}
            [month, day, hour, direct, diffuse] => {
                let row = WeaRow {
                    month: month
                        .parse()
                        .map_err(|_| WeaError::InvalidRecord(index + 1))?,
                    day: day
                        .parse()
                        .map_err(|_| WeaError::InvalidRecord(index + 1))?,
                    hour: hour
                        .parse()
                        .map_err(|_| WeaError::InvalidRecord(index + 1))?,
                    direct: direct
                        .parse()
                        .map_err(|_| WeaError::InvalidRecord(index + 1))?,
                    diffuse: diffuse
                        .parse()
                        .map_err(|_| WeaError::InvalidRecord(index + 1))?,
                };
                if !(1..=12).contains(&row.month)
                    || !(0.0..=24.0).contains(&row.hour)
                    || !row.direct.is_finite()
                    || row.direct < 0.0
                    || !row.diffuse.is_finite()
                    || row.diffuse < 0.0
                {
                    return Err(WeaError::InvalidRecord(index + 1));
                }
                rows.push(row);
            }
            _ => return Err(WeaError::InvalidRecord(index + 1)),
        }
    }
    if rows.is_empty() {
        return Err(WeaError::Empty);
    }
    let place = place.ok_or(WeaError::InvalidHeader("place"))?;
    let latitude = latitude.ok_or(WeaError::InvalidHeader("latitude"))?;
    let longitude = -longitude_west.ok_or(WeaError::InvalidHeader("longitude"))?;
    let time_zone_hours = -time_zone_west.ok_or(WeaError::InvalidHeader("time_zone"))? / 15.0;
    let elevation = elevation.ok_or(WeaError::InvalidHeader("site_elevation"))?;
    let solar = SolarLocation::try_new(latitude, longitude, elevation)
        .map_err(|_| WeaError::InvalidHeader("location"))?;
    let year = if rows.iter().any(|row| row.month == 2 && row.day == 29) {
        2020
    } else {
        2021
    };
    let duration_hours = infer_wea_duration(&rows, year)?;
    let records_per_hour = (1.0 / duration_hours).round() as u8;
    if records_per_hour == 0 || !60_u8.is_multiple_of(records_per_hour) {
        return Err(WeaError::UnsupportedResolution);
    }
    let offset_seconds = (time_zone_hours * 3_600.0).round() as i64;
    let mut records = Vec::with_capacity(rows.len());
    let mut samples = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        let date = NaiveDate::from_ymd_opt(year, u32::from(row.month), u32::from(row.day))
            .ok_or(WeaError::InvalidRecord(index + 1))?;
        let local_midpoint = date.and_hms_opt(0, 0, 0).expect("midnight exists")
            + TimeDelta::milliseconds((row.hour * 3_600_000.0).round() as i64);
        let midpoint = local_midpoint.and_utc().timestamp() - offset_seconds;
        samples.push(TimeSample {
            unix_seconds_utc: midpoint,
            duration_hours,
            weight: 1.0,
        });
        let local_end =
            local_midpoint + TimeDelta::milliseconds((duration_hours * 1_800_000.0).round() as i64);
        let seconds =
            (local_end - date.and_hms_opt(0, 0, 0).expect("midnight exists")).num_seconds();
        let (hour, minute) = if seconds >= 86_400 {
            (24_u8, 60_u8)
        } else {
            let end_minutes = seconds / 60;
            let remainder = end_minutes % 60;
            if remainder == 0 {
                (u8::try_from(end_minutes / 60).unwrap_or(24), 60)
            } else {
                (
                    u8::try_from(end_minutes / 60 + 1).unwrap_or(24),
                    u8::try_from(remainder).unwrap_or(60),
                )
            }
        };
        records.push(EpwRecord {
            year,
            month: row.month,
            day: row.day,
            hour,
            minute,
            midpoint_unix_seconds_utc: midpoint,
            duration_hours,
            data_source_and_uncertainty_flags: "WEA".to_owned(),
            dry_bulb_celsius: None,
            atmospheric_pressure_pascals: None,
            global_horizontal_wh_m2: 0.0,
            direct_normal_wh_m2: row.direct * duration_hours,
            diffuse_horizontal_wh_m2: row.diffuse * duration_hours,
            albedo: None,
            global_horizontal_missing: false,
            direct_normal_missing: false,
            diffuse_horizontal_missing: false,
        });
    }
    let sun = calculate_sun_set(
        solar,
        &samples,
        SolarOptions {
            pressure_millibars: 0.0,
            ..SolarOptions::default()
        },
    )
    .map_err(|_| WeaError::Solar)?;
    for (record, sample) in records.iter_mut().zip(sun.samples) {
        record.global_horizontal_wh_m2 = record
            .direct_normal_wh_m2
            .mul_add(sample.direction.z.max(0.0), record.diffuse_horizontal_wh_m2);
    }
    let location = EpwLocation {
        city: place,
        state_or_province: String::new(),
        country: String::new(),
        data_source: "Radiance WEA".to_owned(),
        station_identifier: String::new(),
        solar,
        time_zone_hours,
    };
    let data_period = EpwDataPeriod {
        records_per_hour,
        name: "WEA".to_owned(),
        start_day_of_week: "Unknown".to_owned(),
        start_date: format!("{}/{}", rows[0].month, rows[0].day),
        end_date: format!(
            "{}/{}",
            rows.last().expect("not empty").month,
            rows.last().expect("not empty").day
        ),
    };
    let missing_counts = EpwMissingCounts::default();
    let content_hash = weather_hash(&location, &data_period, &records, missing_counts);
    Ok(EpwWeather {
        location,
        data_period,
        records,
        missing_counts,
        content_hash,
    })
}

fn infer_wea_duration(rows: &[WeaRow], year: i32) -> Result<f64, WeaError> {
    if rows.len() == 1 {
        return Ok(1.0);
    }
    let mut smallest = f64::INFINITY;
    for pair in rows.windows(2) {
        let first = NaiveDate::from_ymd_opt(year, u32::from(pair[0].month), u32::from(pair[0].day))
            .ok_or(WeaError::UnsupportedResolution)?;
        let second =
            NaiveDate::from_ymd_opt(year, u32::from(pair[1].month), u32::from(pair[1].day))
                .ok_or(WeaError::UnsupportedResolution)?;
        let days = (second - first).num_days() as f64;
        let difference = days.mul_add(24.0, pair[1].hour - pair[0].hour);
        if difference > 0.0 {
            smallest = smallest.min(difference);
        }
    }
    let records_per_hour = (1.0 / smallest).round();
    if !smallest.is_finite()
        || records_per_hour < 1.0
        || (records_per_hour * smallest - 1.0).abs() > 1.0e-6
        || records_per_hour > 60.0
    {
        return Err(WeaError::UnsupportedResolution);
    }
    Ok(1.0 / records_per_hour)
}

/// Writes Radiance WEA with midpoint local-standard timestamps and irradiance units.
#[must_use]
pub fn write_wea(weather: &EpwWeather) -> String {
    use std::fmt::Write as _;
    let mut output = String::new();
    writeln!(output, "place {}", weather.location.city).expect("String write succeeds");
    writeln!(
        output,
        "latitude {:.8}",
        weather.location.solar.latitude_degrees
    )
    .expect("String write succeeds");
    writeln!(
        output,
        "longitude {:.8}",
        -weather.location.solar.longitude_degrees
    )
    .expect("String write succeeds");
    writeln!(
        output,
        "time_zone {:.8}",
        -15.0 * weather.location.time_zone_hours
    )
    .expect("String write succeeds");
    writeln!(
        output,
        "site_elevation {:.8}",
        weather.location.solar.elevation_meters
    )
    .expect("String write succeeds");
    output.push_str("weather_data_file_units 1\n");
    for record in &weather.records {
        let end_hour = f64::from(record.hour - 1) + f64::from(record.minute) / 60.0;
        let midpoint = end_hour - record.duration_hours / 2.0;
        writeln!(
            output,
            "{} {} {:.6} {:.6} {:.6}",
            record.month,
            record.day,
            midpoint,
            record.direct_normal_wh_m2 / record.duration_hours,
            record.diffuse_horizontal_wh_m2 / record.duration_hours
        )
        .expect("String write succeeds");
    }
    output
}

/// Invalid canonical XVARNA weather CSV.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WeatherCsvError {
    /// Magic or column contract does not match version one.
    InvalidHeader,
    /// Site coordinates or time zone are missing or invalid.
    InvalidLocation,
    /// One-based source line contains an invalid field.
    InvalidRecord(usize),
    /// No data rows followed the header.
    Empty,
}

impl fmt::Display for WeatherCsvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHeader => formatter.write_str("invalid XVARNA weather CSV header"),
            Self::InvalidLocation => formatter.write_str("invalid XVARNA weather CSV location"),
            Self::InvalidRecord(line) => write!(
                formatter,
                "invalid XVARNA weather CSV record on line {line}"
            ),
            Self::Empty => formatter.write_str("XVARNA weather CSV has no records"),
        }
    }
}
impl std::error::Error for WeatherCsvError {}

/// Writes the versioned lossless weather interchange CSV.
#[must_use]
pub fn write_weather_csv(weather: &EpwWeather) -> String {
    use std::fmt::Write as _;
    let city = weather.location.city.replace(',', ";");
    let mut output = String::from("# XVARNA_WEATHER_CSV_V1\n");
    writeln!(
        output,
        "# location,{city},{:.17},{:.17},{:.17},{:.17}",
        weather.location.solar.latitude_degrees,
        weather.location.solar.longitude_degrees,
        weather.location.solar.elevation_meters,
        weather.location.time_zone_hours
    )
    .expect("String write succeeds");
    output.push_str("year,month,day,hour,minute,midpoint_utc,duration_hours,ghi_wh_m2,dni_wh_m2,dhi_wh_m2,dry_bulb_c,pressure_pa,albedo,ghi_missing,dni_missing,dhi_missing,source_flags\n");
    for record in &weather.records {
        let flags = record.data_source_and_uncertainty_flags.replace(',', ";");
        writeln!(
            output,
            "{},{},{},{},{},{},{:.17},{:.17},{:.17},{:.17},{},{},{},{},{},{},{}",
            record.year,
            record.month,
            record.day,
            record.hour,
            record.minute,
            record.midpoint_unix_seconds_utc,
            record.duration_hours,
            record.global_horizontal_wh_m2,
            record.direct_normal_wh_m2,
            record.diffuse_horizontal_wh_m2,
            optional(record.dry_bulb_celsius),
            optional(record.atmospheric_pressure_pascals),
            optional(record.albedo),
            u8::from(record.global_horizontal_missing),
            u8::from(record.direct_normal_missing),
            u8::from(record.diffuse_horizontal_missing),
            flags
        )
        .expect("String write succeeds");
    }
    output
}

fn optional(value: Option<f64>) -> String {
    value.map_or_else(String::new, |value| format!("{value:.17}"))
}

/// Parses the versioned lossless weather interchange CSV.
pub fn parse_weather_csv(text: &str) -> Result<EpwWeather, WeatherCsvError> {
    let mut lines = text.lines();
    if lines.next().map(str::trim) != Some("# XVARNA_WEATHER_CSV_V1") {
        return Err(WeatherCsvError::InvalidHeader);
    }
    let location_fields = lines
        .next()
        .ok_or(WeatherCsvError::InvalidLocation)?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if location_fields.len() != 6 || location_fields[0] != "# location" {
        return Err(WeatherCsvError::InvalidLocation);
    }
    let parse_location = |index: usize| {
        location_fields[index]
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or(WeatherCsvError::InvalidLocation)
    };
    let solar = SolarLocation::try_new(parse_location(2)?, parse_location(3)?, parse_location(4)?)
        .map_err(|_| WeatherCsvError::InvalidLocation)?;
    let time_zone_hours = parse_location(5)?;
    let expected_header = "year,month,day,hour,minute,midpoint_utc,duration_hours,ghi_wh_m2,dni_wh_m2,dhi_wh_m2,dry_bulb_c,pressure_pa,albedo,ghi_missing,dni_missing,dhi_missing,source_flags";
    if lines.next().map(str::trim) != Some(expected_header) {
        return Err(WeatherCsvError::InvalidHeader);
    }
    let mut records = Vec::new();
    for (offset, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let fields = line.split(',').map(str::trim).collect::<Vec<_>>();
        if fields.len() != 17 {
            return Err(WeatherCsvError::InvalidRecord(offset + 4));
        }
        let record = EpwRecord {
            year: parse_csv(fields[0], offset)?,
            month: parse_csv(fields[1], offset)?,
            day: parse_csv(fields[2], offset)?,
            hour: parse_csv(fields[3], offset)?,
            minute: parse_csv(fields[4], offset)?,
            midpoint_unix_seconds_utc: parse_csv(fields[5], offset)?,
            duration_hours: parse_csv(fields[6], offset)?,
            global_horizontal_wh_m2: parse_csv(fields[7], offset)?,
            direct_normal_wh_m2: parse_csv(fields[8], offset)?,
            diffuse_horizontal_wh_m2: parse_csv(fields[9], offset)?,
            dry_bulb_celsius: parse_optional(fields[10], offset)?,
            atmospheric_pressure_pascals: parse_optional(fields[11], offset)?,
            albedo: parse_optional(fields[12], offset)?,
            global_horizontal_missing: parse_bool(fields[13], offset)?,
            direct_normal_missing: parse_bool(fields[14], offset)?,
            diffuse_horizontal_missing: parse_bool(fields[15], offset)?,
            data_source_and_uncertainty_flags: fields[16].to_owned(),
        };
        if !record.duration_hours.is_finite() || record.duration_hours <= 0.0 {
            return Err(WeatherCsvError::InvalidRecord(offset + 4));
        }
        records.push(record);
    }
    if records.is_empty() {
        return Err(WeatherCsvError::Empty);
    }
    let records_per_hour = (1.0 / records[0].duration_hours).round() as u8;
    let location = EpwLocation {
        city: location_fields[1].to_owned(),
        state_or_province: String::new(),
        country: String::new(),
        data_source: "XVARNA CSV".to_owned(),
        station_identifier: String::new(),
        solar,
        time_zone_hours,
    };
    let data_period = EpwDataPeriod {
        records_per_hour,
        name: "XVARNA CSV".to_owned(),
        start_day_of_week: "Unknown".to_owned(),
        start_date: format!("{}/{}", records[0].month, records[0].day),
        end_date: format!(
            "{}/{}",
            records.last().expect("not empty").month,
            records.last().expect("not empty").day
        ),
    };
    let missing_counts = count_missing(&records);
    let content_hash = weather_hash(&location, &data_period, &records, missing_counts);
    Ok(EpwWeather {
        location,
        data_period,
        records,
        missing_counts,
        content_hash,
    })
}

fn parse_csv<T: core::str::FromStr>(value: &str, offset: usize) -> Result<T, WeatherCsvError> {
    value
        .parse()
        .map_err(|_| WeatherCsvError::InvalidRecord(offset + 4))
}
fn parse_optional(value: &str, offset: usize) -> Result<Option<f64>, WeatherCsvError> {
    if value.is_empty() {
        Ok(None)
    } else {
        parse_csv(value, offset).map(Some)
    }
}
fn parse_bool(value: &str, offset: usize) -> Result<bool, WeatherCsvError> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(WeatherCsvError::InvalidRecord(offset + 4)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WEA: &str = "place Test Site\nlatitude 35.0\nlongitude -51.0\ntime_zone -52.5\nsite_elevation 1200\nweather_data_file_units 1\n6 21 11.5 600 100\n6 21 12.5 700 110\n";

    #[test]
    fn wea_and_csv_round_trip_into_analysis_weather() {
        let weather = parse_wea(WEA).expect("WEA parses");
        assert_eq!(weather.records.len(), 2);
        assert!(weather.records[0].global_horizontal_wh_m2 >= 100.0);
        let csv = write_weather_csv(&weather);
        let restored = parse_weather_csv(&csv).expect("CSV parses");
        assert_eq!(restored.records, weather.records);
        assert_eq!(write_wea(&restored).lines().count(), 8);
    }

    #[test]
    fn interpolation_and_occupancy_are_explicit() {
        let mut weather = parse_wea(WEA).expect("WEA parses");
        weather.records[0].direct_normal_missing = true;
        weather.records[0].direct_normal_wh_m2 = 0.0;
        let report = apply_weather_policy(
            &weather,
            WeatherPolicy {
                missing_radiation: MissingRadiationPolicy::LinearInterpolation,
                gaps: GapPolicy::Preserve,
                ..WeatherPolicy::default()
            },
        )
        .expect("interpolation succeeds");
        assert_eq!(report.interpolated_direct_count, 1);
        assert!((report.weather.records[0].direct_normal_wh_m2 - 700.0).abs() < f64::EPSILON);
        let selection = select_weather(
            &report.weather,
            WeatherFilter::default(),
            &WeeklyOccupancy::always(),
        )
        .expect("selection succeeds");
        assert_eq!(selection.samples.len(), 2);
        assert!((selection.weighted_hours - 2.0).abs() < f64::EPSILON);
    }
    #[test]
    fn intentional_leap_day_removal_is_not_a_source_gap() {
        let mut weather = parse_wea(WEA).unwrap();
        let template = weather.records[0].clone();
        let start = chrono::NaiveDate::from_ymd_opt(2024, 2, 28)
            .unwrap()
            .and_hms_opt(23, 30, 0)
            .unwrap()
            .and_utc()
            .timestamp();
        weather.records = (0..26)
            .map(|i| {
                let mut record = template.clone();
                record.midpoint_unix_seconds_utc = start + i * 3600;
                record.year = 2024;
                record.month = if i == 25 { 3 } else { 2 };
                record.day = if i == 0 {
                    28
                } else if i == 25 {
                    1
                } else {
                    29
                };
                record
            })
            .collect();
        let report = apply_weather_policy(
            &weather,
            WeatherPolicy {
                leap_day: LeapDayPolicy::Drop,
                gaps: GapPolicy::Reject,
                ..WeatherPolicy::default()
            },
        )
        .unwrap();
        assert_eq!(report.dropped_leap_day_count, 24);
        assert_eq!(report.gap_count, 0);
        assert_eq!(report.weather.records.len(), 2);
        weather.records.remove(12);
        assert_eq!(
            apply_weather_policy(
                &weather,
                WeatherPolicy {
                    leap_day: LeapDayPolicy::Drop,
                    gaps: GapPolicy::Reject,
                    ..WeatherPolicy::default()
                }
            )
            .unwrap_err(),
            WeatherPolicyError::TimelineGap
        );
    }

    #[test]
    fn interpolation_uses_time_and_preserves_valid_samples() {
        let template = parse_wea(WEA).unwrap().records[0].clone();
        let mut records: Vec<_> = (0..5)
            .map(|i| {
                let mut r = template.clone();
                r.midpoint_unix_seconds_utc += i * 3600;
                r.direct_normal_missing = i > 0 && i < 4;
                r.direct_normal_wh_m2 = (i as f64).mul_add(50.0, 100.0);
                r
            })
            .collect();
        for r in &mut records[1..4] {
            r.direct_normal_wh_m2 = 0.0;
        }
        let count = interpolate_channel(
            &mut records,
            |r| (r.direct_normal_wh_m2, r.direct_normal_missing),
            |r, v| r.direct_normal_wh_m2 = v,
        )
        .unwrap();
        assert_eq!(count, 3);
        for (i, r) in records.iter().enumerate() {
            assert!((r.direct_normal_wh_m2 - ((i as f64).mul_add(50.0, 100.0))).abs() < 1e-10);
        }
    }
}
