use std::convert::TryFrom;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

use nom::bytes::complete::take_till;
use nom::character::complete::{char, one_of, space1};
use nom::combinator::{map_res, opt};
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
pub struct Action<'a> {
    action_type: ItemTypes,
    path: &'a OsStr,
    mode: &'a str,
    user: &'a str,
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
            _ => Err("Not implemented"),
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

fn path(input: &[u8]) -> IResult<&[u8], &OsStr> {
    // NOTE as per this comment in "opentmpfiles" we keep the parsing simple
    // > Upstream says whitespace is NOT permitted in the Path argument.
    //https://github.com/OpenRC/opentmpfiles/blob/09a1675f68d8106ba08acfc72a263843dabdb588/tmpfiles.sh#L505
    let (input, path_bytes) = take_till(|c| (c as char).is_whitespace())(input)?;
    Ok((input, OsStr::from_bytes(path_bytes)))
}

fn parse_line(input: &[u8]) -> IResult<&[u8], Action> {
    let (input, (action_type, boot_only, append_or_force, allow_failure)) = item_type(input)?;
    let (input, path_os_str) = path(input)?;

    Ok((
        input,
        Action {
            action_type,
            path: path_os_str,
            mode: "0755",
            user: "daemon",
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
    fn test_parse_line() {
        assert_eq!(
            Action {
                action_type: ItemTypes::RELABEL_PATH,
                path: &OsStr::new("/tmp/z/f"),
                mode: "0755",
                user: "daemon",
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
                mode: "0755",
                user: "daemon",
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
