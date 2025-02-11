//! A library providing PostgreSQL extensions and utilities.
extern crate base64;
extern crate md5;
extern crate rand;

use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Datelike, Timelike};
#[allow(unused_imports)]
use pgrx::prelude::{
    pg_extern, pg_module_magic, pg_schema, pg_test, AnyNumeric, Date, Interval, Timestamp,
    VariadicArray,
};
use pgrx::AnyElement;
use rand::{rngs::ThreadRng, Rng};
use regex::Regex;
use serde_json::json;

pg_module_magic!();

/// Returns the version number of the library.
/// # Returns
/// A static string representing the library version.
#[pg_extern(create_or_replace)]
pub fn bfn_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Generates a Version 7 UUID (time-ordered).
/// # Returns
/// An uuid object containing a time-ordered UUID (v7).
#[pg_extern(create_or_replace)]
pub fn new_uuid() -> pgrx::Uuid {
    let uuid_v7 = uuid::Uuid::now_v7();
    pgrx::Uuid::from_bytes(*uuid_v7.as_bytes())
}

/// Converts a Version 7 UUID (UUIDv7) into a timestamp.
///
/// # Parameters
/// - `uuid`: A UUID input.
///
/// # Returns
/// The timestamp if the UUID contains a valid timestamp, or `null` if:
/// - The UUID is not Version 7.
/// - The extracted timestamp is out of the supported range.
/// - The timestamp data is invalid.
#[pg_extern(create_or_replace)]
pub fn uuid_to_ts(uuid: pgrx::Uuid) -> Option<Timestamp> {
    let bytes = uuid.as_bytes();
    let version = (bytes[6] >> 4) & 0x0F;
    if version != 7 {
        return None; // Not a V7 UUID
    }
    let timestamp_ms = ((bytes[0] as u64) << 40)
        | ((bytes[1] as u64) << 32)
        | ((bytes[2] as u64) << 24)
        | ((bytes[3] as u64) << 16)
        | ((bytes[4] as u64) << 8)
        | bytes[5] as u64;
    let timestamp_secs = (timestamp_ms / 1000) as i64;
    let timestamp_nanos = ((timestamp_ms % 1000) * 1_000_000) as u32;
    if timestamp_secs < -62_135_596_800 || timestamp_secs > 253_402_300_799 {
        return None; // Chronos only supports years between 0000 and 9999
    }
    let datetime_utc = match DateTime::from_timestamp(timestamp_secs, timestamp_nanos) {
        Some(dt) => dt,
        None => return None, // Invalid timestamp
    };
    let seconds_with_fraction = datetime_utc.second() as f64
        + (datetime_utc.timestamp_subsec_nanos() as f64 / 1_000_000_000.0);
    Timestamp::new(
        datetime_utc.year(),
        datetime_utc.month() as u8,
        datetime_utc.day() as u8,
        datetime_utc.hour() as u8,
        datetime_utc.minute() as u8,
        seconds_with_fraction,
    )
    .ok()
}

/// Extracts a `f64` value from a given array at the specified index.
///
/// Returns `None` if the index is out of bounds, the value is `None`, or cannot be parsed as `f64`.
fn extract_f64_from_vec(vec: &Vec<Option<AnyNumeric>>, index: usize) -> Option<f64> {
    vec.get(index).and_then(|v| {
        v.as_ref().and_then(|num| {
            num.to_string().parse::<f64>().ok() // Convert AnyNumeric to f64
        })
    })
}

/// Converts the given input parameters into a JSONB representation of an address.
///
/// # Parameters
/// - `street`: An optional string representing the street address.
/// - `city`: An optional string representing the city name.
/// - `postal_code`: An optional string representing the postal or ZIP code.
/// - `country`: An optional string representing the country name.
/// - `gps`: An optional vector containing two optional numeric values representing
///    GPS coordinates (latitude and longitude). If fewer than two values are present
///    or if the conversion to floating-point numbers fails, the GPS field
///    will be `null`.
/// - `adr_type`: An optional string representing the type of address (e.g., "home", "work").
///
/// # Returns
/// A `pgrx::JsonB` object containing:
/// - `address`: The provided street value (or `null` if not provided).
/// - `city`: The provided city value (or `null` if not provided).
/// - `postalCode`: The provided postal code value (or `null` if not provided).
/// - `country`: The provided country value (or `null` if not provided).
/// - `gps`: A vector of valid latitude and longitude values as floating-point numbers
///    if they were successfully converted; otherwise, `null`.
/// - `type`: The provided address type (or `null` if not provided).
#[pg_extern(create_or_replace)]
pub fn to_address(
    street: Option<&str>,
    city: Option<&str>,
    postal_code: Option<&str>,
    country: Option<&str>,
    gps: Option<Vec<Option<AnyNumeric>>>,
    adr_type: Option<&str>,
) -> pgrx::JsonB {
    let gps_valid = gps.and_then(|vec| {
        if vec.len() >= 2 {
            let first = extract_f64_from_vec(&vec, 0);
            let second = extract_f64_from_vec(&vec, 1);
            if let (Some(first), Some(second)) = (first, second) {
                Some(vec![first, second]) // Valid f64 numbers in Vec
            } else {
                None // Conversion failed for one or both elements
            }
        } else {
            None // Not enough elements in the vector
        }
    });
    pgrx::JsonB(json!({
        "address": street,
        "city": city,
        "postalCode": postal_code,
        "country": country,
        "gps": gps_valid,
        "type": adr_type
    }))
}

/// Returns an array of all dates between given dates, including given dates.
/// # Overview
/// This function returns a vector containing every date from `start` to `end`,
/// including both boundary dates. If the `start` date is greater than the `end` date,
/// an empty vector is returned.
///
/// # See Also
/// - [`date_range`](fn.date_range.html): The primary function that this alias is based on.
///
/// # Parameters
/// - `start`: The starting date of the range (inclusive).
/// - `end`: The ending date of the range (inclusive).
/// # Returns
/// - A `Vec<Date>` containing all dates within the specified range, including `start` and `end`.
/// - An empty vector if `start` is greater than `end`.
#[pg_extern(create_or_replace)]
pub fn all_dates_from(start: Date, end: Date) -> Vec<Date> {
    if start > end {
        return Vec::new();
    }
    let mut my_vec: Vec<Date> = Vec::new();
    let mut date = start;
    while date <= end {
        my_vec.push(date);
        let ts: Timestamp = date.into();
        date = (ts + Interval::from_days(1)).into();
    }
    my_vec
}

