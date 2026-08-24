use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, USER_AGENT};
use std::time::Duration;

use crate::models::{
    YnabBudgetsWrapper, YnabCategoryGroupsWrapper, YnabDataWrapper, YnabMonthWrapper,
    YnabUserWrapper,
};

const BASE_URL: &str = "https://api.ynab.com/v1";
const REQUEST_TIMEOUT_SECS: u64 = 10;

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
    token: String,
}

impl YnabClient {
    pub fn new(token: String) -> Result<Self, ApiError> {
        let mut headers = HeaderMap::new();
        let auth_val = format!("Bearer {}", token.trim());
        let mut auth_header = HeaderValue::from_str(&auth_val)
            .map_err(|_| ApiError::Unauthorized)?;
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
            .redirect(reqwest::redirect::Policy::limited(2))
            .build()
            .map_err(|e| ApiError::NetworkError(e.to_string()))?;

        Ok(Self { client, token })
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
        let url = format!("{}/budgets/{}/months", BASE_URL, budget_id);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<crate::models::YnabMonthsWrapper>(resp)
    }

    /// Fetches current month overview with GET /v1/budgets/{budget_id}/months/current
    pub fn get_current_month(&self, budget_id: &str) -> Result<YnabMonthWrapper, ApiError> {
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
        let url = format!("{}/budgets/{}/transactions?type=unapproved", BASE_URL, budget_id);
        let resp = self.client.get(&url).send().map_err(|e| ApiError::NetworkError(e.to_string()))?;

        self.handle_response::<crate::models::YnabTransactionsWrapper>(resp)
    }

    fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::blocking::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();
        if status.is_success() {
            let wrapper: YnabDataWrapper<T> = response
                .json()
                .map_err(|e| ApiError::JsonError(e.to_string()))?;
            Ok(wrapper.data)
        } else if status == reqwest::StatusCode::UNAUTHORIZED {
            Err(ApiError::Unauthorized)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            Err(ApiError::RateLimited)
        } else if status == reqwest::StatusCode::NOT_FOUND {
            Err(ApiError::NotFound("Endpoint or budget not found".to_string()))
        } else {
            let err_text = response.text().unwrap_or_default();
            Err(ApiError::HttpError(status.as_u16(), err_text))
        }
    }
}
