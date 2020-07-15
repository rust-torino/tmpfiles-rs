use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fs::Permissions;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;

use btoi::{btoi, btoi_radix};

use nom::branch::alt;
use nom::bytes::complete::{tag, take_till, take_while_m_n};
use nom::character::complete::{char, digit1, one_of, space1};
use nom::character::is_oct_digit;
use nom::combinator::{all_consuming, map, map_res, opt, peek, rest};
use nom::sequence::{preceded, terminated, tuple};
use nom::IResult;

// NOTE: taken from systemd source code
// https://github.com/systemd/systemd/blob/3a712fda86ea7d7dc1082b1332f9e94d19c0739a/src/tmpfiles/tmpfiles.c#L73
const VALID_ITEM_TYPES: &str = "fFdDvqQpLcbCwetTaAhHxXrRzZm";

// Time unit multipliers
const USEC_PER_MSEC: u64 = 1_000u64;
const USEC_PER_SEC: u64 = 1_000 * USEC_PER_MSEC;
const USEC_PER_MIN: u64 = 60 * USEC_PER_SEC;
const USEC_PER_HOUR: u64 = 60 * USEC_PER_MIN;
const USEC_PER_DAY: u64 = 24 * USEC_PER_HOUR;
const USEC_PER_WEEK: u64 = 7 * USEC_PER_DAY;

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq)]
pub enum ItemTypes {
    CREATE_DIRECTORY,
    _CREATE_SUBVOLUME,
    _CREATE_SUBVOLUME_INHERIT_QUOTA,
    _CREATE_SUBVOLUME_NEW_QUOTA,
    _EMPTY_DIRECTORY,
    _TRUNCATE_DIRECTORY,
    _CREATE_FIFO,
    _IGNORE_PATH,
    _IGNORE_DIRECTORY_PATH,
    _REMOVE_PATH,
    _RECURSIVE_REMOVE_PATH,
    _ADJUST_MODE,
    RELABEL_PATH,
    _RECURSIVE_RELABEL_PATH,
    CREATE_FILE,
    _TRUNCATE_FILE,
    // TODO: adds missing cases
}

#[derive(Debug, PartialEq)]
pub struct Mode {
    masked: bool,
    mode: Permissions,
}

#[derive(Debug, PartialEq)]
pub enum User<'a> {
    Name(&'a OsStr),
    ID(u32),
}

#[derive(Debug, PartialEq)]
pub enum Group<'a> {
    Name(&'a OsStr),
    ID(u32),
}

#[derive(Debug, PartialEq)]
pub struct CleanupAge {
    age: u64,
    keep_first_level: bool,
}

#[derive(Debug, PartialEq)]
pub struct Action<'a> {
    action_type: ItemTypes,
    path: &'a OsStr,
    mode: Option<Mode>,
    user: Option<User<'a>>,
    group: Option<Group<'a>>,
    age: Option<CleanupAge>,
    argument: Option<&'a OsStr>,
    boot_only: bool,
    append_or_force: bool,
    allow_failure: bool,
}

impl TryFrom<char> for ItemTypes {
    type Error = &'static str;

    fn try_from(type_char: char) -> Result<Self, Self::Error> {
        match type_char {
            'd' => Ok(ItemTypes::CREATE_DIRECTORY),
            'f' => Ok(ItemTypes::CREATE_FILE),
            'z' => Ok(ItemTypes::RELABEL_PATH),
            other => todo!("Support for type `{}` is missing", other),
        }
    }
}

fn item_type_char(input: &[u8]) -> IResult<&[u8], ItemTypes> {
    map_res(one_of(VALID_ITEM_TYPES), ItemTypes::try_from)(input)
}

fn item_type(input: &[u8]) -> IResult<&[u8], (ItemTypes, bool, bool, bool)> {
    let (input, (i_type, boot_only, append_or_force, allow_failure)) = tuple((
        item_type_char,
        opt(char('!')),
        opt(char('+')),
        opt(char('-')),
    ))(input)?;
    let (input, _) = space1(input)?;

    Ok((
        input,
        (
            i_type,
            boot_only.is_some(),
            append_or_force.is_some(),
            allow_failure.is_some(),
        ),
    ))
}

fn empty_placeholder(input: &[u8]) -> IResult<&[u8], char> {
    char('-')(input)
}

