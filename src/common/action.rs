use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;


#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq)]
pub enum ItemTypes {
    CREATE_FILE,
    // TRUNCATE_FILE, DEPRECATED
    CREATE_DIRECTORY,
    TRUNCATE_DIRECTORY,
    CREATE_SUBVOLUME,
    CREATE_SUBVOLUME_INHERIT_QUOTA,
    CREATE_SUBVOLUME_NEW_QUOTA,
    CREATE_FIFO,
    CREATE_SYMLINK,
    CREATE_BLOCK_DEVICE,
    CREATE_CHAR_DEVICE,
    COPY_FILES,

    WRITE_FILE,
    EMPTY_DIRECTORY,
    SET_XATTR,
    RECURSIVE_SET_XATTR,
    SET_ACL,
    RECURSIVE_SET_ACL,
    SET_ATTRIBUTE,
    RECURSIVE_SET_ATTRIBUTE,
    IGNORE_PATH,
    IGNORE_DIRECTORY_PATH,
    REMOVE_PATH,
    RECURSIVE_REMOVE_PATH,
    // ADJUST_MODE, legacy same as RELABEL_PATH
    RELABEL_PATH,
    RECURSIVE_RELABEL_PATH,
}

impl TryFrom<char> for ItemTypes {
    type Error = String;

    fn try_from(type_char: char) -> Result<Self, Self::Error> {
        match type_char {
            'f' => Ok(ItemTypes::CREATE_FILE),
            'd' => Ok(ItemTypes::CREATE_DIRECTORY),
            'D' => Ok(ItemTypes::TRUNCATE_DIRECTORY),
            'v' => Ok(ItemTypes::CREATE_SUBVOLUME),
            'q' => Ok(ItemTypes::CREATE_SUBVOLUME_INHERIT_QUOTA),
            'Q' => Ok(ItemTypes::CREATE_SUBVOLUME_NEW_QUOTA),
            'p' => Ok(ItemTypes::CREATE_FIFO),
            'L' => Ok(ItemTypes::CREATE_SYMLINK),
            'b' => Ok(ItemTypes::CREATE_BLOCK_DEVICE),
            'c' => Ok(ItemTypes::CREATE_CHAR_DEVICE),
            'C' => Ok(ItemTypes::COPY_FILES),

            'w' => Ok(ItemTypes::WRITE_FILE),
            'e' => Ok(ItemTypes::EMPTY_DIRECTORY),
            't' => Ok(ItemTypes::SET_XATTR),
            'T' => Ok(ItemTypes::RECURSIVE_SET_XATTR),
            'a' => Ok(ItemTypes::SET_ACL),
            'A' => Ok(ItemTypes::RECURSIVE_SET_ACL),
            'h' => Ok(ItemTypes::SET_ATTRIBUTE),
            'H' => Ok(ItemTypes::RECURSIVE_SET_ATTRIBUTE),
            'x' => Ok(ItemTypes::IGNORE_PATH),
            'X' => Ok(ItemTypes::IGNORE_DIRECTORY_PATH),
            'r' => Ok(ItemTypes::REMOVE_PATH),
            'R' => Ok(ItemTypes::RECURSIVE_REMOVE_PATH),
            'z' => Ok(ItemTypes::RELABEL_PATH),
            'Z' => Ok(ItemTypes::RECURSIVE_RELABEL_PATH),
            invalid => Err(format!("Invalid item type: '{}'", invalid)),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct Mode {
    pub masked: bool,
    pub mode: Permissions,
}

impl Mode {
    pub fn new(masked: bool, mode: u32) -> Self {
        Mode { masked, mode: Permissions::from_mode(mode) }
    }

    pub fn default_for_file() -> Self {
        Self::new(false, 0o644)
    }

    pub fn default_for_folder() -> Self {
        Self::new(false, 0o755)
    }
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
    pub age: u64,
    pub keep_first_level: bool,
}

impl CleanupAge {
    pub fn new(age: u64, keep_first_level: bool) -> Self {
        CleanupAge { age, keep_first_level }
    }
}

#[derive(Debug, PartialEq)]
pub struct Action<'a> {
    pub action_type: ItemTypes,
    pub path: &'a OsStr,
    pub mode: Option<Mode>,
    pub user: Option<User<'a>>,
    pub group: Option<Group<'a>>,
    pub age: Option<CleanupAge>,
    pub argument: Option<&'a OsStr>,
    pub boot_only: bool,
    pub append_or_force: bool,
    pub allow_failure: bool,
}

impl <'a> Default for Action<'a> {
    fn default() -> Self {
        Action {
            action_type: ItemTypes::CREATE_DIRECTORY,
            path: OsStr::new(""),
            mode: None,
            user: None,
            group: None,
            age: None,
            argument: None,
            boot_only: false,
            append_or_force: false,
            allow_failure: false,
        }
    }
}


