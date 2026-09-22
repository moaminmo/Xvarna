//! Strict, dependency-light `EnergyPlus` Weather (EPW) ingestion.

use super::{SolarLocation, TimeSample};
use chrono::{NaiveDate, TimeDelta};
use core::fmt;

const HEADER_LINE_COUNT: usize = 8;
const MINIMUM_DATA_FIELDS: usize = 33;

/// Geographic and provenance metadata from the EPW `LOCATION` header.
#[derive(Clone, Debug, PartialEq)]
pub struct EpwLocation {
    /// Human-readable city or site name.
    pub city: String,
    /// State, province, or region.
    pub state_or_province: String,
    /// Country name or code.
    pub country: String,
    /// Weather-data source description.
    pub data_source: String,
    /// WMO station identifier.
    pub station_identifier: String,
    /// Validated observer coordinates used by the solar engine.
    pub solar: SolarLocation,
    /// Local standard-time offset from UTC, in hours.
    pub time_zone_hours: f64,
}

/// Coverage declared by the first EPW `DATA PERIODS` header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpwDataPeriod {
    /// Number of records in every hour.
    pub records_per_hour: u8,
    /// User-facing period name.
    pub name: String,
    /// Declared first day of week.
    pub start_day_of_week: String,
    /// Declared local start date such as `1/1`.
    pub start_date: String,
    /// Declared local end date such as `12/31`.
    pub end_date: String,
}

/// One source-aligned EPW radiation interval.
#[derive(Clone, Debug, PartialEq)]
pub struct EpwRecord {
    /// Source calendar year.
    pub year: i32,
    /// Source calendar month.
    pub month: u8,
    /// Source calendar day.
    pub day: u8,
    /// EPW end-of-interval hour in the range 1..=24.
    pub hour: u8,
    /// EPW end-of-interval minute in the range 1..=60.
    pub minute: u8,
    /// UTC Unix timestamp at the centre of the radiation interval.
    pub midpoint_unix_seconds_utc: i64,
    /// Interval duration in hours.
    pub duration_hours: f64,
    /// EPW data-source and uncertainty flags.
    pub data_source_and_uncertainty_flags: String,
    /// Dry-bulb temperature in degrees Celsius, when valid.
    pub dry_bulb_celsius: Option<f64>,
    /// Atmospheric pressure in pascals, when valid.
    pub atmospheric_pressure_pascals: Option<f64>,
    /// Global horizontal radiation accumulated over the interval, Wh/m².
    pub global_horizontal_wh_m2: f64,
    /// Direct normal radiation accumulated over the interval, Wh/m².
    pub direct_normal_wh_m2: f64,
    /// Diffuse horizontal radiation accumulated over the interval, Wh/m².
    pub diffuse_horizontal_wh_m2: f64,
    /// Surface albedo for the interval, when valid.
    pub albedo: Option<f64>,
    /// True when the source global-horizontal value was missing or invalid.
    pub global_horizontal_missing: bool,
    /// True when the source direct-normal value was missing or invalid.
    pub direct_normal_missing: bool,
    /// True when the source diffuse-horizontal value was missing or invalid.
    pub diffuse_horizontal_missing: bool,
}

impl EpwRecord {
    /// Converts this weather interval to a unit-weight solar schedule sample.
    #[must_use]
    pub const fn time_sample(&self) -> TimeSample {
        TimeSample {
            unix_seconds_utc: self.midpoint_unix_seconds_utc,
            duration_hours: self.duration_hours,
            weight: 1.0,
        }
    }
}

/// Counts of radiation values sanitized to zero during parsing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EpwMissingCounts {
    /// Missing or invalid global-horizontal intervals.
    pub global_horizontal: usize,
    /// Missing or invalid direct-normal intervals.
    pub direct_normal: usize,
    /// Missing or invalid diffuse-horizontal intervals.
    pub diffuse_horizontal: usize,
}

