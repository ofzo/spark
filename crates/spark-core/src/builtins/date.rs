#![allow(unused)]
#![allow(unused_variables, unused_imports, dead_code)]
//! Date built-in.
//!
//! Implements the JavaScript Date constructor and its methods.
//! Dates store UTC epoch milliseconds in internal_slots["value"].
//! Calendar math uses the Howard Hinnant algorithm (no external dependencies).

use std::cell::RefCell;
use std::rc::Rc;

use crate::value::{JSValue, JSObject, FunctionBody};
use crate::context::JSContext;

// ============================================================================
// UTC calendar helpers (Howard Hinnant algorithm)
// ============================================================================

/// Days from 1970-01-01 to the civil date (year, month 1-12, day 1-31).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - (month <= 2) as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Convert days from epoch to (year, month 1-12, day 1-31).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Days in month (1-12) for the given year.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

/// Day of week (0=Sun, 1=Mon, ..., 6=Sat) from epoch days.
fn day_of_week_from_days(days: i64) -> i64 {
    ((days % 7) + 7) % 7
}

// ============================================================================
// Date <-> epoch millisecond conversions
// ============================================================================

/// Convert UTC (year, month 1-12, day 1-31, hour, min, sec, ms) to epoch ms.
fn utc_to_epoch_ms(
    year: i64, month: i64, day: i64,
    hour: i64, min: i64, sec: i64, ms: i64,
) -> f64 {
    let days = days_from_civil(year, month, day);
    ((days * 86_400_000) + hour * 3_600_000 + min * 60_000 + sec * 1_000 + ms) as f64
}

/// Convert epoch ms to UTC components: (year, month 1-12, day 1-31, hour, min, sec, ms).
fn epoch_ms_to_utc(ms: f64) -> (i64, i64, i64, i64, i64, i64, i64) {
    let total_ms = ms.floor() as i64;
    let total_days = total_ms.div_euclid(86_400_000);
    let ms_of_day = total_ms.rem_euclid(86_400_000);

    let hour = ms_of_day / 3_600_000;
    let remainder = ms_of_day % 3_600_000;
    let min = remainder / 60_000;
    let sec = (remainder % 60_000) / 1_000;
    let milli = remainder % 1_000;

    let (year, month, day) = civil_from_days(total_days);
    (year, month, day, hour, min, sec, milli)
}

// ============================================================================
// Date object helpers
// ============================================================================

/// Get the epoch ms from a Date object (this).
fn get_date_value(this: &JSValue) -> f64 {
    match this {
        JSValue::Object(obj) => {
            let borrow = obj.borrow();
            borrow
                .internal_slots
                .get("value")
                .map(|v| v.to_number())
                .unwrap_or(f64::NAN)
        }
        _ => f64::NAN,
    }
}

/// Create a Date object from epoch ms.
fn make_date(epoch_ms: f64) -> JSValue {
    let mut obj = JSObject {
        properties: std::collections::HashMap::new(),
        descriptors: std::collections::HashMap::new(),
        prototype: None,
        internal_slots: std::collections::HashMap::new(),
        class_name: "Date".to_string(),
    };
    obj.internal_slots.insert("value".to_string(), JSValue::Float(epoch_ms));
    JSValue::Object(Rc::new(RefCell::new(obj)))
}

/// Create an Invalid Date.
fn make_invalid_date() -> JSValue {
    make_date(f64::NAN)
}

/// Check if this date is valid.
fn is_valid_date(this: &JSValue) -> bool {
    let v = get_date_value(this);
    !v.is_nan() && v.is_finite()
}

/// Clamp a value to integer (for Date component arguments).
fn to_integer(v: f64) -> f64 {
    if v.is_nan() || v.is_infinite() {
        f64::NAN
    } else {
        v.trunc()
    }
}

// ============================================================================
// Date constructor
// ============================================================================

