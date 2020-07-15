use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fs::Permissions;
use std::os::unix::fs::PermissionsExt;


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


