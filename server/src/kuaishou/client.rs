use serde::de::DeserializeOwned;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ApiEnvelope<T> {
    pub code: i64,
    #[serde(rename = "message")]
    pub _message: String,
    pub data: Option<T>,
}

#[derive(Debug, thiserror::Error)]
pub enum KsError {
    #[error("kuaishou_token_expired")]
    TokenExpired,
    #[error("kuaishou_rate_limited")]
    RateLimited,
    #[error("kuaishou_upstream_server")]
    UpstreamServer,
    #[error("kuaishou_business_error")]
    Business,
    #[error("kuaishou_decode_error")]
    Decode,
}

pub fn decode_envelope<T: DeserializeOwned>(body: &str) -> Result<T, KsError> {
    let env: ApiEnvelope<T> = serde_json::from_str(body).map_err(|_| KsError::Decode)?;

    match env.code {
        0 => env.data.ok_or(KsError::Decode),
        40001 => Err(KsError::TokenExpired),
        40029 => Err(KsError::RateLimited),
        50000 => Err(KsError::UpstreamServer),
        _ => Err(KsError::Business),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{decode_envelope, KsError};

    #[derive(Debug, Deserialize)]
    struct EmptyData {}

    #[test]
    fn oauth_error_response_maps_40001() {
        let body = r#"{"code":40001,"message":"access_token expired","data":null}"#;
        let err = decode_envelope::<EmptyData>(body).expect_err("should map 40001");
        assert!(matches!(err, KsError::TokenExpired));
    }

    #[test]
    fn oauth_error_response_maps_40029() {
        let body = r#"{"code":40029,"message":"api freq out of limit","data":null}"#;
        let err = decode_envelope::<EmptyData>(body).expect_err("should map 40029");
        assert!(matches!(err, KsError::RateLimited));
    }
}
