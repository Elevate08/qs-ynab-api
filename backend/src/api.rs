use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use std::io::Read;
use std::time::Duration;
use zeroize::Zeroizing;

use crate::models::{
    YnabBudgetsWrapper, YnabCategoryGroupsWrapper, YnabDataWrapper, YnabMonthWrapper,
    YnabUserWrapper,
};

const BASE_URL: &str = "https://api.ynab.com/v1";
const REQUEST_TIMEOUT_SECS: u64 = 10;
/// Upstream error bodies are echoed to the UI; cap them so a hostile or
/// misbehaving endpoint cannot flood the panel or the log with megabytes.
const MAX_ERROR_BODY_CHARS: usize = 200;
/// Ceiling on any response body we will buffer. reqwest reads a body into
/// memory with no limit of its own, so without this a compromised or hostile
/// endpoint could stream until the process is killed by the OOM reaper.
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;

/// Rejects any budget identifier that is not a plain UUID.
///
/// Every id we use is chosen from YNAB's own budget list, so this should
/// always pass. It is enforced anyway because the id is interpolated into a
/// request path: a value carrying `?`, `#`, or `../` would silently retarget
/// the request to a different endpoint while still carrying the bearer token.
pub fn is_valid_budget_id(id: &str) -> bool {
    id.len() == 36
        && id.chars().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => c == '-',
            _ => c.is_ascii_hexdigit(),
        })
}

#[derive(Debug)]
pub enum ApiError {
    Unauthorized,
    RateLimited,
    NotFound(String),
    NetworkError(String),
    JsonError(String),
    HttpError(u16, String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized => write!(f, "Invalid or expired YNAB Personal Access Token (401)"),
            ApiError::RateLimited => write!(f, "YNAB API rate limit exceeded (429)"),
            ApiError::NotFound(res) => write!(f, "Resource not found: {}", res),
            ApiError::NetworkError(msg) => write!(f, "Network communication error: {}", msg),
            ApiError::JsonError(msg) => write!(f, "JSON decoding error: {}", msg),
            ApiError::HttpError(code, msg) => write!(f, "HTTP {} error from YNAB API: {}", code, msg),
        }
    }
}

impl std::error::Error for ApiError {}

pub struct YnabClient {
    client: Client,
}

impl YnabClient {
    pub fn new(token: Zeroizing<String>) -> Result<Self, ApiError> {
        let mut headers = HeaderMap::new();
        let auth_val = Zeroizing::new(format!("Bearer {}", token.trim()));
        let mut auth_header =
            HeaderValue::from_str(&auth_val).map_err(|_| ApiError::Unauthorized)?;
        auth_header.set_sensitive(true);

        headers.insert(AUTHORIZATION, auth_header);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("omarchy-ynab-pulse/1.0"),
        );