/// Fully parsed weather data with a deterministic content identity.
#[derive(Clone, Debug, PartialEq)]
pub struct EpwWeather {
    /// Geographic and station metadata.
    pub location: EpwLocation,
    /// Declared temporal resolution and period coverage.
    pub data_period: EpwDataPeriod,
    /// Ordered radiation intervals.
    pub records: Vec<EpwRecord>,
    /// Sanitization diagnostics retained for transparent reporting.
    pub missing_counts: EpwMissingCounts,
    /// BLAKE3 identity of the semantic parsed content.
    pub content_hash: [u8; 32],
}

impl EpwWeather {
    /// Creates a unit-weight schedule aligned exactly with every weather record.
    #[must_use]
    pub fn time_samples(&self) -> Vec<TimeSample> {
        self.records.iter().map(EpwRecord::time_sample).collect()
    }
}

/// EPW syntax or semantic validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EpwError {
    /// The file does not contain the eight mandatory header records.
    MissingHeaders,
    /// A mandatory header has the wrong name or field count.
    InvalidHeader {
        /// One-based source line number.
        line: usize,
        /// Human-readable field or header name.
        field: &'static str,
    },
    /// A field could not be parsed or is outside the supported range.
    InvalidField {
        /// One-based source line number.
        line: usize,
        /// Human-readable field name.
        field: &'static str,
    },
    /// No weather data records followed the headers.
    EmptyData,
    /// More than one declared data period is not yet representable as one timeline.
    MultipleDataPeriods,
}

impl fmt::Display for EpwError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingHeaders => formatter.write_str("EPW requires eight header records"),
            Self::InvalidHeader { line, field } => {
                write!(formatter, "invalid EPW header '{field}' on line {line}")
            }
            Self::InvalidField { line, field } => {
                write!(formatter, "invalid EPW field '{field}' on line {line}")
            }
            Self::EmptyData => formatter.write_str("EPW contains no weather data records"),
            Self::MultipleDataPeriods => {
                formatter.write_str("EPW files with multiple data periods are not supported")
            }
        }
    }
}

impl std::error::Error for EpwError {}

/// Parses an `EnergyPlus` Weather file from UTF-8 text.
pub fn parse_epw(text: &str) -> Result<EpwWeather, EpwError> {
    let lines = text
        .lines()
        .map(|line| line.trim_end_matches('\r'))
        .collect::<Vec<_>>();
    if lines.len() < HEADER_LINE_COUNT {
        return Err(EpwError::MissingHeaders);
    }
    let location = parse_location(lines[0])?;
    validate_header_names(&lines[..HEADER_LINE_COUNT])?;
    let data_period = parse_data_period(lines[7])?;
    let mut records = Vec::with_capacity(lines.len().saturating_sub(HEADER_LINE_COUNT));
    let mut missing_counts = EpwMissingCounts::default();
    for (line_index, line) in lines.iter().enumerate().skip(HEADER_LINE_COUNT) {
        if line.trim().is_empty() {
            continue;
        }
        let record = parse_record(
            line,
            line_index + 1,
            location.time_zone_hours,
            data_period.records_per_hour,
        )?;
        missing_counts.global_horizontal += usize::from(record.global_horizontal_missing);
        missing_counts.direct_normal += usize::from(record.direct_normal_missing);
        missing_counts.diffuse_horizontal += usize::from(record.diffuse_horizontal_missing);
        records.push(record);
    }
    if records.is_empty() {
        return Err(EpwError::EmptyData);
    }
    let content_hash = weather_hash(&location, &data_period, &records, missing_counts);
    Ok(EpwWeather {
        location,
        data_period,
        records,
        missing_counts,
        content_hash,
    })
}

