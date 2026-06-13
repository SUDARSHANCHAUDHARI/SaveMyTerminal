use anyhow::{Context, Result, bail};
use secrecy::SecretString;
use std::{fs::OpenOptions, io::Write, path::Path};
use uuid::Uuid;

pub fn load_or_create_token(path: &Path) -> Result<SecretString> {
    if path.exists() {
        return load_token(path);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create token directory {}", parent.display()))?;
    }

    let value = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = match options.open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return load_token(path);
        }
        Err(error) => return Err(error.into()),
    };
    file.write_all(value.as_bytes())?;
    file.sync_all()?;
    Ok(SecretString::from(value))
}

pub fn load_token(path: &Path) -> Result<SecretString> {
    let value = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read token file {}", path.display()))?;
    let value = value.trim().to_owned();
    if value.is_empty() {
        bail!("token file is empty")
    }
    Ok(SecretString::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    #[test]
    fn creates_and_reuses_a_nonempty_token() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.token");

        let first = load_or_create_token(&path).unwrap();
        let second = load_or_create_token(&path).unwrap();

        assert_eq!(first.expose_secret(), second.expose_secret());
        assert!(first.expose_secret().len() >= 64);
    }

    #[test]
    fn load_token_does_not_create_a_missing_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.token");

        assert!(load_token(&path).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn rejects_an_empty_token_file() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.token");
        std::fs::write(&path, "\n").unwrap();

        assert!(load_token(&path).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn token_file_is_user_only_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("auth.token");
        load_or_create_token(&path).unwrap();

        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
