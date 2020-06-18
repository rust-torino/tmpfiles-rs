use nom::IResult;

#[derive(Debug, PartialEq)]
pub struct Action<'a> {
    action_type: &'a str,
    path: &'a str,
    mode: &'a str,
    user: &'a str,
    group: &'a str,
    age: &'a str,
    argument: &'a str,
    boot_only: bool,
}


pub enum ActionType {
    CREATE_DIRECTORY,
    CREATE_SUBVOLUME,
    CREATE_SUBVOLUME_INHERIT_QUOTA,
    CREATE_SUBVOLUME_NEW_QUOTA,
    EMPTY_DIRECTORY,
    TRUNCATE_DIRECTORY,
    CREATE_FIFO,
    IGNORE_PATH,
    IGNORE_DIRECTORY_PATH,
    REMOVE_PATH,
    RECURSIVE_REMOVE_PATH,
    ADJUST_MODE,
    RELABEL_PATH,
    RECURSIVE_RELABEL_PATH,
    CREATE_FILE,
    TRUNCATE_FILE,
    // TODO: adds missing cases
}


fn parse_line(line: &[u8]) -> IResult<&[u8], Action> {

    Ok((line, Action {
        action_type: "z",
        path: "/tmp/z/f",
        mode: "0755",
        user: "daemon",
        group: "daemon",
        age: "-",
        argument: "-",
        boot_only: false,
    }))
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_parse_line() {

        let input = "z     /tmp/z/f1    0755 daemon daemon - -";
        let res = parse_line(input.as_bytes());
        
        let expected = Action {
            action_type: "z",
            path: "/tmp/z/f",
            mode: "0755",
            user: "daemon",
            group: "daemon",
            age: "-",
            argument: "-",
            boot_only: false,
        };
        assert_eq!(expected, res.unwrap().1);
        
    }

}