/// Generates a range of dates between the given start and end dates (inclusive).
///
/// Alias of the `all_dates_from` function.
///
/// # See Also
/// - [`all_dates_from`](fn.all_dates_from.html): The primary function that this alias is based on.
///
/// # Parameters
/// - `start`: The starting date of the range (inclusive).
/// - `end`: The ending date of the range (inclusive).
/// # Returns
/// - A `Vec<Date>` containing all dates within the specified range, including `start` and `end`.
/// - An empty vector if `start` is greater than `end`.
#[pg_extern(create_or_replace)]
pub fn date_range(start: Date, end: Date) -> Vec<Date> {
    all_dates_from(start, end)
}

/// Returns the first day of the given month.
///
/// This function takes a date representing any day in a given month
/// and returns the date for the `first day` of that month.
///
/// # Arguments
///
/// * `date` - A date object representing any day of the target month.
///
/// # Returns
///
/// A date object representing the first day of the given month.
#[pg_extern(create_or_replace)]
pub fn first_day_of_month(date: Date) -> Date {
    Date::new(date.year(), date.month(), 1).unwrap()
}

/// Compares two string values for equality (case-insensitive).
///
/// # Parameters
/// - `a`: The first optional string to compare.
/// - `b`: The second optional string to compare.
/// - `default`: An optional default value to use if `a` is `null` (not provided).
///
/// # Functionality
/// A default value may be optionally supplied to replace the first one if it is `null`.
/// - If `a` is `null`, the function uses `default` (if provided) for `a`.
/// - Both strings are trimmed of whitespace and converted to lowercase before the comparison.
/// - Returns `true` if the processed values of `a` and `b` are equal, otherwise returns `false`.
#[pg_extern(create_or_replace)]
pub fn isi(a: Option<&str>, b: Option<&str>, default: Option<&str>) -> bool {
    let a_san = a
        .unwrap_or_else(|| default.unwrap_or(""))
        .trim()
        .to_lowercase();
    let b_san = b.unwrap_or("").trim().to_lowercase();
    a_san == b_san
}

/// Determines whether the provided string is empty or contains only whitespace.
///
/// # Parameters
/// - `value`: String to validate.
///   - If `null`, it is considered empty.
///   - If it contains a value, leading and trailing whitespace will be ignored during the check.
///
/// # Returns
/// - `true` if the input is `null`, an empty string, or contains only whitespace characters (e.g., spaces, tabs, or newlines).
/// - `false` if the input contains any non-whitespace characters.
#[pg_extern(create_or_replace)]
pub fn is_empty(value: Option<&str>) -> bool {
    if value.is_none() {
        return true;
    }
    let trimmed = value.unwrap().trim();
    trimmed.is_empty()
}

/// Determines whether the provided value is `null`.
///
/// Useful when `is null` comparison is not for some reason practical.
///
/// # Parameters
/// - `value`: The value to be checked.
///
/// # Returns
/// - `true` if the value is `null`.
/// - `false` if the value is not `null` (contains some value).
#[pg_extern(create_or_replace)]
pub fn is_null(value: Option<AnyElement>) -> bool {
    value.is_none()
}

/// Checks if the value is false.
///
/// If you give it a value that's `false` or `null`, it will return `true`.
/// If the value is `true`, it will return `false`.
///
/// # Parameters
/// - `value`: An optional boolean. If it's `false` or `null`, the function returns `true`.
///            If it's `true`, the function returns `false`.
///
/// # Returns
/// - `true` if the input is `false` or `null`.
/// - `false` if the input is `true`.
#[pg_extern(create_or_replace)]
pub fn is_false(value: Option<bool>) -> bool {
    !is_true(value)
}

/// Checks if the value is true.
///
/// If you give it a value that's true, it says "yes, true!" by returning `true`.
/// If you give it nothing or a value that's not true, it simply says `false`.
///
/// # Parameters
/// - `value`: An optional boolean. If it's `true`, the function returns `true`.
///            If it's `null` or `false`, the function returns `false`.
///
/// # Returns
/// - `true` if the input value is `true`.
/// - `false` if the input is `null` or `false`.
#[pg_extern(create_or_replace)]
pub fn is_true(value: Option<bool>) -> bool {
    value.unwrap_or_else(|| false)
}

/// Determines whether the provided value is not `null`.
///
/// Useful when `is not null` comparison is not for some reason practical.
///
/// # Parameters
/// - `value`: The value to be checked.
///
/// # Returns
/// - `true` if the value is not `null` (contains some value).
/// - `false` if the value is `null`.
#[pg_extern(create_or_replace)]
pub fn not_null(value: Option<AnyElement>) -> bool {
    !value.is_none()
}

/// Returns the last day of the given month.
///
/// # Parameters
/// - `date`: A `Date` object representing any day of the target month.
///
/// # Returns
/// - A `Date` object representing the last day of the specified month.
#[pg_extern(create_or_replace)]
pub fn last_day_of_month(date: Date) -> Date {
    Date::new(
        date.year(),
        date.month(),
        last_day_of_month_ym(date.year(), date.month() as i32) as u8,
    )
    .unwrap()
}

/// Calculates the last day of a given month for a specific year.
///
/// This function determines the final day of a given month in a specified year.
/// It accounts for leap years when calculating February's length.
///
/// # Parameters
/// - `year`: The year as a 4-digit integer (e.g., 2023).
/// - `month`: The month as an integer (1 for January, 12 for December).
///
/// # Returns
/// - An integer representing the last day of the given month (28, 29, 30, or 31).
#[pg_extern(create_or_replace)]
pub fn last_day_of_month_ym(year: i32, month: i32) -> i32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 31,
    }
}