/// Date() / new Date() / new Date(value) / new Date(y,m,d,...)
pub fn date_constructor(_this: &JSValue, args: &[JSValue]) -> JSValue {
    if args.is_empty() {
        // new Date() -> current time
        let now_ms = current_time_ms();
        return make_date(now_ms);
    }
    if args.len() == 1 {
        let arg = args[0].to_number();
        if arg.is_nan() {
            return make_invalid_date();
        }
        // new Date(value) -> UTC ms
        return make_date(arg);
    }
    // new Date(year, month [, date [, hours [, minutes [, seconds [, ms ]]]]])
    let year = args[0].to_number() as i64;
    let month = args[1].to_number();
    let month_i = month as i64;
    let date_f = args.get(2).map(|v| to_integer(v.to_number())).unwrap_or(1.0);
    let hours_f = args.get(3).map(|v| to_integer(v.to_number())).unwrap_or(0.0);
    let minutes_f = args.get(4).map(|v| to_integer(v.to_number())).unwrap_or(0.0);
    let seconds_f = args.get(5).map(|v| to_integer(v.to_number())).unwrap_or(0.0);
    let ms_f = args.get(6).map(|v| to_integer(v.to_number())).unwrap_or(0.0);

    if month.is_nan() || date_f.is_nan() || hours_f.is_nan() || minutes_f.is_nan()
        || seconds_f.is_nan() || ms_f.is_nan()
    {
        return make_invalid_date();
    }

    let date = date_f as i64;
    let hours = hours_f as i64;
    let minutes = minutes_f as i64;
    let seconds = seconds_f as i64;
    let ms = ms_f as i64;

    let epoch = utc_to_epoch_ms(year, month_i + 1, date, hours, minutes, seconds, ms);
    make_date(epoch)
}

/// Get current time in milliseconds.
/// Uses the host-provided clock if available, otherwise falls back to SystemTime.
fn current_time_ms() -> f64 {
    if let Some(clock) = crate::interpreter::get_clock() {
        clock.now_ms()
    } else {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as f64
    }
}

// ============================================================================
// Static methods
// ============================================================================

/// Date.now() - Returns current time in ms since epoch.
pub fn date_now(_this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::Float(current_time_ms())
}

/// Date.parse(string) - Parses a date string and returns epoch ms.
pub fn date_parse(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let s = args.get(0).map(|v| v.to_string()).unwrap_or_default();
    match parse_date_string(&s) {
        Some(ms) => JSValue::Float(ms),
        None => JSValue::Float(f64::NAN),
    }
}

/// Date.UTC(year, month [, date [, hours [, minutes [, seconds [, ms ]]]]])
pub fn date_utc(_this: &JSValue, args: &[JSValue]) -> JSValue {
    let year = args[0].to_number() as i64;
    let month = args.get(1).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let date = args.get(2).map(|v| to_integer(v.to_number())).unwrap_or(1.0) as i64;
    let hours = args.get(3).map(|v| to_integer(v.to_number())).unwrap_or(0.0) as i64;
    let minutes = args.get(4).map(|v| to_integer(v.to_number())).unwrap_or(0.0) as i64;
    let seconds = args.get(5).map(|v| to_integer(v.to_number())).unwrap_or(0.0) as i64;
    let ms = args.get(6).map(|v| to_integer(v.to_number())).unwrap_or(0.0) as i64;
    JSValue::Float(utc_to_epoch_ms(year, month + 1, date, hours, minutes, seconds, ms))
}

// ============================================================================
// Date.prototype getters (UTC)
// ============================================================================

/// Date.prototype.getTime()
pub fn date_get_time(this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::Float(get_date_value(this))
}

/// Date.prototype.getFullYear() - UTC
pub fn date_get_full_year(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (y, _, _, _, _, _, _) = epoch_ms_to_utc(v);
    JSValue::Float(y as f64)
}

/// Date.prototype.getUTCFullYear()
pub fn date_get_utc_full_year(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_full_year(this, args)
}

/// Date.prototype.getMonth() - UTC
pub fn date_get_month(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (_, m, _, _, _, _, _) = epoch_ms_to_utc(v);
    JSValue::Float((m - 1) as f64)
}

/// Date.prototype.getUTCMonth()
pub fn date_get_utc_month(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_month(this, args)
}

/// Date.prototype.getDate() - UTC
pub fn date_get_date(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (_, _, d, _, _, _, _) = epoch_ms_to_utc(v);
    JSValue::Float(d as f64)
}

/// Date.prototype.getUTCDate()
pub fn date_get_utc_date(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_date(this, args)
}

/// Date.prototype.getDay() - UTC
pub fn date_get_day(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (total_ms, _) = (v.floor() as i64, ());
    let total_days = total_ms.div_euclid(86_400_000);
    JSValue::Float(day_of_week_from_days(total_days) as f64)
}

/// Date.prototype.getUTCDay()
pub fn date_get_utc_day(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_day(this, args)
}

/// Date.prototype.getHours() - UTC
pub fn date_get_hours(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (_, _, _, h, _, _, _) = epoch_ms_to_utc(v);
    JSValue::Float(h as f64)
}