/// Explicitly maps a typical meteorological year to one simulation calendar.
///
/// Source `record.year` values are retained; only UTC midpoint timestamps change.
/// Missing days and duplicate intervals remain detectable by weather policies.
pub fn with_simulation_year(weather: &EpwWeather, year: i32) -> Result<EpwWeather, EpwError> {
    let mut result = weather.clone();
    // Parsed timezone and interval values are bounded; nearest-second conversion is intentional.
    #[allow(clippy::cast_possible_truncation)]
    let offset = (weather.location.time_zone_hours * 3600.0).round() as i64;
    for (index, record) in result.records.iter_mut().enumerate() {
        let date = NaiveDate::from_ymd_opt(year, u32::from(record.month), u32::from(record.day))
            .ok_or(EpwError::InvalidField {
                line: index + 9,
                field: "simulation calendar date",
            })?;
        let end = date.and_hms_opt(0, 0, 0).expect("midnight")
            + TimeDelta::hours(i64::from(record.hour - 1))
            + TimeDelta::minutes(i64::from(record.minute));
        #[allow(clippy::cast_possible_truncation)]
        let half_interval_seconds = (record.duration_hours * 1800.0).round() as i64;
        record.midpoint_unix_seconds_utc =
            end.and_utc().timestamp() - offset - half_interval_seconds;
    }
    result.content_hash = weather_hash(
        &result.location,
        &result.data_period,
        &result.records,
        result.missing_counts,
    );
    Ok(result)
}

fn validate_header_names(lines: &[&str]) -> Result<(), EpwError> {
    const NAMES: [&str; HEADER_LINE_COUNT] = [
        "LOCATION",
        "DESIGN CONDITIONS",
        "TYPICAL/EXTREME PERIODS",
        "GROUND TEMPERATURES",
        "HOLIDAYS/DAYLIGHT SAVINGS",
        "COMMENTS 1",
        "COMMENTS 2",
        "DATA PERIODS",
    ];
    for (index, (line, expected)) in lines.iter().zip(NAMES).enumerate() {
        let actual = line
            .trim_start_matches('\u{feff}')
            .split(',')
            .next()
            .unwrap_or_default()
            .trim();
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(EpwError::InvalidHeader {
                line: index + 1,
                field: expected,
            });
        }
    }
    Ok(())
}

fn parse_location(line: &str) -> Result<EpwLocation, EpwError> {
    let fields = split_fields(line.trim_start_matches('\u{feff}'));
    if fields.len() < 10 || !fields[0].eq_ignore_ascii_case("LOCATION") {
        return Err(EpwError::InvalidHeader {
            line: 1,
            field: "LOCATION",
        });
    }
    let latitude = parse_number(fields[6], 1, "latitude")?;
    let longitude = parse_number(fields[7], 1, "longitude")?;
    let time_zone_hours = parse_number(fields[8], 1, "time zone")?;
    let elevation = parse_number(fields[9], 1, "elevation")?;
    if !(-12.0..=14.0).contains(&time_zone_hours)
        || (time_zone_hours * 4.0 - (time_zone_hours * 4.0).round()).abs() > 1.0e-9
    {
        return Err(EpwError::InvalidField {
            line: 1,
            field: "time zone",
        });
    }
    let solar = SolarLocation::try_new(latitude, longitude, elevation).map_err(|_| {
        EpwError::InvalidField {
            line: 1,
            field: "location coordinates",
        }
    })?;
    Ok(EpwLocation {
        city: fields[1].to_owned(),
        state_or_province: fields[2].to_owned(),
        country: fields[3].to_owned(),
        data_source: fields[4].to_owned(),
        station_identifier: fields[5].to_owned(),
        solar,
        time_zone_hours,
    })
}

fn parse_data_period(line: &str) -> Result<EpwDataPeriod, EpwError> {
    let fields = split_fields(line);
    if fields.len() < 7 || !fields[0].eq_ignore_ascii_case("DATA PERIODS") {
        return Err(EpwError::InvalidHeader {
            line: 8,
            field: "DATA PERIODS",
        });
    }
    let period_count = parse_integer::<u8>(fields[1], 8, "data period count")?;
    if period_count != 1 {
        return Err(EpwError::MultipleDataPeriods);
    }
    let records_per_hour = parse_integer::<u8>(fields[2], 8, "records per hour")?;
    if records_per_hour == 0 || records_per_hour > 60 || !60_u8.is_multiple_of(records_per_hour) {
        return Err(EpwError::InvalidField {
            line: 8,
            field: "records per hour",
        });
    }
    Ok(EpwDataPeriod {
        records_per_hour,
        name: fields[3].to_owned(),
        start_day_of_week: fields[4].to_owned(),
        start_date: fields[5].to_owned(),
        end_date: fields[6].to_owned(),
    })
}

