use crate::extendedbigdecimal::ExtendedBigDecimal;
use crate::number::PreciseNumber;
use crate::numberparse::ParseNumberError;
use bigdecimal::BigDecimal;
use num_bigint::BigInt;

pub fn parse_hexadecimal_float(s: &str) -> Result<PreciseNumber, ParseNumberError> {
    let (value, scale) = parse_float(s).map(|x| float_to_scaled_integer(x, None))?;
    let num = BigInt::from(value);
    let num = BigDecimal::from_bigint(num, -scale);
    let fractional_digits = if scale < 0 { -scale as usize } else { 0 };
    Ok(PreciseNumber::new(
        ExtendedBigDecimal::BigDecimal(num),
        0,
        fractional_digits,
    ))
}

fn parse_float(s: &str) -> Result<f64, ParseNumberError> {
    let mut s = s.trim();

    // Detect a sign
    let sign = if s.starts_with('-') {
        s = &s[1..];
        -1.0
    } else if s.starts_with('+') {
        s = &s[1..];
        1.0
    } else {
        1.0
    };

    // Is HEX?
    if s.starts_with("0x") || s.starts_with("0X") {
        s = &s[2..];
    } else {
        return Err(ParseNumberError::Float);
    }

    // Read an integer part (if presented)
    let length = s.chars().take_while(|c| c.is_ascii_hexdigit()).count();
    let integer = u64::from_str_radix(&s[..length], 16).unwrap_or(0);
    s = &s[length..];

    // Read a fractional part (if presented)
    let fractional = if s.starts_with('.') {
        s = &s[1..];
        let length = s.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        let value = parse_fractional_part(&s[..length])?;
        s = &s[length..];
        Some(value)
    } else {
        None
    };

    // Read a power (if presented)
    let power = if s.starts_with('p') || s.starts_with('P') {
        s = &s[1..];
        let length = s
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '-' || *c == '+')
            .count();
        let value = s[..length].parse().map_err(|_| ParseNumberError::Float)?;
        s = &s[length..];
        Some(value)
    } else {
        None
    };

    // Post checks:
    // - Both Fractions & Power values can't be none in the same time
    // - string should be consumed. Otherwise, it's possible to have garbage symbols after the HEX
    // float
    if fractional.is_none() && power.is_none() {
        return Err(ParseNumberError::Float);
    }

    if !s.is_empty() {
        return Err(ParseNumberError::Float);
    }

    // Build the result
    let total =
        sign * (integer as f64 + fractional.unwrap_or(0.0)) * (2.0_f64).powi(power.unwrap_or(0));
    Ok(total)
}

fn parse_fractional_part(s: &str) -> Result<f64, ParseNumberError> {
    if s.is_empty() {
        return Err(ParseNumberError::Float);
    }

    let mut multiplier = 1.0 / 16.0;
    let mut total = 0.0;
    for c in s.chars() {
        let digit = c
            .to_digit(16)
            .map(|x| x as u8)
            .ok_or(ParseNumberError::Float)?;
        total += (digit as f64) * multiplier;
        multiplier /= 16.0;
    }
    Ok(total)
}

fn float_to_scaled_integer(input: f64, precision: Option<f64>) -> (i64, i64) {
    let mut scaled_value = input;
    let mut scale = 0;
    let mut multiplier = 10.0;
    let precision = precision.unwrap_or(0.000001);
    loop {
        let rounded_value = scaled_value.round();
        if f64::abs(rounded_value - scaled_value) <= precision {
            return (rounded_value as i64, -scale);
        }
        scale += 1;
        // 'scaled_value *= 10.0' is efficient but less precise, due to accumulating rounding errors over iterations.
        // 'scaled_value = input * 10.0_f64.powi(scale)' is more precise, but it's less efficient due to recalculating the power each time.
        // 'scaled_value = input * multiplier' - seems like a good balance between calculation errors and efficiency.
        scaled_value = input * multiplier;
        multiplier *= 10.0;
    }
}

#[cfg(test)]
mod tests {
    use super::{float_to_scaled_integer, parse_float, parse_hexadecimal_float};
    use crate::ExtendedBigDecimal;
    use num_traits::ToPrimitive;
    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_parse_float_from_invalid_values() {
        let samples = vec![
            "1", "1p", "0x1", "0x1.", "0x1p", "0x1p+", "-0xx1p1", "0x1.k", "0x1", "-0x1pa",
            "0x1.1pk", "0x1.8p2z", "0x1p3.2",
        ];

        for s in samples {
            assert_eq!(parse_float(s).is_err(), true);
        }
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_parse_float_from_valid_values() {
        let samples = vec![
            ("0x1p1", 2.0),
            ("+0x1p1", 2.0),
            ("-0x1p1", -2.0),
            ("0x1p-1", 0.5),
            ("0x1.8", 1.5),
            ("-0x1.8", -1.5),
            ("0x1.8p2", 6.0),
            ("0x1.8p+2", 6.0),
            ("0x1.8p-2", 0.375),
            ("0x.8", 0.5),
            ("0x10p0", 16.0),
            ("0x0.0", 0.0),
            ("0x0p0", 0.0),
            ("0x0.0p0", 0.0),
        ];

        for (sample, control_value) in samples {
            let value = parse_float(sample).unwrap();
            assert_eq!(value, control_value);
        }
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_parse_precise_number_from_valid_values() {
        let samples = vec![
            ("0x1p1", 2.0),
            ("+0x1p1", 2.0),
            ("-0x1p1", -2.0),
            ("0x1p-1", 0.5),
            ("0x1.8", 1.5),
            ("-0x1.8", -1.5),
            ("0x1.8p2", 6.0),
            ("0x1.8p+2", 6.0),
            ("0x1.8p-2", 0.375),
            ("0x.8", 0.5),
            ("0x10p0", 16.0),
            ("0x0.0", 0.0),
            ("0x0p0", 0.0),
            ("0x0.0p0", 0.0),
        ];

        for (s, v) in samples {
            match parse_hexadecimal_float(s).unwrap().number {
                ExtendedBigDecimal::BigDecimal(bd) => assert_eq!(bd.to_f64().unwrap(), v),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    #[allow(clippy::cognitive_complexity)]
    fn test_float_to_scaled_integers() {
        let samples = [
            (0.0, (0, 0)),
            (0.5, (5, -1)),
            (1.5, (15, -1)),
            (-0.5, (-5, -1)),
            (-1.5, (-15, -1)),
            (0.375, (375, -3)),
            (1.375, (1375, -3)),
            (1.375, (1375, -3)),
            (1.372, (1372, -3)),
            (1.378, (1378, -3)),
            (1.0, (1, 0)),
            (10.0, (10, 0)),
            (123.12345678, (12312345678, -8)),
            (-123.12345678, (-12312345678, -8)),
            (-334.22923, (-33422923, -5)),
            (1000.00001, (100000001, -5)),
            (4334.123456788, (4334123456788, -9)),
            (123456789.123456789, (1234567891234568, -7)), // truncated value
            (-123456789.123456789, (-1234567891234568, -7)), // truncated value
        ];

        for (input, (control_value, control_scale)) in samples {
            let (value, scale) = float_to_scaled_integer(input, None);
            assert_eq!(value, control_value);
            assert_eq!(scale, control_scale);
        }
    }
}
