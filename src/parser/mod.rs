use std::convert::TryFrom;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;

use btoi::{btoi, btoi_radix};

use nom::branch::alt;
use nom::bytes::complete::{tag, take_till, take_while_m_n};
use nom::character::complete::{char, digit1, one_of, space1};
use nom::character::is_oct_digit;
use nom::combinator::{all_consuming, map, map_res, opt, rest};
use nom::sequence::{preceded, tuple};
use nom::IResult;

use crate::common::action::{ItemTypes, Mode, User, Group, Action};

mod basic;
mod age;
use basic::empty_placeholder;
use age::age;


// NOTE: taken from systemd source code
// https://github.com/systemd/systemd/blob/3a712fda86ea7d7dc1082b1332f9e94d19c0739a/src/tmpfiles/tmpfiles.c#L73
const VALID_ITEM_TYPES: &str = "fFdDvqQpLcbCwetTaAhHxXrRzZm";

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
    Ok((input, Mode::new(masked.is_some(), octal_permission)))
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

pub fn parse_line(input: &[u8]) -> IResult<&[u8], Action> {
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
    use crate::common::action::CleanupAge;

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
            Some(Mode::new(false, 0o644)),
            mode(b"0644").unwrap().1
        );

        assert_eq!(
            Some(Mode::new(false, 0o4755)),
            mode(b"04755").unwrap().1
        );

        assert_eq!(
            Some(Mode::new(true, 0o444)),
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
                mode: Some(Mode::new(false, 0o755)),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                .. Action::default()
            },
            parse_line(b"z     /tmp/z/f    0755 daemon daemon - -")
                .unwrap()
                .1
        );

        assert_eq!(
            Action {
                action_type: ItemTypes::CREATE_FILE,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode::new(false, 0o755)),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                .. Action::default()
            },
            parse_line(b"f     /tmp/z/f    0755 daemon daemon - -")
                .unwrap()
                .1
        );

        assert_eq!(
            Action {
                action_type: ItemTypes::CREATE_DIRECTORY,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode::new(false, 0o755)),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                age: Some(CleanupAge {
                    age: 97_200_000_000,
                    keep_first_level: false
                }),
                .. Action::default()
            },
            parse_line(b"d     /tmp/z/f    0755 daemon daemon 1d3h -")
                .unwrap()
                .1
        );

        assert_eq!(
            Action {
                action_type: ItemTypes::CREATE_DIRECTORY,
                path: &OsStr::new("/tmp/z/f"),
                mode: Some(Mode::new(false, 0o755)),
                user: Some(User::Name(OsStr::new("daemon"))),
                group: Some(Group::Name(OsStr::new("daemon"))),
                argument: Some(OsStr::new("/tmp/C/1-origin")),
                .. Action::default()
            },
            parse_line(b"d  /tmp/z/f    0755 daemon daemon - /tmp/C/1-origin")
                .unwrap()
                .1
        );
    }
}
