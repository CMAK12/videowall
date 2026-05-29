#[derive(Clone, Debug)]
pub struct BearerToken(String);

impl BearerToken {
    pub fn new(value: impl Into<String>) -> Self {
        let raw: String = value.into();
        let trimmed = raw.trim();
        let cleaned = trimmed
            .strip_prefix("Bearer ")
            .map(str::trim)
            .unwrap_or(trimmed);
        Self(cleaned.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct RequestBody(Vec<u8>);

impl RequestBody {
    pub fn from_string(value: String) -> Self {
        Self(value.into_bytes())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