        let client = Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            // The YNAB API never redirects. Refusing to follow one removes any
            // path by which the bearer token could be replayed to another host.
            .redirect(reqwest::redirect::Policy::none())
            .https_only(true)
            .build()
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        // The token deliberately is not retained on the struct: it lives only
        // inside the client's sensitive default header.
        Ok(Self { client })
    }

    /// Verifies authentication with GET /v1/user
    pub fn get_user(&self) -> Result<YnabUserWrapper, ApiError> {
        let url = format!("{}/user", BASE_URL);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<YnabUserWrapper>(resp)
    }

    /// Lists user's budgets with GET /v1/budgets
    pub fn get_budgets(&self) -> Result<YnabBudgetsWrapper, ApiError> {
        let url = format!("{}/budgets?include_accounts=false", BASE_URL);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<YnabBudgetsWrapper>(resp)
    }

    /// Fetches all budget months summary with GET /v1/budgets/{budget_id}/months
    pub fn get_months(&self, budget_id: &str) -> Result<crate::models::YnabMonthsWrapper, ApiError> {
        Self::guard_budget_id(budget_id)?;
        let url = format!("{}/budgets/{}/months", BASE_URL, budget_id);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<crate::models::YnabMonthsWrapper>(resp)
    }

    /// Fetches current month overview with GET /v1/budgets/{budget_id}/months/current
    pub fn get_current_month(&self, budget_id: &str) -> Result<YnabMonthWrapper, ApiError> {
        Self::guard_budget_id(budget_id)?;
        let url = format!("{}/budgets/{}/months/current", BASE_URL, budget_id);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<YnabMonthWrapper>(resp)
    }

    /// Fetches category groups and categories with GET /v1/budgets/{budget_id}/categories
    pub fn get_categories(
        &self,
        budget_id: &str,
        server_knowledge: Option<i64>,
    ) -> Result<YnabCategoryGroupsWrapper, ApiError> {
        Self::guard_budget_id(budget_id)?;
        let url = match server_knowledge {
            Some(sk) if sk > 0 => format!(
                "{}/budgets/{}/categories?last_knowledge_of_server={}",
                BASE_URL, budget_id, sk
            ),
            _ => format!("{}/budgets/{}/categories", BASE_URL, budget_id),
        };

        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<YnabCategoryGroupsWrapper>(resp)
    }

    /// Fetches unapproved transactions with GET /v1/budgets/{budget_id}/transactions?type=unapproved
    pub fn get_unapproved_transactions(
        &self,
        budget_id: &str,
    ) -> Result<crate::models::YnabTransactionsWrapper, ApiError> {
        Self::guard_budget_id(budget_id)?;
        let url = format!("{}/budgets/{}/transactions?type=unapproved", BASE_URL, budget_id);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<crate::models::YnabTransactionsWrapper>(resp)
    }

    fn guard_budget_id(budget_id: &str) -> Result<(), ApiError> {
        if is_valid_budget_id(budget_id) {
            Ok(())
        } else {
            Err(ApiError::NotFound("Malformed budget identifier".to_string()))
        }
    }

    /// Buffers at most `MAX_RESPONSE_BYTES` of a response body.
    ///
    /// An advertised `content-length` over the cap is rejected before a single
    /// byte is read; a body that lies about its length (or omits it, as a
    /// chunked response does) is caught by the short read that follows.
    fn read_body_capped(response: reqwest::blocking::Response) -> Result<Vec<u8>, ApiError> {
        if let Some(len) = response.content_length() {
            if len > MAX_RESPONSE_BYTES {
                return Err(ApiError::NetworkError(format!(
                    "Response too large: {} bytes",
                    len
                )));
            }
        }

        // The advertised length is already known and already checked against
        // the cap, so the buffer is sized once instead of doubling its way up
        // through a categories response.
        let mut buf = Vec::with_capacity(
            response.content_length().unwrap_or(0).min(MAX_RESPONSE_BYTES) as usize,
        );
        response
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        if buf.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(ApiError::NetworkError(
                "Response exceeded the size limit".to_string(),
            ));
        }
        Ok(buf)
    }

    fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();
        if status.is_success() {
            let body = Self::read_body_capped(response)?;
            let wrapper: YnabDataWrapper<T> = serde_json::from_slice(&body)
                .map_err(|e| ApiError::JsonError(e.to_string()))?;
            Ok(wrapper.data)
        } else if status == reqwest::StatusCode::UNAUTHORIZED {
            Err(ApiError::Unauthorized)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            Err(ApiError::RateLimited)
        } else if status == reqwest::StatusCode::NOT_FOUND {
            Err(ApiError::NotFound("Endpoint or budget not found".to_string()))
        } else {
            // Bounded read first: .text() would buffer the whole body before
            // the character cap below could ever apply.
            let err_text: String = Self::read_body_capped(response)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default()
                .chars()
                .take(MAX_ERROR_BODY_CHARS)
                .collect();
            Err(ApiError::HttpError(status.as_u16(), err_text))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_valid_budget_id;

    #[test]
    fn accepts_a_uuid_budget_id() {
        assert!(is_valid_budget_id("3fa85f64-5717-4562-b3fc-2c963f66afa6"));
    }

    #[test]
    fn rejects_path_and_query_injection() {
        assert!(!is_valid_budget_id("../../user"));
        assert!(!is_valid_budget_id("3fa85f64-5717-4562-b3fc-2c963f66afa6?x=1"));
        assert!(!is_valid_budget_id("last-used"));
        assert!(!is_valid_budget_id(""));
        assert!(!is_valid_budget_id("3fa85f64_5717_4562_b3fc_2c963f66afa6"));
    }
}
