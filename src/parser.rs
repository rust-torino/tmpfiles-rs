use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fs::Permissions;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;

use btoi::{btoi, btoi_radix};

use nom::branch::alt;
use nom::bytes::complete::take_till;
use nom::character::complete::{char, digit1, oct_digit1, one_of, space1};
use nom::combinator::{map, map_res, opt};
use nom::sequence::tuple;
use nom::IResult;

// NOTE: taken from systemd source code
// https://github.com/systemd/systemd/blob/3a712fda86ea7d7dc1082b1332f9e94d19c0739a/src/tmpfiles/tmpfiles.c#L73
const VALID_ITEM_TYPES: &str = "fFdDvqQpLcbCwetTaAhHxXrRzZm";

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq)]
pub enum ItemTypes {
    _CREATE_DIRECTORY,
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
pub struct Action<'a> {
    action_type: ItemTypes,
    path: &'a OsStr,
    mode: Option<Mode>,
    user: Option<User<'a>>,
    group: &'a str,
    age: &'a str,
    argument: &'a str,
    boot_only: bool,
    append_or_force: bool,
    allow_failure: bool,
}

impl TryFrom<char> for ItemTypes {
    type Error = &'static str;

    fn try_from(type_char: char) -> Result<Self, Self::Error> {
        match type_char {
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

fn mode(input: &[u8]) -> IResult<&[u8], Option<Mode>> {
    alt((
        map(empty_placeholder, |_| None),
        map(non_empty_mode, |mode| Some(mode)),
    ))(input)
}

fn non_empty_mode(input: &[u8]) -> IResult<&[u8], Mode> {
    let (input, masked) = opt(char('~'))(input)?;
    let (input, mode_digits) = oct_digit1(input)?;

    let octal_permission = btoi_radix(mode_digits, 8).unwrap();

    Ok((
        input,
        Mode {
            masked: masked.is_some(),
            mode: Permissions::from_mode(octal_permission),
        },
    ))
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
                map(user_or_group_id, |uid| User::ID(uid)),
                map(user_or_group_name, |username| User::Name(username)),
            )),
            |user| Some(user),
        ),
    ))(input)
}

fn parse_line(input: &[u8]) -> IResult<&[u8], Action> {
    let (input, (action_type, boot_only, append_or_force, allow_failure)) = item_type(input)?;
    let (input, path_os_str) = path(input)?;
    let (input, _) = space1(input)?;
    let (input, mode) = mode(input)?;
    let (input, _) = space1(input)?;
    let (input, user) = user(input)?;


    Ok((
        input,
        Action {
            action_type,
            path: path_os_str,
            mode: mode,
            user: user,
            group: "daemon",
            age: "-",
            argument: "-",
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
                group: "daemon",
                age: "-",
                argument: "-",
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
                group: "daemon",
                age: "-",
                argument: "-",
                boot_only: false,
                append_or_force: false,
                allow_failure: false,
            },
            parse_line(b"f     /tmp/z/f    0755 daemon daemon - -")
                .unwrap()
                .1
        );
    }
}