fn parse_record(
    line: &str,
    line_number: usize,
    time_zone_hours: f64,
    records_per_hour: u8,
) -> Result<EpwRecord, EpwError> {
    let fields = split_fields(line);
    if fields.len() < MINIMUM_DATA_FIELDS {
        return Err(EpwError::InvalidField {
            line: line_number,
            field: "weather record field count",
        });
    }
    let year = parse_integer::<i32>(fields[0], line_number, "year")?;
    let month = parse_integer::<u8>(fields[1], line_number, "month")?;
    let day = parse_integer::<u8>(fields[2], line_number, "day")?;
    let hour = parse_integer::<u8>(fields[3], line_number, "hour")?;
    let raw_minute = parse_integer::<u8>(fields[4], line_number, "minute")?;
    // Hourly TMY3/OneBuilding files use minute=0 for the same end-hour convention.
    // Do not apply this compatibility rule to subhourly files.
    let minute = if raw_minute == 0 && records_per_hour == 1 {
        60
    } else {
        raw_minute
    };
    if !(1..=24).contains(&hour) || !(1..=60).contains(&minute) {
        return Err(EpwError::InvalidField {
            line: line_number,
            field: "end-of-interval time",
        });
    }
    let date = NaiveDate::from_ymd_opt(year, u32::from(month), u32::from(day)).ok_or(
        EpwError::InvalidField {
            line: line_number,
            field: "calendar date",
        },
    )?;
    let duration_minutes = i64::from(60 / records_per_hour);
    if i64::from(minute) % duration_minutes != 0 {
        return Err(EpwError::InvalidField {
            line: line_number,
            field: "minute resolution",
        });
    }
    let local_end = date
        .and_hms_opt(0, 0, 0)
        .expect("midnight is representable")
        + TimeDelta::hours(i64::from(hour - 1))
        + TimeDelta::minutes(i64::from(minute));
    let offset_seconds = (time_zone_hours * 3_600.0).round();
    #[allow(clippy::cast_possible_truncation)]
    let offset_seconds = offset_seconds as i64;
    let midpoint_unix_seconds_utc =
        local_end.and_utc().timestamp() - offset_seconds - duration_minutes * 30;
    let (global_horizontal_wh_m2, global_horizontal_missing) = parse_radiation(fields[13]);
    let (direct_normal_wh_m2, direct_normal_missing) = parse_radiation(fields[14]);
    let (diffuse_horizontal_wh_m2, diffuse_horizontal_missing) = parse_radiation(fields[15]);
    Ok(EpwRecord {
        year,
        month,
        day,
        hour,
        minute,
        midpoint_unix_seconds_utc,
        duration_hours: 1.0 / f64::from(records_per_hour),
        data_source_and_uncertainty_flags: fields[5].to_owned(),
        dry_bulb_celsius: parse_optional_range(fields[6], -90.0, 70.0, 99.9),
        atmospheric_pressure_pascals: parse_optional_range(
            fields[9], 31_000.0, 120_000.0, 999_999.0,
        ),
        global_horizontal_wh_m2,
        direct_normal_wh_m2,
        diffuse_horizontal_wh_m2,
        albedo: parse_optional_range(fields[32], 0.0, 1.0, 999.0),
        global_horizontal_missing,
        direct_normal_missing,
        diffuse_horizontal_missing,
    })
}

fn split_fields(line: &str) -> Vec<&str> {
    line.split(',').map(str::trim).collect()
}

fn parse_number(value: &str, line: usize, field: &'static str) -> Result<f64, EpwError> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or(EpwError::InvalidField { line, field })
}