fn path(input: &[u8]) -> IResult<&[u8], &OsStr> {
    // NOTE as per this comment in "opentmpfiles" we keep the parsing simple
    // > Upstream says whitespace is NOT permitted in the Path argument.
    //https://github.com/OpenRC/opentmpfiles/blob/09a1675f68d8106ba08acfc72a263843dabdb588/tmpfiles.sh#L505
    let (input, path_bytes) = take_till(|c| (c as char).is_whitespace())(input)?;
    Ok((input, OsStr::from_bytes(path_bytes)))
}

fn non_empty_mode(input: &[u8]) -> IResult<&[u8], Mode> {
    let (input, masked) = opt(char('~'))(input)?;
    let (input, mode_digits) = preceded(
        opt(tag(b"0")),
        alt((
            take_while_m_n(4, 4, is_oct_digit),
            take_while_m_n(3, 3, is_oct_digit),
        )),
    )(input)?;

    let octal_permission = btoi_radix(mode_digits, 8).unwrap();
    Ok((
        input,
        Mode {
            masked: masked.is_some(),
            mode: Permissions::from_mode(octal_permission),
        },
    ))
}

fn mode(input: &[u8]) -> IResult<&[u8], Option<Mode>> {
    alt((map(empty_placeholder, |_| None), map(non_empty_mode, Some)))(input)
}

fn user_or_group_id(input: &[u8]) -> IResult<&[u8], u32> {
    map(digit1, |uid| btoi(uid).unwrap())(input)
}

fn user_or_group_name(input: &[u8]) -> IResult<&[u8], &OsStr> {
    // TODO the user/group name parsing is kept simple until
    // we have a better specification.
    let (input, chars) = take_till(|c| (c as char).is_whitespace())(input)?;
    Ok((input, OsStr::from_bytes(chars)))
}

fn user(input: &[u8]) -> IResult<&[u8], Option<User>> {
    alt((
        map(empty_placeholder, |_| None),
        map(
            alt((
                map(user_or_group_id, User::ID),
                map(user_or_group_name, User::Name),
            )),
            Some,
        ),
    ))(input)
}

fn group(input: &[u8]) -> IResult<&[u8], Option<Group>> {
    alt((
        map(empty_placeholder, |_| None),
        map(
            alt((
                map(user_or_group_id, Group::ID),
                map(user_or_group_name, Group::Name),
            )),
            Some,
        ),
    ))(input)
}

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

fn age(input: &[u8]) -> IResult<&[u8], Option<CleanupAge>> {
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

    Ok((
        input,
        Some(CleanupAge {
            keep_first_level: keep_first_level.is_some(),
            age,
        }),
    ))
}

#[rustfmt::skip]
fn argument(input: &[u8]) -> IResult<&[u8], Option<&OsStr>> {
    alt((
        map(
            all_consuming( alt(( tag("-"), tag("") )) ),
            |_| None
        ),
        map(
            rest,
            |arg| Some(OsStr::from_bytes(arg))
        )
    ))(input)
}

