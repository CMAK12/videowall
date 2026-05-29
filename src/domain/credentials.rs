#[derive(Clone, Debug)]
pub struct LiveKitCredentials {
    pub wss_url: String,
    pub jwt: String,
}

impl LiveKitCredentials {
    pub fn from_api_response(api_url: String, jwt: String) -> Self {
        Self {
            wss_url: to_ws_url(api_url),
            jwt,
        }
    }
}

fn to_ws_url(mut url: String) -> String {
    if let Some(rest) = url.strip_prefix("https://") {
        let mut out = String::with_capacity(rest.len() + 6);
        out.push_str("wss://");
        out.push_str(rest);
        out
    } else if let Some(rest) = url.strip_prefix("http://") {
        let mut out = String::with_capacity(rest.len() + 5);
        out.push_str("ws://");
        out.push_str(rest);
        out
    } else {
        url.shrink_to_fit();
        url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_https_to_wss() {
        let c = LiveKitCredentials::from_api_response(
            "https://stream-us-central1.lumana.ai".to_string(),
            "jwt".to_string(),
        );
        assert_eq!(c.wss_url, "wss://stream-us-central1.lumana.ai");
        assert_eq!(c.jwt, "jwt");
    }

    #[test]
    fn rewrites_http_to_ws() {
        let c = LiveKitCredentials::from_api_response(
            "http://localhost:7880".to_string(),
            "jwt".to_string(),
        );
        assert_eq!(c.wss_url, "ws://localhost:7880");
    }

    #[test]
    fn passes_through_unknown_scheme() {
        let c = LiveKitCredentials::from_api_response(
            "wss://already.example".to_string(),
            "jwt".to_string(),
        );
        assert_eq!(c.wss_url, "wss://already.example");
    }
}
