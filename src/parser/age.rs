use btoi::btoi;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{char, digit1, space1};
use nom::combinator::{opt, peek};
use nom::sequence::terminated;
use nom::IResult;


use crate::common::action::CleanupAge;
use super::basic::empty_placeholder;

// Time unit multipliers
const USEC_PER_MSEC: u64 = 1_000u64;
const USEC_PER_SEC: u64 = 1_000 * USEC_PER_MSEC;
const USEC_PER_MIN: u64 = 60 * USEC_PER_SEC;
const USEC_PER_HOUR: u64 = 60 * USEC_PER_MIN;
const USEC_PER_DAY: u64 = 24 * USEC_PER_HOUR;
const USEC_PER_WEEK: u64 = 7 * USEC_PER_DAY;


fn weeks(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, weeks) = opt(terminated(digit1, alt((tag("w"), tag("weeks")))))(input)?;
    match weeks {
        Some(w) => Ok((i, btoi::<u64>(w).unwrap() * USEC_PER_WEEK)),
        None => Ok((i, 0)),
    }
}

fn days(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, days) = opt(terminated(digit1, alt((tag("d"), tag("days")))))(input)?;
    match days {
        Some(d) => Ok((i, btoi::<u64>(d).unwrap() * USEC_PER_DAY)),
        None => Ok((i, 0)),
    }
}

fn hours(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, hours) = opt(terminated(digit1, alt((tag("h"), tag("hours")))))(input)?;
    match hours {
        Some(h) => Ok((i, btoi::<u64>(h).unwrap() * USEC_PER_HOUR)),
        None => Ok((i, 0)),
    }
}

fn minutes(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, minutes) = opt(terminated(
        digit1,
        alt((tag("m"), tag("min"), tag("minutes"))),
    ))(input)?;
    match minutes {
        Some(m) => Ok((i, btoi::<u64>(m).unwrap() * USEC_PER_MIN)),
        None => Ok((i, 0)),
    }
}

fn seconds(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, seconds) = opt(terminated(digit1, alt((tag("s"), tag("seconds")))))(input)?;
    match seconds {
        Some(s) => Ok((i, btoi::<u64>(s).unwrap() * USEC_PER_SEC)),
        None => Ok((i, 0)),
    }
}

fn milli_seconds(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, milli_seconds) = opt(terminated(digit1, alt((tag("ms"), tag("milliseconds")))))(input)?;
    match milli_seconds {
        Some(ms) => Ok((i, btoi::<u64>(ms).unwrap() * USEC_PER_MSEC)),
        None => Ok((i, 0)),
    }
}

fn micro_seconds(input: &[u8]) -> IResult<&[u8], u64> {
    let (i, micro_seconds) = opt(terminated(digit1, alt((tag("ms"), tag("microseconds")))))(input)?;
    match micro_seconds {
        Some(us) => Ok((i, btoi::<u64>(us).unwrap())),
        None => Ok((i, 0)),
    }
}

fn age_with_unit(input: &[u8]) -> IResult<&[u8], u64> {
    let (input, weeks) = weeks(input)?;
    let (input, days) = days(input)?;
    let (input, hours) = hours(input)?;
    let (input, minutes) = minutes(input)?;
    let (input, seconds) = seconds(input)?;
    let (input, milli_seconds) = milli_seconds(input)?;
    let (input, micro_seconds) = micro_seconds(input)?;

    let age_components = [
        weeks,
        days,
        hours,
        minutes,
        seconds,
        milli_seconds,
        micro_seconds,
    ];
    let age = age_components.iter().sum();
    Ok((input, age))
}

fn age_without_unit(input: &[u8]) -> IResult<&[u8], u64> {
    let (input, digits) = digit1(input)?;
    let (input, _) = peek(space1)(input)?;
    Ok((input, btoi::<u64>(digits).unwrap() * USEC_PER_SEC))
}

pub fn age(input: &[u8]) -> IResult<&[u8], Option<CleanupAge>> {
    let (input, keep_first_level) = opt(char('~'))(input)?;
    let (input, omitted) = opt(empty_placeholder)(input)?;
    if omitted.is_some() {
        return Ok((input, None));
    }

    let (input, age) = alt((
        // If an integer is given without a unit, s is assumed.
        age_without_unit,
        // otherwise a series of integers each followed by one time unit
        age_with_unit,
    ))(input)?;

    Ok((input, Some(CleanupAge::new(age, keep_first_level.is_some()))))
}


#[cfg(test)]
mod test {

    use super::*;
    use crate::common::action::CleanupAge;

    #[test]
    fn test_age() {
        assert_eq!(None, age(b"-").unwrap().1,);

        assert_eq!(
            Some(CleanupAge::new(5_000_000, false)),
            age(b"5s").unwrap().1,
        );

        assert_eq!(
            Some(CleanupAge::new(60_000_000, false)),
            age(b"1m").unwrap().1,
        );

        assert_eq!(
            Some(CleanupAge::new(110_000_000, false)),
            age(b"1m50s").unwrap().1,
        );

        assert_eq!(
            Some(CleanupAge::new(60_000_000, false)),
            age(b"60 ").unwrap().1,
        );
    }
}