fn parse_integer<T>(value: &str, line: usize, field: &'static str) -> Result<T, EpwError>
where
    T: core::str::FromStr,
{
    value
        .parse::<T>()
        .map_err(|_| EpwError::InvalidField { line, field })
}

fn parse_radiation(value: &str) -> (f64, bool) {
    match value.parse::<f64>() {
        Ok(value) if value.is_finite() && (0.0..9_999.0).contains(&value) => (value, false),
        _ => (0.0, true),
    }
}

fn parse_optional_range(value: &str, minimum: f64, maximum: f64, missing: f64) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && value.to_bits() != missing.to_bits())
        .filter(|value| (minimum..=maximum).contains(value))
}

pub fn weather_hash(
    location: &EpwLocation,
    period: &EpwDataPeriod,
    records: &[EpwRecord],
    missing: EpwMissingCounts,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"XVARNA_ZURVAN_EPW_V1\0");
    for value in [
        &location.city,
        &location.state_or_province,
        &location.country,
        &location.data_source,
        &location.station_identifier,
        &period.name,
        &period.start_day_of_week,
        &period.start_date,
        &period.end_date,
    ] {
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    for value in [
        location.solar.latitude_degrees,
        location.solar.longitude_degrees,
        location.solar.elevation_meters,
        location.time_zone_hours,
    ] {
        hasher.update(&value.to_bits().to_le_bytes());
    }
    hasher.update(&[period.records_per_hour]);
    for count in [
        missing.global_horizontal,
        missing.direct_normal,
        missing.diffuse_horizontal,
    ] {
        hasher.update(&(count as u64).to_le_bytes());
    }
    for record in records {
        hasher.update(&record.midpoint_unix_seconds_utc.to_le_bytes());
        for value in [
            record.duration_hours,
            record.global_horizontal_wh_m2,
            record.direct_normal_wh_m2,
            record.diffuse_horizontal_wh_m2,
            record.albedo.unwrap_or(f64::NAN),
        ] {
            hasher.update(&value.to_bits().to_le_bytes());
        }
        hasher.update(&[
            u8::from(record.global_horizontal_missing),
            u8::from(record.direct_normal_missing),
            u8::from(record.diffuse_horizontal_missing),
        ]);
    }
    *hasher.finalize().as_bytes()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    const HEADERS: &str = concat!(
        "LOCATION,Tehran,Tehran,IRN,IWEC,407540,35.68,51.32,3.5,1191\n",
        "DESIGN CONDITIONS,0\n",
        "TYPICAL/EXTREME PERIODS,0\n",
        "GROUND TEMPERATURES,0\n",
        "HOLIDAYS/DAYLIGHT SAVINGS,No,0,0,0\n",
        "COMMENTS 1,XVARNA fixture\n",
        "COMMENTS 2,interval-ending radiation\n",
        "DATA PERIODS,1,1,Data,Monday,1/1,12/31\n",
    );

    fn record(hour: u8, ghi: &str, dni: &str, dhi: &str, albedo: &str) -> String {
        let hour_text = hour.to_string();
        let mut fields = vec!["0"; 35];
        fields[0] = "2024";
        fields[1] = "1";
        fields[2] = "1";
        fields[3] = &hour_text;
        fields[4] = "60";
        fields[5] = "A7A7";
        fields[6] = "5";
        fields[9] = "90000";
        fields[12] = "333";
        fields[13] = ghi;
        fields[14] = dni;
        fields[15] = dhi;
        fields[31] = "88";
        fields[32] = albedo;
        fields.join(",") + "\n"
    }

    #[test]
    fn standard_epw_solar_columns_do_not_read_horizontal_infrared() {
        let weather =
            parse_epw(&(HEADERS.to_owned() + &record(12, "100", "200", "50", "0.39"))).unwrap();
        let r = &weather.records[0];
        assert_eq!(r.global_horizontal_wh_m2, 100.0);
        assert_eq!(r.direct_normal_wh_m2, 200.0);
        assert_eq!(r.diffuse_horizontal_wh_m2, 50.0);
        assert_eq!(r.albedo, Some(0.39));
    }

    #[test]
    fn hourly_tmy_minute_zero_preserves_the_end_hour() {
        let sixty = HEADERS.to_owned() + &record(1, "100", "200", "50", "0.2");
        let zero = sixty.replace("2024,1,1,1,60,", "2024,1,1,1,0,");
        assert_eq!(
            parse_epw(&sixty).unwrap().records[0].midpoint_unix_seconds_utc,
            parse_epw(&zero).unwrap().records[0].midpoint_unix_seconds_utc
        );
    }

    #[test]
    fn hourly_interval_is_converted_from_local_end_to_utc_midpoint() {
        let weather = parse_epw(&(HEADERS.to_owned() + &record(1, "100", "200", "50", "0.2")))
            .expect("fixture parses");
        let expected = NaiveDate::from_ymd_opt(2023, 12, 31)
            .expect("date")
            .and_hms_opt(21, 0, 0)
            .expect("time")
            .and_utc()
            .timestamp();
        assert_eq!(weather.records[0].midpoint_unix_seconds_utc, expected);
        assert_eq!(weather.records[0].duration_hours, 1.0);
        assert_eq!(weather.records[0].direct_normal_wh_m2, 200.0);
        assert_eq!(weather.records[0].albedo, Some(0.2));
    }

    #[test]
    fn missing_radiation_is_zeroed_and_counted_without_hiding_diagnostics() {
        let weather = parse_epw(&(HEADERS.to_owned() + &record(12, "9999", "-1", "bad", "999")))
            .expect("fixture parses");
        let record = &weather.records[0];
        assert_eq!(record.global_horizontal_wh_m2, 0.0);
        assert_eq!(record.direct_normal_wh_m2, 0.0);
        assert_eq!(record.diffuse_horizontal_wh_m2, 0.0);
        assert_eq!(record.albedo, None);
        assert_eq!(
            weather.missing_counts,
            EpwMissingCounts {
                global_horizontal: 1,
                direct_normal: 1,
                diffuse_horizontal: 1,
            }
        );
    }

    #[test]
    fn subhourly_records_use_declared_resolution_and_midpoints() {
        let headers = HEADERS.replace("DATA PERIODS,1,1,", "DATA PERIODS,1,4,");
        let source = headers
            + "2024,6,1,1,15,A,20,0,0,90000,0,0,10,20,5,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0.25,0,0,0\n";
        let weather = parse_epw(&source).expect("subhourly fixture parses");
        assert_eq!(weather.records[0].duration_hours, 0.25);
        let expected = NaiveDate::from_ymd_opt(2024, 5, 31)
            .expect("date")
            .and_hms_opt(20, 37, 30)
            .expect("time")
            .and_utc()
            .timestamp();
        assert_eq!(weather.records[0].midpoint_unix_seconds_utc, expected);
    }

    #[test]
    fn semantic_content_hash_is_deterministic() {
        let source = HEADERS.to_owned() + &record(8, "100", "200", "50", "0.2");
        assert_eq!(
            parse_epw(&source).expect("first").content_hash,
            parse_epw(&source).expect("second").content_hash
        );
    }

    #[test]
    fn explicit_simulation_year_preserves_source_year_and_changes_hash() {
        let first = record(24, "100", "200", "50", "0.2").replace("2024,1,1,", "2015,1,31,");
        let second = record(1, "100", "200", "50", "0.2").replace("2024,1,1,", "2025,2,1,");
        let source = parse_epw(&(HEADERS.to_owned() + &first + &second)).unwrap();
        let mapped = with_simulation_year(&source, 2001).unwrap();
        assert_eq!(mapped.records[0].year, 2015);
        assert_eq!(mapped.records[1].year, 2025);
        assert_eq!(
            mapped.records[1].midpoint_unix_seconds_utc
                - mapped.records[0].midpoint_unix_seconds_utc,
            3600
        );
        assert_ne!(mapped.content_hash, source.content_hash);
    }

    #[test]
    fn simulation_year_does_not_silently_drop_february_29() {
        let line = record(1, "100", "200", "50", "0.2").replace("2024,1,1,", "2024,2,29,");
        let source = parse_epw(&(HEADERS.to_owned() + &line)).unwrap();
        assert!(with_simulation_year(&source, 2001).is_err());
        assert!(with_simulation_year(&source, 2000).is_ok());
    }
}
