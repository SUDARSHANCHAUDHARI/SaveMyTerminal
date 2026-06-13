use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    id: String,
    comment_prefix: String,
}

impl Marker {
    pub fn new(
        id: impl Into<String>,
        comment_prefix: impl Into<String>,
    ) -> Result<Self, ManagedError> {
        let id = id.into();
        let comment_prefix = comment_prefix.into();
        if !valid_identifier(&id) {
            return Err(ManagedError::InvalidMarker("invalid marker identifier"));
        }
        if comment_prefix.is_empty() || comment_prefix.contains(['\r', '\n']) {
            return Err(ManagedError::InvalidMarker("invalid comment prefix"));
        }
        Ok(Self { id, comment_prefix })
    }

    fn begin(&self) -> String {
        format!("{} >>> SaveMyTerminal:{} >>>", self.comment_prefix, self.id)
    }

    fn end(&self) -> String {
        format!("{} <<< SaveMyTerminal:{} <<<", self.comment_prefix, self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockState {
    Missing,
    Present,
}

#[derive(Debug, Error)]
pub enum ManagedError {
    #[error("managed marker is invalid: {0}")]
    InvalidMarker(&'static str),
    #[error("managed block markers are malformed or ambiguous")]
    Conflict,
    #[error("managed block is missing")]
    Missing,
}

pub fn inspect(original: &str, marker: &Marker) -> Result<BlockState, ManagedError> {
    Ok(match locate(original, marker)? {
        Some(_) => BlockState::Present,
        None => BlockState::Missing,
    })
}

pub fn insert_or_replace(
    original: &str,
    marker: &Marker,
    body: &str,
) -> Result<String, ManagedError> {
    let block = format!(
        "{}\n{}\n{}\n",
        marker.begin(),
        body.trim_end_matches(['\r', '\n']),
        marker.end()
    );
    if let Some((start, end)) = locate(original, marker)? {
        let mut updated = String::with_capacity(original.len() - (end - start) + block.len());
        updated.push_str(&original[..start]);
        updated.push_str(&block);
        updated.push_str(&original[end..]);
        return Ok(updated);
    }

    let mut updated = original.to_owned();
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&block);
    Ok(updated)
}

pub fn insert_or_replace_prepend(
    original: &str,
    marker: &Marker,
    body: &str,
) -> Result<String, ManagedError> {
    let block = format!(
        "{}\n{}\n{}\n",
        marker.begin(),
        body.trim_end_matches(['\r', '\n']),
        marker.end()
    );
    if let Some((start, end)) = locate(original, marker)? {
        let mut updated = String::with_capacity(original.len() - (end - start) + block.len());
        updated.push_str(&original[..start]);
        updated.push_str(&block);
        updated.push_str(&original[end..]);
        return Ok(updated);
    }
    Ok(format!("{block}{original}"))
}

pub fn remove(original: &str, marker: &Marker) -> Result<String, ManagedError> {
    let (start, end) = locate(original, marker)?.ok_or(ManagedError::Missing)?;
    let mut updated = String::with_capacity(original.len() - (end - start));
    updated.push_str(&original[..start]);
    updated.push_str(&original[end..]);
    Ok(updated)
}

fn locate(original: &str, marker: &Marker) -> Result<Option<(usize, usize)>, ManagedError> {
    let begin = marker.begin();
    let end = marker.end();
    let mut begin_span = None;
    let mut end_span = None;
    let mut offset = 0;

    for line in original.split_inclusive('\n') {
        let content = line.trim_end_matches(['\r', '\n']);
        if content == begin {
            if begin_span.is_some() {
                return Err(ManagedError::Conflict);
            }
            begin_span = Some(offset);
        }
        if content == end {
            if end_span.is_some() {
                return Err(ManagedError::Conflict);
            }
            end_span = Some(offset + line.len());
        }
        offset += line.len();
    }

    match (begin_span, end_span) {
        (None, None) => Ok(None),
        (Some(start), Some(end)) if start < end => Ok(Some((start, end))),
        _ => Err(ManagedError::Conflict),
    }
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}