fn parse_line(input: &[u8]) -> IResult<&[u8], Action> {
    let (input, (action_type, boot_only, append_or_force, allow_failure)) = item_type(input)?;
    let (input, path_os_str) = path(input)?;
    let (input, _) = space1(input)?;
    let (input, mode) = mode(input)?;
    let (input, _) = space1(input)?;
    let (input, user) = user(input)?;
    let (input, _) = space1(input)?;
    let (input, group) = group(input)?;
    let (input, _) = space1(input)?;
    let (input, age) = age(input)?;
    let (input, _) = space1(input)?;
    let (input, argument) = argument(input)?;

    Ok((
        input,
        Action {
            action_type,
            path: path_os_str,
            mode,
            user,
            group,
            age,
            argument,
            boot_only,
            append_or_force,
            allow_failure,
        },
    ))
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_parse_item_type() {
        assert_eq!(
            (ItemTypes::RELABEL_PATH, false, false, false),
            item_type(b"z ").unwrap().1
        );

        assert_eq!(
            (ItemTypes::RELABEL_PATH, true, false, false),
            item_type(b"z! ").unwrap().1
        );

        assert_eq!(
            (ItemTypes::RELABEL_PATH, false, true, false),
            item_type(b"z+ ").unwrap().1
        );

        assert_eq!(
            (ItemTypes::RELABEL_PATH, false, false, true),
            item_type(b"z- ").unwrap().1
        );
    }

    #[test]
    fn test_parse_item_type_error() {
        assert!(item_type(b"y ").is_err());
        assert!(item_type(b"foobar").is_err());
    }

    #[test]
    fn test_mode() {
        assert_eq!(
            Some(Mode {
                masked: false,
                mode: Permissions::from_mode(0o644)
            }),
            mode(b"0644").unwrap().1
        );

        assert_eq!(
            Some(Mode {
                masked: false,
                mode: Permissions::from_mode(0o4755)
            }),
            mode(b"04755").unwrap().1
        );

        assert_eq!(
            Some(Mode {
                masked: true,
                mode: Permissions::from_mode(0o444)
            }),
            mode(b"~0444").unwrap().1
        );

        assert_eq!(None, mode(b"-").unwrap().1);
    }

    #[test]
    fn test_user() {
        assert_eq!(Some(User::ID(0)), user(b"0").unwrap().1);
        assert_eq!(Some(User::ID(42)), user(b"42").unwrap().1);
        assert_eq!(
            Some(User::Name(OsStr::new("root"))),
            user(b"root").unwrap().1
        );
        assert_eq!(
            Some(User::Name(OsStr::new("nobody"))),
            user(b"nobody").unwrap().1
        );
        assert_eq!(None, user(b"-").unwrap().1);
    }

    #[test]
    fn test_group() {
        assert_eq!(Some(Group::ID(0)), group(b"0").unwrap().1);
        assert_eq!(Some(Group::ID(42)), group(b"42").unwrap().1);
        assert_eq!(
            Some(Group::Name(OsStr::new("root"))),
            group(b"root").unwrap().1
        );
        assert_eq!(
            Some(Group::Name(OsStr::new("nogroup"))),
            group(b"nogroup").unwrap().1
        );
        assert_eq!(None, group(b"-").unwrap().1);
    }

    #[test]
    fn test_age() {
        assert_eq!(None, age(b"-").unwrap().1,);

        assert_eq!(
            Some(CleanupAge {
                age: 5_000_000,
                keep_first_level: false
            }),
            age(b"5s").unwrap().1,
        );

        assert_eq!(
            Some(CleanupAge {
                age: 60_000_000,
                keep_first_level: false
            }),
            age(b"1m").unwrap().1,
        );

        assert_eq!(
            Some(CleanupAge {
                age: 110_000_000,
                keep_first_level: false
            }),
            age(b"1m50s").unwrap().1,
        );

        assert_eq!(
            Some(CleanupAge {
                age: 60_000_000,
                keep_first_level: false
            }),
            age(b"60 ").unwrap().1,
        );
    }

    #[test]
    fn test_argument() {
        assert_eq!(None, argument(b"-").unwrap().1);

        assert_eq!(None, argument(b"").unwrap().1);

        assert_eq!(
            Some(OsStr::new(
                "Egg and bacon\n Egg, sausage and bacon\nEgg and Spam"
            )),
            argument(b"Egg and bacon\n Egg, sausage and bacon\nEgg and Spam")
                .unwrap()
                .1
        );
    }

    #[test]
    fn test_parse_line() {
        assert_eq!(
            Action {
                action_type: ItemTypes::RELABEL_PATH,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode {
                    masked: false,
                    mode: Permissions::from_mode(0o755)
                }),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                age: None,
                argument: None,
                boot_only: false,
                append_or_force: false,
                allow_failure: false,
            },
            parse_line(b"z     /tmp/z/f    0755 daemon daemon - -")
                .unwrap()
                .1
        );

        assert_eq!(
            Action {
                action_type: ItemTypes::CREATE_FILE,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode {
                    masked: false,
                    mode: Permissions::from_mode(0o755)
                }),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                age: None,
                argument: None,
                boot_only: false,
                append_or_force: false,
                allow_failure: false,
            },
            parse_line(b"f     /tmp/z/f    0755 daemon daemon - -")
                .unwrap()
                .1
        );

        assert_eq!(
            Action {
                action_type: ItemTypes::CREATE_DIRECTORY,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode {
                    masked: false,
                    mode: Permissions::from_mode(0o755)
                }),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                age: Some(CleanupAge {
                    age: 97_200_000_000,
                    keep_first_level: false
                }),
                argument: None,
                boot_only: false,
                append_or_force: false,
                allow_failure: false,
            },
            parse_line(b"d     /tmp/z/f    0755 daemon daemon 1d3h -")
                .unwrap()
                .1
        );

        assert_eq!(
            Action {
                action_type: ItemTypes::CREATE_DIRECTORY,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode {
                    masked: false,
                    mode: Permissions::from_mode(0o755)
                }),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                age: None,
                argument: Some(OsStr::new("/tmp/C/1-origin")),
                boot_only: false,
                append_or_force: false,
                allow_failure: false,
            },
            parse_line(b"d  /tmp/z/f    0755 daemon daemon - /tmp/C/1-origin")
                .unwrap()
                .1
        );
    }
}
