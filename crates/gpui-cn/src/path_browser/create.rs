//! Making a folder from a picker: the typed error a source answers with, and
//! the one check every name passes before a source is asked.

use gpui_kit::SharedString;

use super::PathStyle;

/// The longest folder name the picker passes on, in characters.
const MAX_NAME: usize = 255;

/// Why a source could not make a folder. The picker shows the message under
/// the row that names the folder, and keeps the row editable.
///
/// A source returns one from `create_folder`. A name that is empty, has a
/// separator, or is `.` or `..` is refused by the picker before the source
/// is asked, with [`InvalidName`](Self::InvalidName).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CreateFolderError {
    /// The source cannot make folders.
    Unsupported,
    /// A folder or file with that name is already there.
    Exists,
    /// The name cannot be a folder name. The text says why, and is shown
    /// to the user.
    InvalidName(SharedString),
    /// The user may not make folders there.
    PermissionDenied,
    /// Any other failure. The text is shown to the user.
    Other(SharedString),
}

impl CreateFolderError {
    /// The text the picker shows for the folder called `name`.
    pub fn message(&self, name: &str) -> SharedString {
        match self {
            Self::Unsupported => "This location cannot make folders".into(),
            Self::Exists => format!("A folder named {name} already exists").into(),
            Self::PermissionDenied => "You cannot make folders here".into(),
            Self::InvalidName(reason) | Self::Other(reason) => reason.clone(),
        }
    }
}

impl From<std::io::Error> for CreateFolderError {
    fn from(error: std::io::Error) -> Self {
        use std::io::ErrorKind;
        match error.kind() {
            ErrorKind::AlreadyExists => Self::Exists,
            ErrorKind::PermissionDenied => Self::PermissionDenied,
            ErrorKind::InvalidInput | ErrorKind::InvalidFilename => {
                Self::InvalidName("That name is not allowed".into())
            }
            _ => Self::Other(error.to_string().into()),
        }
    }
}

/// The name a folder would get, or why it cannot: surrounding spaces go,
/// and what is left must be there, short, free of separators, nulls and
/// control characters, not `.` or `..`, and in a Windows style without the
/// characters Windows refuses.
pub(crate) fn validate_folder_name(
    name: &str,
    style: &PathStyle,
) -> Result<String, CreateFolderError> {
    let invalid = |reason: &str| Err(CreateFolderError::InvalidName(reason.to_string().into()));
    let name = name.trim();
    if name.is_empty() {
        return invalid("Enter a name for the folder");
    }
    if name == "." || name == ".." {
        return invalid("A folder cannot be named . or ..");
    }
    if name.chars().count() > MAX_NAME {
        return invalid("That name is too long");
    }
    if name.contains(['/', '\\']) {
        return invalid("A folder name cannot contain / or \\");
    }
    if name.chars().any(char::is_control) {
        return invalid("A folder name cannot contain control characters");
    }
    if style.is_windows() && name.contains(['<', '>', ':', '"', '|', '?', '*']) {
        return invalid("A folder name cannot contain < > : \" | ? *");
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_good_name_is_trimmed() {
        let posix = PathStyle::posix();
        assert_eq!(
            validate_folder_name("  src  ", &posix).as_deref(),
            Ok("src")
        );
        assert_eq!(
            validate_folder_name(".config", &posix).as_deref(),
            Ok(".config")
        );
        assert_eq!(
            validate_folder_name("a b.c", &posix).as_deref(),
            Ok("a b.c")
        );
        assert_eq!(validate_folder_name("a:b", &posix).as_deref(), Ok("a:b"));
    }

    #[test]
    fn a_bad_name_says_why() {
        let posix = PathStyle::posix();
        for name in ["", "   ", ".", "..", "a/b", "a\\b", "a\0b", "a\nb"] {
            assert!(
                matches!(
                    validate_folder_name(name, &posix),
                    Err(CreateFolderError::InvalidName(_))
                ),
                "{name:?}"
            );
        }
        let long = "x".repeat(256);
        assert!(validate_folder_name(&long, &posix).is_err());
        assert!(validate_folder_name(&"x".repeat(255), &posix).is_ok());
    }

    #[test]
    fn windows_refuses_what_windows_refuses() {
        let windows = PathStyle::windows();
        for name in ["a:b", "a*b", "a?b", "a|b", "a<b", "a\"b"] {
            assert!(validate_folder_name(name, &windows).is_err(), "{name}");
        }
        assert!(validate_folder_name("My Files", &windows).is_ok());
    }

    #[test]
    fn the_message_names_the_folder() {
        assert_eq!(
            CreateFolderError::Exists.message("docs"),
            "A folder named docs already exists"
        );
        assert_eq!(
            CreateFolderError::Other("Disk full".into()).message("x"),
            "Disk full"
        );
    }

    #[test]
    fn io_errors_map_to_the_matching_failure() {
        use std::io::{Error, ErrorKind};
        assert_eq!(
            CreateFolderError::from(Error::from(ErrorKind::AlreadyExists)),
            CreateFolderError::Exists
        );
        assert_eq!(
            CreateFolderError::from(Error::from(ErrorKind::PermissionDenied)),
            CreateFolderError::PermissionDenied
        );
    }
}