/// Uppercase first letter of given string. (internal access only
fn upper_first_internal(word: &str) -> String {
    let mut c = word.chars();
    match c.next() {
        None => String::new(),
        Some(ch) => ch.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Converts the first letter of a given string to uppercase.
///
/// This function takes a string slice and returns a new string with the first character
/// converted to uppercase while leaving the rest of the string unchanged.
/// If the input string is empty, it returns `null`.
///
/// # Parameters
/// - `word`: A string slice whose first letter will be converted to uppercase.
///
/// # Returns
/// - String with the first character capitalized if the input is non-empty.
/// - `null` if the input is an empty string.
#[pg_extern(create_or_replace)]
pub fn upper_first(word: &str) -> Option<String> {
    let mut c = word.chars();
    match c.next() {
        None => None,
        Some(ch) => Some(ch.to_uppercase().collect::<String>() + c.as_str()),
    }
}

/// Generates a random Base64-encoded string.
///
/// This function produces a random string of 36 bytes, encodes it using Base64,
/// and returns the resulting encoded string.
///
/// # Returns
/// - A string containing a random Base64-encoded value.
#[pg_extern(create_or_replace)]
pub fn random_base64() -> String {
    let mut rng: ThreadRng = rand::rng();
    let random_bytes: Vec<u8> = (0..36).map(|_| rng.random()).collect();
    general_purpose::STANDARD.encode(&random_bytes)
}

/// Computes the MD5 hash of a given string and encodes it as a Base64 string.
///
/// This function takes an input string, calculates its MD5 hash, and returns
/// the hash encoded in Base64 format.
///
/// # Parameters
/// - `in_string`: The input string for which the MD5 hash will be computed.
///
/// # Returns
/// - A string containing the Base64 representation of the MD5 hash.
#[pg_extern(create_or_replace)]
pub fn md5_as_base64(in_string: &str) -> String {
    let digest = md5::compute(in_string);
    let md5_hash_bytes = digest.0;
    general_purpose::STANDARD.encode(&md5_hash_bytes)
}

/// Computes the MD5 hash of a given string and converts the result into a UUID.
///
/// This function generates an `MD5` hash from the input string and uses the resulting
/// hash bytes to create a valid `UUID`.
///
/// # Parameters
/// - `in_string`: The input string for which the MD5 hash will be computed and turned into a UUID.
///
/// # Returns
/// - A `uuid` object derived from the MD5 hash of the input string.
#[pg_extern(create_or_replace)]
pub fn md5_as_uuid(in_string: &str) -> pgrx::Uuid {
    let digest = md5::compute(in_string);
    let md5_hash_bytes = digest.0;
    pgrx::Uuid::from_bytes(md5_hash_bytes)
}

/// Verifies whether a given string matches a specified MD5 hash encoded in Base64 format.
///
/// This function computes the MD5 hash of the provided string, encodes it as Base64,
/// and checks if it matches the provided Base64 hash.
///
/// # Parameters
/// - `in_string`: The input string to be hashed and verified.
/// - `in_hash`: The expected Base64-encoded MD5 hash to compare against.
///
/// # Returns
/// - `true` if the computed Base64-encoded MD5 hash matches the provided hash.
/// - `false` otherwise.
#[pg_extern(create_or_replace)]
pub fn md5_verify_base64(in_string: &str, in_hash: &str) -> bool {
    let hash_of_input = md5_as_base64(in_string);
    hash_of_input == in_hash
}

/// Verifies whether a given string matches a specified MD5 hash provided in UUID format.
///
/// This function computes the MD5 hash of the input string, converts it into a UUID,
/// and checks if it matches the provided UUID.
///
/// # Parameters
/// - `in_string`: The input string to be hashed and verified.
/// - `in_uuid`: The expected UUID derived from an MD5 hash to compare against.
///
/// # Returns
/// - `true` if the computed UUID matches the provided UUID.
/// - `false` otherwise.
#[pg_extern(create_or_replace)]
pub fn md5_verify_uuid(in_string: &str, in_uuid: pgrx::Uuid) -> bool {
    let ver_uuid = md5_as_uuid(in_string);
    ver_uuid == in_uuid
}

/// Removes leading and trailing whitespace from the given string.
///
/// This function takes an optional string input, trims any leading or trailing
/// whitespace, and returns the resulting string. If the input is `null`, an
/// empty string is returned.
///
/// # Parameters
/// - `value`: An optional string slice to be trimmed.
///
/// # Returns
/// - A string with leading and trailing whitespace removed. If the input is `null`,
///   an empty string is returned.
#[pg_extern(create_or_replace)]
pub fn trim(value: Option<&str>) -> String {
    let val = value.unwrap_or_else(|| "");
    val.trim().to_string()
}

/// Sanitizes and trims the given string.
///
/// Sanitizes the given string by removing leading and trailing whitespace and
/// replacing one or more consecutive spaces, tabs, or newlines with a single space.
///
/// If the input is `null`, an empty string is returned.
///
/// # Parameters
/// - `value`: An optional string slice to be sanitized.
///
/// # Returns
/// - A `String` with leading and trailing whitespace removed and all internal
///   whitespace collapsed into single spaces. If the input is `null`, an empty
///   string is returned.
#[pg_extern(create_or_replace)]
pub fn san_trim(value: Option<&str>) -> String {
    let re = Regex::new(r"\s+").unwrap();
    let val = value.unwrap_or_else(|| "");
    re.replace_all(val, " ").trim().to_string()
}

/// Sanitizes a string by removing all HTML tags.
///
/// Sanitizes the given string by removing all `HTML` tags and then
/// replacing all `<` or `>` characters in the final result width `«` or `»`.
/// It guarantees that the string is secure for usage within
/// any HTML/XML code without causing a disruption.
///
/// # Parameters
/// - `value`: An `Option<&str>` representing the input string to be sanitized.
///
/// # Returns
/// - A string with all HTML tags stripped and `<` or `>` characters replaced.
pub fn strip_tags(value: Option<&str>) -> String {
    if let Some(input) = value {
        let space_re = Regex::new(r"\s+").unwrap();
        let tag_re = Regex::new(r"</?[a-zA-Z][^>]*>").unwrap();
        let without_tags = tag_re.replace_all(input, "").to_string();
        let re_less = Regex::new(r"<(\s*<)*").unwrap(); // Collapse consecutive '<'
        let re_greater = Regex::new(r">(\s*>)*").unwrap(); // Collapse consecutive '>'
        let without_arr = re_greater
            .replace_all(&re_less.replace_all(&without_tags, "«"), "»")
            .to_string();
        space_re.replace_all(&without_arr, " ").trim().to_string()
    } else {
        String::new()
    }
}

/// Parses a given string to determine its boolean representation.
///
/// # Arguments
/// * `value` - A string slice representing the input to be parsed.
///
/// # Returns
/// * `true` - If the string starts with any of the following truthy prefixes (case-insensitive):
///   `"1"`, `"+1"`, `"+"`, `"tru"`, `"tr"`, `"t"`, `"yes"`, `"ye"`, `"y"`.
/// * `false` - If the string does not match any of the truthy prefixes.
///
/// # Behavior
/// The function is case-insensitive and checks if the input string starts with any of the predefined truthy prefixes.
/// If a match is found among the prefixes, the function returns `true`. Otherwise, it returns `false`.
#[pg_extern(create_or_replace)]
pub fn parse_bool(value: &str) -> bool {
    let prefixes = vec!["1", "+", "+1", "tru", "tr", "t", "yes", "ye", "y"];
    for prefix in prefixes {
        if value.to_lowercase().starts_with(prefix) {
            return true;
        }
    }
    false
}

/// Parses a given optional string into a 64-bit integer.
///
/// # Arguments
/// * `value` - An optional string slice. If `null` or an empty string, the default value will be treated as `""`.
///
/// # Returns
/// * An 64-bit integer parsed from the input string.
/// * Non-digit characters are removed from the string prior to parsing.
/// * If the resulting string is empty or cannot be parsed as an 64-bit integer, the function returns `0`.
///
/// # Behavior
/// 1. If the input is `null`, it is treated as an empty string.
/// 2. Removes all non-digit characters from the input string using a regular expression.
/// 3. Attempts to parse the sanitized string into a 64-bit integer.
/// 4. If parsing fails (e.g., the sanitized string is empty), the function safely returns `0`.
#[pg_extern(create_or_replace)]
pub fn parse_i64(value: Option<&str>) -> i64 {
    let val = value.unwrap_or_else(|| "");
    let re = Regex::new(r"\D").unwrap();
    let san = re.replace_all(val, "").to_string();
    san.parse::<i64>().unwrap_or(0)
}

/// Parses the given string to check if it matches a valid disposal code pattern.
///
/// This function validates whether the input string conforms to a disposal code format:
/// - Starts with a `D` (case-insensitive)
/// - Followed by 1 or 2 digits
/// - Optionally followed by a period and 1 or 2 additional digits
///
/// If the string matches the pattern, it returns the code with the first letter capitalized.
/// Otherwise, it returns `null`.
#[pg_extern(create_or_replace)]
pub fn parse_disposal_code(value: &str) -> Option<String> {
    let re = Regex::new(r"(?i)^D\d{1,2}$|^D\d{1,2}\.\d{1,2}$").unwrap();
    let san = value.trim();
    if let Some(mat) = re.captures(&san) {
        return upper_first(mat.get(0)?.as_str());
    }
    None
}

/// Parses the given string to check if it matches a valid recovery code pattern.
///
/// This function validates whether the input string conforms to a recovery code format:
/// - Starts with an `R` (case-insensitive)
/// - Followed by 1 or 2 digits
/// - Optionally followed by a period and 1 or 2 additional digits
///
/// If the string matches the pattern, it returns the code with the first letter capitalized.
/// Otherwise, it returns `null`.
#[pg_extern(create_or_replace)]
pub fn parse_recovery_code(value: &str) -> Option<String> {
    let re = Regex::new(r"(?i)^R\d{1,2}$|^R\d{1,2}\.\d{1,2}$").unwrap();
    let san = value.trim();
    if let Some(mat) = re.captures(&san) {
        return upper_first(mat.get(0)?.as_str());
    }
    None
}

/// Parses a given string into a valid LoW (List of Waste) code.
///
/// # Parameters
/// - `value`: A string representing the input to be parsed into a LoW code.
///   This may include extra characters that will be sanitized during the process.
///
/// # What it does
/// The function processes the input string as follows:
/// 1. Removes all characters except digits (`0-9`) and the wildcard character (`*`).
/// 2. After sanitization, it checks if the resulting string matches valid LoW code formats:
///    - 2 digits (e.g., `12`).
///    - 4 digits (e.g., `1234`).
///    - 6 digits (e.g., `123456`).
///    - 6 digits followed by a `*` (e.g., `123456*`).
/// 3. If the sanitized string matches one of the above formats, it is returned as the result.
/// 4. If the sanitized string is empty or does not match any of the valid formats, the function returns `None`.
///
/// # Returns
/// - `Option<String>`:
///   - `Some(String)`: The parsed, valid LoW code if the input string matches a known format.
///   - `None`: If the input cannot be sanitized into a valid LoW code.
#[pg_extern(create_or_replace)]
pub fn parse_low_code(value: &str) -> Option<String> {
    let re1 = Regex::new("[^0-9*]").unwrap();
    let san = re1.replace_all(value, "");
    if san.is_empty() {
        return None;
    }
    let re2 = Regex::new(r"^\d{2}$|^\d{4}$|^\d{6}$|^\d{6}\*$").unwrap();
    if let Some(mat) = re2.captures(&san) {
        return Some(mat.get(0)?.as_str().to_string());
    }
    None
}

/// Parses the given string into disposal code, recovery code, or LoW code.
///
/// The function determines the type of code to parse based on the `code_type` parameter:
/// - If `code_type` is `"disposalcode"`, it attempts to parse the input as a disposal code.
/// - If `code_type` is `"recoverycode"`, it attempts to parse the input as a recovery code.
/// - If `code_type` is `"lowcode"`, it attempts to parse the input as a low code.
///
/// If the provided `code_type` or `value` is empty, or if the `code_type` is invalid, it returns `null`.
///
/// # Parameters
/// - `value`: The string slice representing the potential code to parse.
/// - `code_type`: The type of code to parse (`"disposalcode"`, `"recoverycode"`, or `"lowcode"`).
///
/// # Returns
/// - String containing the parsed and formatted code if it matches the corresponding type and format.
/// - `null` if the `code_type` is invalid, `value` is empty, or the input does not match the expected code format.
///
//noinspection SpellCheckingInspection
#[pg_extern(create_or_replace)]
pub fn parse_env_code(value: &str, code_type: &str) -> Option<String> {
    if code_type.is_empty() || value.is_empty() {
        return None;
    }
    let c_type: &str = &*code_type.to_lowercase();
    match c_type {
        "disposalcode" => parse_disposal_code(value),
        "recoverycode" => parse_recovery_code(value),
        "lowcode" => parse_low_code(value),
        _ => None,
    }
}

/// Combines an array of names into a single formatted string.
///
/// # Parameters
/// - `in_names`: A list (or array) of optional names to process and combine.
///   Any `None` values or empty strings in the array will be ignored.
///
/// # What it does
/// This function processes a list of names in several steps:
/// 1. Filters out any empty or `None` values.
/// 2. Trims unnecessary spaces from each name.
/// 3. Replaces multiple spaces with a single space.
/// 4. Capitalizes the first letter of each word in the names.
/// 5. Combines all the processed names into a single string, separating them with spaces.
///
/// If the list is empty, the function returns an empty string.
///
/// # Example behavior
/// - Input: `["john", "  doe "]`
///   Output: `"John Doe"`
///
/// - Input: `[" alice", None, "BOB "]`
///   Output: `"Alice Bob"`
///
/// - Input: `[None, "", "   "]`
///   Output: `""`
#[pg_extern(create_or_replace)]
pub fn join_names_array(in_names: Vec<Option<String>>) -> String {
    let mut names: Vec<String> = in_names.into_iter().filter_map(|x| x).collect();
    names.retain(|name| !name.trim().is_empty());
    if names.is_empty() {
        return String::from("");
    }
    let re = Regex::new(r"\s+").unwrap();
    // Trim all names
    names = names
        .into_iter()
        .map(|name| name.trim().to_string())
        .collect();
    // Replace multiple blanks with single one
    names = names
        .into_iter()
        .map(|s| re.replace_all(&s, " ").to_string())
        .collect();
    // Uppercase all words
    names = names
        .into_iter()
        .map(|sentence| {
            sentence
                .split_whitespace()
                .map(|word| upper_first_internal(word))
                .collect::<Vec<String>>()
                .join(" ")
        })
        .collect();
    // Concat the result
    let full_name = names.join(" ");
    full_name
}

/// Combines a list of names into a single string.
///
/// # Parameters
/// - `in_names`: A list of names (or text values) to be combined.
///
/// # What it does
/// This function takes a list of names and merges them into one string.
/// It processes each name, converts it to a string if needed, and then joins them together.
///
/// # Example behavior
/// - If the input is `["Alice", "Bob", "Charlie"]`, the function returns `"Alice Bob Charlie"`.
/// - If the input is empty, it returns an empty string.
#[pg_extern(create_or_replace)]
pub fn join_names<'dat>(in_names: VariadicArray<'dat, &'dat str>) -> String {
    let mut converted_vars: Vec<Option<String>> = Vec::new();
    for item in in_names.iter() {
        if let Some(value) = item {
            converted_vars.push(Some(value.to_string()));
        }
    }
    join_names_array(converted_vars)
}

/// Scales given numeric value down by 1000
///
/// # Parameters
/// - `in_value`: The number you want to scale down.
///
/// # What it does
/// Reduces a given number by dividing it by 1000.
/// This function takes the number you provide and divides it by 1000, returning the result.
/// It's useful when you need to convert a large value into a smaller,
/// scaled-down version, for example grams to kilograms.
#[pg_extern(create_or_replace)]
pub fn metric_scale_down(in_value: AnyNumeric) -> AnyNumeric {
    in_value / AnyNumeric::from(1000)
}

/// Scales a number up by multiplying it by 1000.
///
/// # Parameters
/// - `in_value`: The number you want to scale up.
///
/// # What it does
/// This function takes the number you provide and multiplies it by 1000, returning a larger value.
/// It's useful when you need to convert a smaller number into a larger,
/// scaled-up version, for example kilograms to grams.
#[pg_extern(create_or_replace)]
pub fn metric_scale_up(in_value: AnyNumeric) -> AnyNumeric {
    let mut scaled = in_value;
    scaled = scaled * 1000;
    scaled
}

/// Returns the number provided, or `0` (zero) if no value is given.
///
/// # Parameters
/// - `num`: An optional numeric value. If no value is provided (`null`), it will default to `0`.
///
/// # What it does
/// If a number is given, it simply returns that number. If nothing is provided, it returns `0` as a fallback.
///
/// # Example behavior
/// - If the input is `42`, it returns `42`.
/// - If the input is `null`, it returns `0`.
#[pg_extern(create_or_replace)]
pub fn zero(num: Option<AnyNumeric>) -> AnyNumeric {
    if num.is_none() {
        return AnyNumeric::from(0);
    }
    num.unwrap()
}

/// Returns `0` (zero) if given value is `null` otherwise given value.
///
/// # Arguments
/// * `num` - An optional 64-bit integer.
///
/// # Returns
/// * The value contained in `num` if it is not `null`.
/// * `0` if `num` is `null`.
#[pg_extern(create_or_replace)]
pub fn zi64(num: Option<i64>) -> i64 {
    if num.is_none() {
        return 0;
    }
    num.unwrap()
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use super::*;
    use pgrx::AnyElement;
    use std::env;
    use std::fs;
    use toml::Value;
    use uuid::Uuid;

    /// Tests `bfn_version`
    #[pg_test]
    fn test_bfn_version() {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        let cargo_toml_path = format!("{}/Cargo.toml", manifest_dir);
        let cargo_toml_content =
            fs::read_to_string(cargo_toml_path).expect("Failed to read Cargo.toml");
        let parsed_toml: Value =
            toml::from_str(&cargo_toml_content).expect("Failed to parse Cargo.toml");
        if let Some(version) = parsed_toml
            .get("package")
            .and_then(|pkg| pkg.get("version"))
            .and_then(|v| v.as_str())
        {
            assert_eq!(bfn_version(), version);
        } else {
            panic!("Version not found in Cargo.toml");
        }
    }

    /// Tests `new_uuid`
    #[pg_test]
    fn test_new_uuid() {
        // Generate a UUID
        let v_uuid = new_uuid();
        let uuid_string = v_uuid.to_string();

        // Check that the UUID string is valid and has the correct format
        assert_eq!(uuid_string.len(), 36, "UUID string length is not 36");

        // Ensure the UUID follows the correct structure (8-4-4-4-12 format with hyphens in the right places)
        assert_eq!(
            uuid_string.chars().nth(8),
            Some('-'),
            "Missing hyphen at position 8"
        );
        assert_eq!(
            uuid_string.chars().nth(13),
            Some('-'),
            "Missing hyphen at position 13"
        );
        assert_eq!(
            uuid_string.chars().nth(18),
            Some('-'),
            "Missing hyphen at position 18"
        );
        assert_eq!(
            uuid_string.chars().nth(23),
            Some('-'),
            "Missing hyphen at position 23"
        );

        // Ensure the UUID is entirely alphanumeric (after removing hyphens) and follows UUID standards
        let stripped_uuid = uuid_string.replace("-", "");
        assert_eq!(stripped_uuid.len(), 32, "Stripped UUID length is not 32");
        assert!(
            stripped_uuid.chars().all(|c| c.is_ascii_hexdigit()),
            "UUID contains non-hexadecimal characters"
        );
    }

    /// Tests `isi`
    #[pg_test]
    fn test_isi() {
        assert_eq!(
            true,
            isi(
                Some("  minotaur  "),
                Some("  MINOTAUR    "),
                Some("  MINOTAUR    ")
            )
        );
        assert_eq!(
            true,
            isi(None, Some("  minotaur    "), Some("  MINOTAUR    "))
        );
        assert_eq!(
            false,
            isi(None, Some("  minotaur    "), Some("  HIPPO    "))
        );
        assert_eq!(
            true,
            isi(
                Some(" MINOTAUR "),
                Some("  minotaur    "),
                Some("  HIPPO    ")
            )
        );
        assert_eq!(false, isi(Some(" MINOTAUR "), None, Some("  HIPPO    ")));
        assert_eq!(false, isi(Some(" MINOTAUR "), None, None));
    }

    /// Tests `is_null`
    #[pg_test]
    fn test_is_null() {
        assert_eq!(true, is_null(None));
        let some: Option<AnyElement> = Some(unsafe { std::mem::zeroed() });
        assert_eq!(false, is_null(some));
    }

    /// Tests `is_empty`
    #[pg_test]
    fn test_is_empty() {
        // Test case: null value
        assert!(is_empty(None), "Expected true for None input");

        // Test case: empty string
        assert!(is_empty(Some("")), "Expected true for empty string");

        // Test case: string with only spaces
        assert!(
            is_empty(Some("   ")),
            "Expected true for string with spaces"
        );

        // Test case: string with tabs and newlines
        assert!(
            is_empty(Some(" \t\n ")),
            "Expected true for string with tabs and newlines"
        );

        // Test case: string with non-whitespace content
        assert!(
            !is_empty(Some("hello")),
            "Expected false for non-empty string"
        );

        // Test case: string with spaces and content
        assert!(
            !is_empty(Some("   hello   ")),
            "Expected false for string with spaces and content"
        );
    }

    /// Tests `not_null`
    #[pg_test]
    fn test_not_null() {
        assert_eq!(false, not_null(None));
    }

    /// Tests `upper_first`
    #[pg_test]
    fn test_upper_first() {
        //println!("{}", upper_first("tiny cat"));
        assert_eq!(Some("Minotaur".to_string()), upper_first("minotaur"));
        assert_eq!(
            Some("Old minotaur".to_string()),
            upper_first("old minotaur")
        );
        assert_eq!(None, upper_first(""));
    }

    /// Tests `san_trim`
    #[pg_test]
    fn test_san_trim() {
        assert_eq!(
            "Old Minotaur",
            san_trim(Some("  Old     Minotaur \n\n\n  \t \t    "))
        );
        assert_eq!("", san_trim(Some("    ")));
        assert_eq!("", san_trim(None));
    }

    /// Tests `strip_tags`
    #[test]
    fn test_strip_tags() {
        // Test case with HTML tags
        let input = Some("<h1>Hello</h1> <p>World!</p> <tag>Text</tag>");
        let expected = "Hello World! Text";
        assert_eq!(strip_tags(input), expected);

        // Test case with only < and > characters
        let input = Some("Some numbers <text>   < and > letters.");
        let expected = "Some numbers « and » letters.";
        assert_eq!(strip_tags(input), expected);

        // Test case with only < and > characters
        let input = Some("Some numbers <text>   and > letters.");
        let expected = "Some numbers and » letters.";
        assert_eq!(strip_tags(input), expected);

        // Test case with only < and > characters
        let input = Some("Some numbers <text> <<<  and   letters.");
        let expected = "Some numbers « and letters.";
        assert_eq!(strip_tags(input), expected);

        // Test case with only < and > characters
        let input = Some("Some numbers <text> <  <  <  and   letters.");
        let expected = "Some numbers « and letters.";
        assert_eq!(strip_tags(input), expected);

        // Test case with only < and > characters
        let input = Some("Some numbers <text>  and >  >    >  letters.");
        let expected = "Some numbers and » letters.";
        assert_eq!(strip_tags(input), expected);

        // Test case with extra whitespace
        let input = Some("   <div>  Line   with     spaces   </div>  ");
        let expected = "Line with spaces";
        assert_eq!(strip_tags(input), expected);

        // Test empty input
        let input = Some("");
        let expected = "";
        assert_eq!(strip_tags(input), expected);

        // Test case with no tags or < >
        let input = Some("Just plain text.");
        let expected = "Just plain text.";
        assert_eq!(strip_tags(input), expected);

        // Test None input
        let input: Option<&str> = None;
        let expected = "";
        assert_eq!(strip_tags(input), expected);
    }

    /// Parse `trim`
    #[pg_test]
    fn test_trim() {
        assert_eq!(
            "Old Minotaur",
            trim(Some("  Old Minotaur \n\n\n  \t \t    "))
        );
        assert_eq!("", trim(Some("    ")));
        assert_eq!("", trim(None));
    }

    /// Tests `parse_disposal_code`
    #[pg_test]
    fn test_parse_disposal_code() {
        assert_eq!(Some("D10".to_string()), parse_disposal_code("  d10   "));
        assert_eq!(
            Some("D10.21".to_string()),
            parse_disposal_code("  d10.21   ")
        );
        assert_eq!(None, parse_disposal_code("  d10.2143434   "));
    }

    /// Tests `parse_recovery_code`
    #[pg_test]
    fn test_parse_recovery_code() {
        //println!("{:?}", parse_recovery_code("  r10   "));
        assert_eq!(Some("R10".to_string()), parse_recovery_code("  r10   "));
        assert_eq!(
            Some("R10.21".to_string()),
            parse_recovery_code("  r10.21   ")
        );
        assert_eq!(None, parse_recovery_code("  r10.2143434   "));
    }

    /// Tests `parse_low_code`
    #[pg_test]
    fn test_parse_low_code() {
        assert_eq!(
            Some("102030".to_string()),
            parse_low_code("  10 20    30   ")
        );
        assert_eq!(
            Some("102030*".to_string()),
            parse_low_code("  10 20    30  * ")
        );
        assert_eq!(
            Some("102030*".to_string()),
            parse_low_code("  abs 10 c 20   30  * ")
        );
        assert_eq!(
            Some("1020".to_string()),
            parse_low_code("  abs 10 c 20      ")
        );
        assert_eq!(Some("10".to_string()), parse_low_code("  abs 10 c       "));
        assert_eq!(None, parse_low_code("  bla bla * "));
        assert_eq!(None, parse_low_code(" abc 125"));
        assert_eq!(None, parse_low_code("  10   * "));
        assert_eq!(None, parse_low_code(""));
    }

    /// Tests `parse_env_code`
    #[pg_test]
    fn test_parse_env_code() {
        assert_eq!(None, parse_env_code("  bla bla * ", "loWCode"));
        assert_eq!(
            Some("102030*".to_string()),
            parse_env_code("  10 20    30  * ", "loWCode")
        );
    }

    /// `is_false`
    #[pg_test]
    fn test_is_false() {
        assert_eq!(false, is_false(Some(true)));
        assert_eq!(true, is_false(None));
        assert_eq!(true, is_false(Some(false)));
    }

    /// Tests `is_true`
    #[pg_test]
    fn test_is_true() {
        assert_eq!(true, is_true(Some(true)));
        assert_eq!(false, is_true(None));
        assert_eq!(false, is_true(Some(false)));
    }

    /// Tests `md5_as_base64`
    //noinspection SpellCheckingInspection
    #[pg_test]
    fn test_md5_as_base64() {
        assert_eq!("bNNVbesNpUvKBgtMOUeYOQ==", md5_as_base64("Hello, world!"));
    }

    /// Tests `md5_verify_base64`
    //noinspection SpellCheckingInspection
    #[pg_test]
    fn test_md5_verify_base64() {
        assert_eq!(
            true,
            md5_verify_base64("Hello, world!", "bNNVbesNpUvKBgtMOUeYOQ==")
        );
    }

    /// Tests `md5_as_uuid`
    #[pg_test]
    fn test_to_md5uuid() {
        let test_val = "6cd3556d-eb0d-a54b-ca06-0b4c39479839";
        let res = md5_as_uuid("Hello, world!").to_string();
        assert_eq!(test_val, res);
    }

    #[pg_test]
    fn test_md5_verify_uuid() {
        let verify_val = "6cd3556d-eb0d-a54b-ca06-0b4c39479839";
        let as_uuid = Uuid::parse_str(verify_val).expect("Failed to parse UUID");
        let pgrx_uuid = pgrx::Uuid::from_bytes(*as_uuid.as_bytes());
        let res = md5_verify_uuid("Hello, world!", pgrx_uuid);
        assert_eq!(true, res);
    }

    /// Tests `join_names_array`
    #[pg_test]
    fn test_join_names_array() {
        let words: Vec<Option<String>> = vec![
            Some("  Hello  ".to_string()),
            None,
            Some("  my    small  ".to_string()),
            Some(" ".to_string()),
            Some(" ".to_string()),
            Some(" world ".to_string()),
        ];
        let full_name = join_names_array(words);
        assert_eq!(full_name, "Hello My Small World");
    }

    // Tests `metric_scale_down`
    #[pg_test]
    fn test_metric_scale_down() {
        // Test case 1: Standard positive input
        let input = AnyNumeric::from(1000);
        let expected = AnyNumeric::from(1);
        let result = metric_scale_down(input);
        assert_eq!(result, expected, "Failed to scale down 1000 to 1");

        // Test case 2: Large positive input
        let input = AnyNumeric::from(1_000_000);
        let expected = AnyNumeric::from(1000);
        let result = metric_scale_down(input);
        assert_eq!(result, expected, "Failed to scale down 1,000,000 to 1000");

        // Test case 3: Zero input
        let input = AnyNumeric::from(0);
        let expected = AnyNumeric::from(0);
        let result = metric_scale_down(input);
        assert_eq!(result, expected, "Failed to scale down 0 to 0");

        // Test case 4: Negative input
        let input = AnyNumeric::from(-1000);
        let expected = AnyNumeric::from(-1);
        let result = metric_scale_down(input);
        assert_eq!(result, expected, "Failed to scale down -1000 to -1");

        // Test case 5: Small positive input less than 1000
        let input = AnyNumeric::from(500);
        let expected = AnyNumeric::try_from("0.5").unwrap(); // Use high-precision string for fractional value
        let result = metric_scale_down(input);
        assert_eq!(result, expected, "Failed to scale down 500 to 0.5");

        // Test case 6: Small negative input less than -1000
        let input = AnyNumeric::from(-250);
        let expected = AnyNumeric::try_from("-0.25").unwrap(); // Use high-precision string for fractional value
        let result = metric_scale_down(input);
        assert_eq!(result, expected, "Failed to scale down -250 to -0.25");
    }

    /// Tests `metric_scale_up`
    #[pg_test]
    fn test_metric_scale_up() {
        // Test case 1: Positive number scaling
        let input = AnyNumeric::try_from(1.0).unwrap(); // Convert to AnyNumeric
        let result = metric_scale_up(input);
        let expected = AnyNumeric::try_from(1000.0).unwrap(); // Expected value
        assert_eq!(result, expected);

        // Test case 2: Decimal number scaling
        let input = AnyNumeric::try_from(2.5).unwrap(); // 2.5
        let result = metric_scale_up(input);
        let expected = AnyNumeric::try_from(2500.0).unwrap(); // 2.5 * 1000 = 2500.0
        assert_eq!(result, expected);

        // Test case 3: Negative number scaling
        let input = AnyNumeric::try_from(-1.5).unwrap(); // -1.5
        let result = metric_scale_up(input);
        let expected = AnyNumeric::try_from(-1500.0).unwrap(); // -1.5 * 1000 = -1500.0
        assert_eq!(result, expected);

        // Test case 4: Zero scaling
        let input = AnyNumeric::try_from(0.0).unwrap(); // 0.0
        let result = metric_scale_up(input);
        let expected = AnyNumeric::try_from(0.0).unwrap(); // 0.0 * 1000 = 0.0
        assert_eq!(result, expected);
    }

    /// Tests `all_dates_from`
    #[pg_test]
    fn test_all_dates_from() {
        let a = Date::new(2023, 12, 12).unwrap();
        let b = Date::new(2023, 12, 15).unwrap();
        let res1 = all_dates_from(a, b);
        assert_eq!(4, res1.len());
        let x = Date::new(2023, 9, 2).unwrap();
        let y = Date::new(2024, 5, 28).unwrap();
        let res2 = all_dates_from(x, y);
        assert_eq!(270, res2.len());
    }

    /// Tests `first_day_of_month`
    #[pg_test]
    fn test_first_day_of_month() {
        let date = Date::new(2023, 12, 12).unwrap();
        let month_start = first_day_of_month(date);
        assert_eq!("2023-12-01", month_start.to_string());
    }

    /// Tests `last_day_of_month`
    #[pg_test]
    fn test_last_day_of_month() {
        let date1 = Date::new(2023, 7, 12).unwrap();
        let date2 = Date::new(2024, 2, 7).unwrap();
        assert_eq!("2023-07-31", last_day_of_month(date1).to_string());
        assert_eq!("2024-02-29", last_day_of_month(date2).to_string());
    }

    /// Tests `parse_pool`
    #[pg_test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("yes"), true);
        assert_eq!(parse_bool("true"), true);
        assert_eq!(parse_bool("T"), true);
        assert_eq!(parse_bool("no"), false);
    }

    /// Tests `zero`
    #[pg_test]
    fn test_zero() {
        assert_eq!(AnyNumeric::from(100), zero(Some(AnyNumeric::from(100))));
        assert_eq!(AnyNumeric::from(4567), zero(Some(AnyNumeric::from(4567))));
        assert_eq!(AnyNumeric::from(0), zero(None));
    }

    /// Tests `zi64`
    #[pg_test]
    fn test_zi64() {
        assert_eq!(100, zi64(Some(100)));
        assert_eq!(154454300, zi64(Some(154454300)));
        assert_eq!(0, zi64(None));
    }

    /// Validate `i64`
    #[pg_test]
    fn test_parse_i64() {
        assert_eq!(0, parse_i64(None));
        assert_eq!(100, parse_i64(Some("  1 xcv 0 0")));
        assert_eq!(456789, parse_i64(Some("  4cc5 6y 7 8%9  ")));
        assert_eq!(456789, parse_i64(Some("  4cc5., 6y 7 8%9  ")));
    }

    /// Tests `to_address`
    #[pg_test]
    fn test_to_address_with_valid_gps() {
        let gps = Some(vec![
            Some(AnyNumeric::try_from(1.23).unwrap()),
            Some(AnyNumeric::try_from(4.56).unwrap()),
            None,
        ]);
        let result = to_address(
            Some("Main St."),
            Some("New York"),
            Some("12345"),
            Some("USA"),
            gps,
            Some("home"),
        );
        let expected = pgrx::JsonB(json!({
            "address": "Main St.",
            "city": "New York",
            "postalCode": "12345",
            "country": "USA",
            "gps": [1.23, 4.56],
            "type": "home"
        }));
        let result_str = serde_json::to_string(&result).unwrap();
        let expected_str = serde_json::to_string(&expected).unwrap();
        assert_eq!(result_str, expected_str);
    }

    /// Tests `uuid_to_ts`
    #[pg_test]
    fn test_uuid_to_ts_a() {
        // 2023-11-26 16:48:29.952000 +00:00
        let uuid = Uuid::from_bytes([
            0x01, 0x8c, 0x0c, 0x88, 0x53, 0x00, 0x7c, 0xb5, 0xb7, 0x8c, 0xdc, 0x0f, 0xe5, 0x42,
            0x78, 0x27,
        ]);
        let timestamp = uuid_to_ts(pgrx::Uuid::from_bytes(*uuid.as_bytes()));
        assert!(timestamp.is_some());
        let ts = timestamp.unwrap();
        // Assert fields of the Timestamp (you may adjust the exact values depending on expectations)
        assert_eq!(ts.year(), 2023); // Replace with the actual expected year
        assert_eq!(ts.month(), 11); // Replace with the actual expected month
        assert_eq!(ts.day(), 26); // Replace with the actual expected day
        assert_eq!(ts.hour(), 16); // Replace with actual expected hour
        assert_eq!(ts.minute(), 48); // Replace with actual expected minute
        assert!((ts.second() - 29.952).abs() < 0.1); // seconds with a small margin
    }

    /// Tests `uuid_to_ts`
    #[pg_test]
    fn test_uuid_to_ts_b() {
        // An invalid UUID (not version 7)
        let uuid = Uuid::from_bytes([
            0x55, 0x0e, 0x84, 0x00, 0xe2, 0x9b, 0x41, 0xd4, 0xa7, 0x16, 0x44, 0x66, 0x55, 0x44,
            0x00, 0x00,
        ]);
        let timestamp = uuid_to_ts(pgrx::Uuid::from_bytes(*uuid.as_bytes()));
        assert!(timestamp.is_none());
    }

    /// Tests `uuid_to_ts`
    #[pg_test]
    fn test_uuid_to_ts_c() {
        // A version 7 UUID with an out-of-range timestamp
        let uuid = Uuid::from_bytes([
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f, 0xbb, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc, 0xcc,
            0xcc, 0xcc,
        ]);
        // Call the uuid_to_ts function
        let timestamp = uuid_to_ts(pgrx::Uuid::from_bytes(*uuid.as_bytes()));
        // Assert the timestamp is None for out-of-range timestamps
        assert!(timestamp.is_none());
    }

    //
}

/// This module is required by `cargo pgrx test` invocations.
/// It must be visible at the root of your extension crate.
#[cfg(test)]
pub mod pg_test {
    pub fn setup(_options: Vec<&str>) {
        // perform one-off initialization when the pg_test framework starts
    }

    pub fn postgresql_conf_options() -> Vec<&'static str> {
        // return any postgresql.conf settings that are required for your tests
        vec![]
    }
}