/// Date.prototype.getUTCHours()
pub fn date_get_utc_hours(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_hours(this, args)
}

/// Date.prototype.getMinutes() - UTC
pub fn date_get_minutes(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (_, _, _, _, m, _, _) = epoch_ms_to_utc(v);
    JSValue::Float(m as f64)
}

/// Date.prototype.getUTCMinutes()
pub fn date_get_utc_minutes(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_minutes(this, args)
}

/// Date.prototype.getSeconds() - UTC
pub fn date_get_seconds(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (_, _, _, _, _, s, _) = epoch_ms_to_utc(v);
    JSValue::Float(s as f64)
}

/// Date.prototype.getUTCSeconds()
pub fn date_get_utc_seconds(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_seconds(this, args)
}

/// Date.prototype.getMilliseconds() - UTC
pub fn date_get_milliseconds(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (_, _, _, _, _, _, ms) = epoch_ms_to_utc(v);
    JSValue::Float(ms as f64)
}

/// Date.prototype.getUTCMilliseconds()
pub fn date_get_utc_milliseconds(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_get_milliseconds(this, args)
}

/// Date.prototype.getTimezoneOffset() - Always 0 (we use UTC internally)
pub fn date_get_timezone_offset(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let _ = get_date_value(this);
    JSValue::Float(0.0)
}

// ============================================================================
// Date.prototype setters (UTC)
// ============================================================================

/// Date.prototype.setTime(time)
pub fn date_set_time(this: &JSValue, args: &[JSValue]) -> JSValue {
    let val = args.get(0).map(|v| v.to_number()).unwrap_or(f64::NAN);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(val));
    }
    JSValue::Float(val)
}

