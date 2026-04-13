use chrono::{DateTime, Datelike, NaiveDateTime, Utc};
use std::fs;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone, Default)]
struct ExifTagValues {
    date_time_digitized: Option<String>,
    date_time_original: Option<String>,
    date_time: Option<String>,
}

pub(crate) fn read_exif_timestamp(path: &Path, min_year: Option<i32>) -> Option<DateTime<Utc>> {
    if !is_supported_image_extension(path) {
        return None;
    }

    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let exif_reader = exif::Reader::new().read_from_container(&mut reader).ok()?;

    let values = ExifTagValues {
        date_time_digitized: exif_reader
            .get_field(exif::Tag::DateTimeDigitized, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string()),
        date_time_original: exif_reader
            .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string()),
        date_time: exif_reader
            .get_field(exif::Tag::DateTime, exif::In::PRIMARY)
            .map(|field| field.display_value().to_string()),
    };

    select_timestamp_from_tag_values(&values, min_year)
}

fn select_timestamp_from_tag_values(
    values: &ExifTagValues,
    min_year: Option<i32>,
) -> Option<DateTime<Utc>> {
    [
        values.date_time_digitized.as_deref(),
        values.date_time_original.as_deref(),
        values.date_time.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find_map(|raw| {
        let parsed = parse_exif_datetime(raw)?;
        if min_year.is_some_and(|year| parsed.year() < year) {
            None
        } else {
            Some(parsed)
        }
    })
}

fn parse_exif_datetime(raw: &str) -> Option<DateTime<Utc>> {
    let trimmed = raw.trim();

    let parse_with_tz = [
        "%Y:%m:%d %H:%M:%S%:z",
        "%Y:%m:%d %H:%M:%S%z",
        "%Y-%m-%d %H:%M:%S%:z",
        "%Y-%m-%d %H:%M:%S%z",
    ]
    .iter()
    .find_map(|format| DateTime::parse_from_str(trimmed, format).ok());

    if let Some(with_tz) = parse_with_tz {
        return Some(with_tz.with_timezone(&Utc));
    }

    ["%Y:%m:%d %H:%M:%S", "%Y-%m-%d %H:%M:%S"]
        .iter()
        .find_map(|format| NaiveDateTime::parse_from_str(trimmed, format).ok())
        .map(|naive| DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

fn is_supported_image_extension(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "jpg" | "jpeg" | "png" | "bmp" | "heic" | "heif"
    )
}

#[cfg(test)]
mod tests {
    use super::{ExifTagValues, select_timestamp_from_tag_values};
    use chrono::{TimeZone, Utc};

    #[test]
    fn falls_back_to_date_time_digitized_first() {
        let values = ExifTagValues {
            date_time_digitized: Some("2020:01:02 03:04:05".to_string()),
            date_time_original: Some("2019:01:02 03:04:05".to_string()),
            date_time: Some("2018:01:02 03:04:05".to_string()),
        };

        let parsed = select_timestamp_from_tag_values(&values, None).expect("timestamp");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2020, 1, 2, 3, 4, 5).unwrap());
    }

    #[test]
    fn falls_back_to_date_time_original_when_digitized_missing() {
        let values = ExifTagValues {
            date_time_digitized: None,
            date_time_original: Some("2019:01:02 03:04:05".to_string()),
            date_time: Some("2018:01:02 03:04:05".to_string()),
        };

        let parsed = select_timestamp_from_tag_values(&values, None).expect("timestamp");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2019, 1, 2, 3, 4, 5).unwrap());
    }

    #[test]
    fn falls_back_to_date_time_when_others_missing() {
        let values = ExifTagValues {
            date_time_digitized: None,
            date_time_original: None,
            date_time: Some("2018-01-02 03:04:05".to_string()),
        };

        let parsed = select_timestamp_from_tag_values(&values, None).expect("timestamp");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2018, 1, 2, 3, 4, 5).unwrap());
    }

    #[test]
    fn parses_timezone_offset_when_present() {
        let values = ExifTagValues {
            date_time_digitized: Some("2020:01:02 03:04:05+02:30".to_string()),
            ..Default::default()
        };

        let parsed = select_timestamp_from_tag_values(&values, None).expect("timestamp");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2020, 1, 2, 0, 34, 5).unwrap());
    }

    #[test]
    fn parses_without_timezone_when_absent() {
        let values = ExifTagValues {
            date_time_digitized: Some("2020:01:02 03:04:05".to_string()),
            ..Default::default()
        };

        let parsed = select_timestamp_from_tag_values(&values, None).expect("timestamp");
        assert_eq!(parsed, Utc.with_ymd_and_hms(2020, 1, 2, 3, 4, 5).unwrap());
    }

    #[test]
    fn invalid_date_strings_are_rejected() {
        let values = ExifTagValues {
            date_time_digitized: Some("bad-value".to_string()),
            ..Default::default()
        };

        let parsed = select_timestamp_from_tag_values(&values, None);
        assert!(parsed.is_none());
    }

    #[test]
    fn years_before_minimum_are_filtered_out() {
        let values = ExifTagValues {
            date_time_digitized: Some("1979:12:31 23:59:59".to_string()),
            date_time_original: Some("1980:01:01 00:00:00".to_string()),
            ..Default::default()
        };

        let parsed = select_timestamp_from_tag_values(&values, Some(1980)).expect("timestamp");
        assert_eq!(parsed, Utc.with_ymd_and_hms(1980, 1, 1, 0, 0, 0).unwrap());
    }
}