/// Date.prototype.setFullYear(year [, month [, date]])
pub fn date_set_full_year(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    let base = if v.is_finite() { v } else { 0.0 };
    let (_, m, d, _, _, _, _) = epoch_ms_to_utc(base);
    let year = args[0].to_number() as i64;
    let month = args.get(1).map(|v| v.to_number()).unwrap_or((m - 1) as f64) as i64;
    let date = args.get(2).map(|v| v.to_number()).unwrap_or(d as f64) as i64;
    let epoch = utc_to_epoch_ms(year, month + 1, date, 0, 0, 0, 0);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCFullYear(year [, month [, date]])
pub fn date_set_utc_full_year(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_full_year(this, args)
}

/// Date.prototype.setMonth(month [, date])
pub fn date_set_month(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    let base = if v.is_finite() { v } else { 0.0 };
    let (y, _, d, _, _, _, _) = epoch_ms_to_utc(base);
    let month = args[0].to_number() as i64;
    let date = args.get(1).map(|v| v.to_number()).unwrap_or(d as f64) as i64;
    let epoch = utc_to_epoch_ms(y, month + 1, date, 0, 0, 0, 0);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCMonth(month [, date])
pub fn date_set_utc_month(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_month(this, args)
}

/// Date.prototype.setDate(date)
pub fn date_set_date(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    let base = if v.is_finite() { v } else { 0.0 };
    let (y, m, _, _, _, _, _) = epoch_ms_to_utc(base);
    let date = args[0].to_number() as i64;
    let epoch = utc_to_epoch_ms(y, m, date, 0, 0, 0, 0);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCDate(date)
pub fn date_set_utc_date(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_date(this, args)
}

/// Date.prototype.setHours(hour [, min [, sec [, ms]]])
pub fn date_set_hours(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (y, m, d, _, _, _, _) = epoch_ms_to_utc(v);
    let h = args[0].to_number() as i64;
    let min = args.get(1).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let sec = args.get(2).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let ms = args.get(3).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let epoch = utc_to_epoch_ms(y, m, d, h, min, sec, ms);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCHours(hour [, min [, sec [, ms]]])
pub fn date_set_utc_hours(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_hours(this, args)
}

/// Date.prototype.setMinutes(min [, sec [, ms]])
pub fn date_set_minutes(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (y, m, d, h, _, _, _) = epoch_ms_to_utc(v);
    let min = args[0].to_number() as i64;
    let sec = args.get(1).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let ms = args.get(2).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let epoch = utc_to_epoch_ms(y, m, d, h, min, sec, ms);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCMinutes(min [, sec [, ms]])
pub fn date_set_utc_minutes(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_minutes(this, args)
}

/// Date.prototype.setSeconds(sec [, ms])
pub fn date_set_seconds(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (y, m, d, h, min, _, _) = epoch_ms_to_utc(v);
    let sec = args[0].to_number() as i64;
    let ms = args.get(1).map(|v| v.to_number()).unwrap_or(0.0) as i64;
    let epoch = utc_to_epoch_ms(y, m, d, h, min, sec, ms);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCSeconds(sec [, ms])
pub fn date_set_utc_seconds(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_seconds(this, args)
}

/// Date.prototype.setMilliseconds(ms)
pub fn date_set_milliseconds(this: &JSValue, args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() { return JSValue::Float(f64::NAN); }
    let (y, m, d, h, min, s, _) = epoch_ms_to_utc(v);
    let ms = args[0].to_number() as i64;
    let epoch = utc_to_epoch_ms(y, m, d, h, min, s, ms);
    if let JSValue::Object(obj) = this {
        obj.borrow_mut()
            .internal_slots
            .insert("value".to_string(), JSValue::Float(epoch));
    }
    JSValue::Float(epoch)
}

/// Date.prototype.setUTCMilliseconds(ms)
pub fn date_set_utc_milliseconds(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_set_milliseconds(this, args)
}

// ============================================================================
// Date.prototype string methods
// ============================================================================

/// Pad a number to 2 digits.
fn pad2(n: i64) -> String {
    format!("{:02}", n)
}

/// Pad a number to 3 digits.
fn pad3(n: i64) -> String {
    format!("{:03}", n)
}

/// Date.prototype.toISOString()
pub fn date_to_iso_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() {
        return JSValue::string("Invalid Date");
    }
    let (y, m, d, h, min, s, ms) = epoch_ms_to_utc(v);
    JSValue::string(&format!(
        "{:04}-{}-{}T{}:{}:{}Z",
        y, pad2(m), pad2(d), pad2(h), pad2(min), pad2(s)
    ))
}

/// Date.prototype.toJSON(key)
pub fn date_to_json(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() {
        return JSValue::string("null");
    }
    date_to_iso_string(this, &[])
}

/// Date.prototype.toString()
pub fn date_to_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() {
        return JSValue::string("Invalid Date");
    }
    let (y, m, d, h, min, s, _) = epoch_ms_to_utc(v);
    let total_days = (v.floor() as i64).div_euclid(86_400_000);
    let dow = day_of_week_from_days(total_days);
    let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    JSValue::string(&format!(
        "{} {} {} {} {}:{:02}:{:02} UTC",
        day_names[dow as usize], m, d, y, h, min, s
    ))
}

/// Date.prototype.toUTCString()
pub fn date_to_utc_string(this: &JSValue, args: &[JSValue]) -> JSValue {
    date_to_string(this, args)
}

/// Date.prototype.toDateString()
pub fn date_to_date_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() {
        return JSValue::string("Invalid Date");
    }
    let (y, m, d, _, _, _, _) = epoch_ms_to_utc(v);
    let total_days = (v.floor() as i64).div_euclid(86_400_000);
    let dow = day_of_week_from_days(total_days);
    let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    JSValue::string(&format!("{} {} {} {}", day_names[dow as usize], m, d, y))
}

/// Date.prototype.toTimeString()
pub fn date_to_time_string(this: &JSValue, _args: &[JSValue]) -> JSValue {
    let v = get_date_value(this);
    if !v.is_finite() {
        return JSValue::string("Invalid Date");
    }
    let (_, _, _, h, min, s, _) = epoch_ms_to_utc(v);
    JSValue::string(&format!("{}:{:02}:{:02} UTC", h, min, s))
}

/// Date.prototype.valueOf()
pub fn date_value_of(this: &JSValue, _args: &[JSValue]) -> JSValue {
    JSValue::Float(get_date_value(this))
}

/// Date.prototype.getTimezoneOffset() (UTC always returns 0)
/// Already defined above.

// ============================================================================
// Simple date string parser
// ============================================================================

fn parse_date_string(s: &str) -> Option<f64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Try ISO 8601 format: YYYY-MM-DDTHH:mm:ss.sssZ or similar
    // Simplified parser that handles common formats
    let bytes = trimmed.as_bytes();

    if bytes.len() < 4 {
        return None;
    }

    let year = parse_ascii_digits(bytes, 0, 4)? as i64;
    let mut pos = 4;

    // Separator
    if pos < bytes.len() && (bytes[pos] == b'-' || bytes[pos] == b'/') {
        pos += 1;
    }

    let month = if pos + 2 <= bytes.len() {
        let m = parse_ascii_digits(bytes, pos, 2)? as i64;
        pos += 2;
        m
    } else {
        1
    };

    if pos < bytes.len() && (bytes[pos] == b'-' || bytes[pos] == b'/') {
        pos += 1;
    }

    let day = if pos + 2 <= bytes.len() {
        let d = parse_ascii_digits(bytes, pos, 2)? as i64;
        pos += 2;
        d
    } else {
        1
    };

    let mut hour = 0i64;
    let mut min = 0i64;
    let mut sec = 0i64;
    let mut ms = 0i64;

    if pos < bytes.len() && bytes[pos] == b'T' {
        pos += 1;
        if pos + 2 <= bytes.len() {
            hour = parse_ascii_digits(bytes, pos, 2)? as i64;
            pos += 2;
        }
        if pos < bytes.len() && bytes[pos] == b':' {
            pos += 1;
        }
        if pos + 2 <= bytes.len() {
            min = parse_ascii_digits(bytes, pos, 2)? as i64;
            pos += 2;
        }
        if pos < bytes.len() && bytes[pos] == b':' {
            pos += 1;
        }
        if pos + 2 <= bytes.len() {
            sec = parse_ascii_digits(bytes, pos, 2)? as i64;
            pos += 2;
        }
        if pos < bytes.len() && bytes[pos] == b'.' {
            pos += 1;
            let ms_str: String = bytes[pos..]
                .iter()
                .take_while(|&&b| b >= b'0' && b <= b'9')
                .map(|&b| b as char)
                .collect();
            if !ms_str.is_empty() {
                let padded = format!("{:0<3}", ms_str);
                ms = padded.parse::<i64>().unwrap_or(0);
            }
        }
    }

    Some(utc_to_epoch_ms(year, month, day, hour, min, sec, ms))
}

fn parse_ascii_digits(bytes: &[u8], start: usize, count: usize) -> Option<u64> {
    if start + count > bytes.len() {
        return None;
    }
    let mut result: u64 = 0;
    for i in start..start + count {
        let b = bytes[i];
        if b < b'0' || b > b'9' {
            return None;
        }
        result = result * 10 + (b - b'0') as u64;
    }
    Some(result)
}

// ============================================================================
// Initialization
// ============================================================================

/// Initialize the Date constructor and prototype.
pub fn init_date(ctx: &mut JSContext) {
    let date_ctor = JSValue::function(
        Some("Date"),
        vec![],
        FunctionBody::Native(date_constructor),
    );

    // Create Date.prototype
    let prototype = JSValue::object("Date");

    // Getter methods
    let getter_methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("getTime", date_get_time),
        ("getFullYear", date_get_full_year),
        ("getUTCFullYear", date_get_utc_full_year),
        ("getMonth", date_get_month),
        ("getUTCMonth", date_get_utc_month),
        ("getDate", date_get_date),
        ("getUTCDate", date_get_utc_date),
        ("getDay", date_get_day),
        ("getUTCDay", date_get_utc_day),
        ("getHours", date_get_hours),
        ("getUTCHours", date_get_utc_hours),
        ("getMinutes", date_get_minutes),
        ("getUTCMinutes", date_get_utc_minutes),
        ("getSeconds", date_get_seconds),
        ("getUTCSeconds", date_get_utc_seconds),
        ("getMilliseconds", date_get_milliseconds),
        ("getUTCMilliseconds", date_get_utc_milliseconds),
        ("getTimezoneOffset", date_get_timezone_offset),
    ];

    for &(name, func) in getter_methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    // Setter methods
    let setter_methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("setTime", date_set_time),
        ("setFullYear", date_set_full_year),
        ("setUTCFullYear", date_set_utc_full_year),
        ("setMonth", date_set_month),
        ("setUTCMonth", date_set_utc_month),
        ("setDate", date_set_date),
        ("setUTCDate", date_set_utc_date),
        ("setHours", date_set_hours),
        ("setUTCHours", date_set_utc_hours),
        ("setMinutes", date_set_minutes),
        ("setUTCMinutes", date_set_utc_minutes),
        ("setSeconds", date_set_seconds),
        ("setUTCSeconds", date_set_utc_seconds),
        ("setMilliseconds", date_set_milliseconds),
        ("setUTCMilliseconds", date_set_utc_milliseconds),
    ];

    for &(name, func) in setter_methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    // String methods
    let string_methods: &[(&str, fn(&JSValue, &[JSValue]) -> JSValue)] = &[
        ("toISOString", date_to_iso_string),
        ("toJSON", date_to_json),
        ("toString", date_to_string),
        ("toUTCString", date_to_utc_string),
        ("toDateString", date_to_date_string),
        ("toTimeString", date_to_time_string),
        ("valueOf", date_value_of),
    ];

    for &(name, func) in string_methods {
        prototype.set_property(
            name,
            JSValue::function(Some(name), vec![], FunctionBody::Native(func)),
        );
    }

    // Set prototype on constructor
    date_ctor.set_property("prototype", prototype);

    // Static methods
    date_ctor.set_property(
        "now",
        JSValue::function(Some("now"), vec![], FunctionBody::Native(date_now)),
    );
    date_ctor.set_property(
        "parse",
        JSValue::function(
            Some("parse"),
            vec!["string".to_string()],
            FunctionBody::Native(date_parse),
        ),
    );
    date_ctor.set_property(
        "UTC",
        JSValue::function(Some("UTC"), vec![], FunctionBody::Native(date_utc)),
    );

    // Install on global
    ctx.global
        .borrow_mut()
        .properties
        .insert("Date".to_string(), date_ctor);
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use crate::runtime::JSRuntime;

    fn make_ctx() -> JSContext {
        let rt = Rc::new(RefCell::new(JSRuntime::new()));
        JSContext::new(rt)
    }

    #[test]
    fn test_date_constructor_no_args() {
        let this = JSValue::undefined();
        let d = date_constructor(&this, &[]);
        assert!(is_valid_date(&d));
    }

    #[test]
    fn test_date_constructor_timestamp() {
        let this = JSValue::undefined();
        let d = date_constructor(&this, &[JSValue::Float(0.0)]);
        assert_eq!(get_date_value(&d), 0.0);
    }

    #[test]
    fn test_date_constructor_yyyymmdd() {
        let this = JSValue::undefined();
        // 1970-01-01T00:00:00Z = 0
        let d = date_constructor(&this, &[
            JSValue::int(1970),
            JSValue::int(0), // month is 0-based
            JSValue::int(1),
        ]);
        assert_eq!(get_date_value(&d), 0.0);
    }

    #[test]
    fn test_date_getters() {
        let this = JSValue::undefined();
        // 2020-06-15T12:30:45.123Z
        let epoch = utc_to_epoch_ms(2020, 6, 15, 12, 30, 45, 123);
        let d = make_date(epoch);

        assert_eq!(date_get_full_year(&d, &[]).to_number(), 2020.0);
        assert_eq!(date_get_month(&d, &[]).to_number(), 5.0); // June = 5
        assert_eq!(date_get_date(&d, &[]).to_number(), 15.0);
        assert_eq!(date_get_hours(&d, &[]).to_number(), 12.0);
        assert_eq!(date_get_minutes(&d, &[]).to_number(), 30.0);
        assert_eq!(date_get_seconds(&d, &[]).to_number(), 45.0);
        assert_eq!(date_get_milliseconds(&d, &[]).to_number(), 123.0);
    }

    #[test]
    fn test_date_setters() {
        let this = JSValue::undefined();
        let d = make_date(0.0);

        date_set_full_year(&d, &[JSValue::int(2025)]);
        assert_eq!(date_get_full_year(&d, &[]).to_number(), 2025.0);

        date_set_month(&d, &[JSValue::int(11)]); // December
        assert_eq!(date_get_month(&d, &[]).to_number(), 11.0);
    }

    #[test]
    fn test_date_to_iso_string() {
        let this = JSValue::undefined();
        let d = make_date(0.0);
        let result = date_to_iso_string(&d, &[]);
        assert_eq!(result.to_string(), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn test_date_invalid() {
        let this = JSValue::undefined();
        let d = make_invalid_date();
        let result = date_to_iso_string(&d, &[]);
        assert_eq!(result.to_string(), "Invalid Date");
    }

    #[test]
    fn test_date_now() {
        let this = JSValue::undefined();
        let result = date_now(&this, &[]);
        assert!(result.to_number() > 0.0);
    }

    #[test]
    fn test_date_utc() {
        let this = JSValue::undefined();
        let result = date_utc(&this, &[
            JSValue::int(1970),
            JSValue::int(0),
            JSValue::int(1),
        ]);
        assert_eq!(result.to_number(), 0.0);
    }

    #[test]
    fn test_init_date() {
        let mut ctx = make_ctx();
        init_date(&mut ctx);
        let global = ctx.global.borrow();
        assert!(global.properties.get("Date").is_some());
    }
}
