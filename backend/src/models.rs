use serde::{Deserialize, Deserializer, Serialize};

/// Remote text is display text, never markup and never multi-line, so it is
/// stripped of control characters and bounded where it enters the process.
///
/// Two caps rather than one: a currency symbol has no business being longer
/// than a few glyphs, while a category name legitimately is. Both go through
/// the same rule so there is one place to change what "sanitized" means.
const MAX_CURRENCY_GLYPH_CHARS: usize = 8;
const MAX_DISPLAY_NAME_CHARS: usize = 120;

fn sanitize(raw: &str, max_chars: usize) -> String {
    raw.chars().filter(|c| !c.is_control()).take(max_chars).collect()
}

fn deserialize_currency_glyph<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(sanitize(&String::deserialize(deserializer)?, MAX_CURRENCY_GLYPH_CHARS))
}

fn deserialize_display_name<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(sanitize(&String::deserialize(deserializer)?, MAX_DISPLAY_NAME_CHARS))
}

fn deserialize_optional_display_name<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?
        .map(|v| sanitize(&v, MAX_DISPLAY_NAME_CHARS)))
}

// --- YNAB Raw API Response Models ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabDataWrapper<T> {
    pub data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabUserWrapper {
    pub user: YnabUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabUser {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabBudgetsWrapper {
    pub budgets: Vec<YnabBudgetSummary>,
    #[serde(default)]
    pub default_budget: Option<YnabBudgetSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabBudgetSummary {
    pub id: String,
    #[serde(deserialize_with = "deserialize_display_name")]
    pub name: String,
    #[serde(default)]
    pub last_modified_on: Option<String>,
    #[serde(default)]
    pub first_month: Option<String>,
    #[serde(default)]
    pub last_month: Option<String>,
    #[serde(default)]
    pub currency_format: Option<YnabCurrencyFormat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabCurrencyFormat {
    #[serde(deserialize_with = "deserialize_currency_glyph")]
    pub iso_code: String,
    #[serde(deserialize_with = "deserialize_currency_glyph")]
    pub currency_symbol: String,
    pub decimal_digits: u32,
    #[serde(deserialize_with = "deserialize_currency_glyph")]
    pub decimal_separator: String,
    pub symbol_first: bool,
    #[serde(deserialize_with = "deserialize_currency_glyph")]
    pub group_separator: String,
    pub display_symbol: bool,
}

impl Default for YnabCurrencyFormat {
    fn default() -> Self {
        Self {
            iso_code: "USD".to_string(),
            currency_symbol: "$".to_string(),
            decimal_digits: 2,
            decimal_separator: ".".to_string(),
            symbol_first: true,
            group_separator: ",".to_string(),
            display_symbol: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabMonthsWrapper {
    pub months: Vec<YnabMonthSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabMonthSummary {
    #[serde(deserialize_with = "deserialize_display_name")]
    pub month: String,
    #[serde(default)]
    pub income: i64,
    #[serde(default)]
    pub budgeted: i64,
    #[serde(default)]
    pub activity: i64,
    #[serde(default)]
    pub to_be_budgeted: i64,
    #[serde(default)]
    pub age_of_money: Option<i64>,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabMonthWrapper {
    pub month: YnabMonthDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabMonthDetail {
    #[serde(deserialize_with = "deserialize_display_name")]
    pub month: String,
    #[serde(default)]
    pub income: i64,
    #[serde(default)]
    pub budgeted: i64,
    #[serde(default)]
    pub activity: i64,
    #[serde(default)]
    pub to_be_budgeted: i64,
    #[serde(default)]
    pub age_of_money: Option<i64>,
    #[serde(default)]
    pub categories: Vec<YnabCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabCategoryGroupsWrapper {
    pub category_groups: Vec<YnabCategoryGroupWithCategories>,
    #[serde(default)]
    pub server_knowledge: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabCategoryGroupWithCategories {
    pub id: String,
    #[serde(deserialize_with = "deserialize_display_name")]
    pub name: String,
    pub hidden: bool,
    pub deleted: bool,
    #[serde(default)]
    pub categories: Vec<YnabCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabCategory {
    pub id: String,
    pub category_group_id: String,
    #[serde(default, deserialize_with = "deserialize_optional_display_name")]
    pub category_group_name: Option<String>,
    #[serde(deserialize_with = "deserialize_display_name")]
    pub name: String,
    pub hidden: bool,
    pub budgeted: i64,
    pub activity: i64,
    pub balance: i64,
    #[serde(default)]
    pub goal_type: Option<String>,
    #[serde(default)]
    pub goal_target: Option<i64>,
    #[serde(default)]
    pub goal_percentage_complete: Option<u32>,
    #[serde(default)]
    pub goal_target_month: Option<String>,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabTransactionsWrapper {
    pub transactions: Vec<YnabTransactionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YnabTransactionSummary {
    pub id: String,
    pub date: String,
    pub amount: i64,
    #[serde(default, deserialize_with = "deserialize_optional_display_name")]
    pub memo: Option<String>,
    pub approved: bool,
    pub cleared: String,
    pub category_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_display_name")]
    pub payee_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_display_name")]
    pub category_name: Option<String>,
    pub deleted: bool,
}

// --- Plugin Aggregated Output Models (Consumable by QML) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOverviewResponse {
    pub ok: bool,
    #[serde(default)]
    pub authenticated: bool,
    #[serde(default)]
    pub error: Option<String>,
    // No user_id: nothing reads it out of this payload (the settings screen
    // takes it from `auth status`), and an account identifier that serves no
    // purpose is just one more thing the cache would be holding.
    #[serde(default)]
    pub budgets: Vec<BudgetSelectionItem>,
    #[serde(default)]
    pub active_budget_id: Option<String>,
    #[serde(default)]
    pub active_budget_name: Option<String>,
    #[serde(default)]
    pub currency: YnabCurrencyFormat,
    #[serde(default)]
    pub current_month: String,
    #[serde(default)]
    pub ready_to_assign_milliunits: i64,
    #[serde(default)]
    pub ready_to_assign_formatted: String,
    #[serde(default)]
    pub ready_to_assign_status: String, // "positive" | "negative" | "zero"
    #[serde(default)]
    pub overspent_categories_count: usize,
    #[serde(default)]
    pub unapproved_transactions_count: usize,
    #[serde(default)]
    pub age_of_money: AgeOfMoneyMetric,
    #[serde(default)]
    pub income_vs_spending: IncomeVsSpendingMetric,
    #[serde(default)]
    pub monthly_trends: Vec<MonthlyTrendItem>,
    #[serde(default)]
    pub category_groups: Vec<PluginCategoryGroup>,
    #[serde(default)]
    pub spending_pie_chart: Vec<SpendingPieSlice>,
    #[serde(default)]
    pub server_knowledge: Option<i64>,
    #[serde(default)]
    pub fetched_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyTrendItem {
    pub month: String,        // "2026-08-01"
    pub month_label: String,  // "Aug"
    pub income_milliunits: i64,
    pub income_formatted: String,
    pub spending_milliunits: i64, // positive amount
    pub spending_formatted: String,
    pub net_milliunits: i64,
    pub net_formatted: String,
    pub savings_rate_percent: f64,
    pub is_positive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetSelectionItem {
    pub id: String,
    pub name: String,
    pub last_modified_on: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgeOfMoneyMetric {
    pub days: i64,
    pub status: String, // "great" | "growing" | "warning" | "unknown"
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IncomeVsSpendingMetric {
    pub income_milliunits: i64,
    pub income_formatted: String,
    pub spending_milliunits: i64, // positive number representing outflow
    pub spending_formatted: String,
    pub net_milliunits: i64,      // income - spending
    pub net_formatted: String,
    pub savings_rate_percent: f64,
    pub is_positive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCategoryGroup {
    pub id: String,
    pub name: String,
    pub budgeted_milliunits: i64,
    pub budgeted_formatted: String,
    pub activity_milliunits: i64,
    pub activity_formatted: String,
    pub balance_milliunits: i64,
    pub balance_formatted: String,
    pub categories: Vec<PluginCategoryItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginCategoryItem {
    pub id: String,
    pub name: String,
    pub budgeted_milliunits: i64,
    pub budgeted_formatted: String,
    pub activity_milliunits: i64,
    pub activity_formatted: String,
    pub balance_milliunits: i64,
    pub balance_formatted: String,
    pub goal_target_milliunits: Option<i64>,
    pub goal_target_formatted: Option<String>,
    pub goal_percentage: Option<u32>,
    pub status_color: String, // "green" | "yellow" | "red" | "muted"
    pub is_overspent: bool,
    pub overspent_amount_formatted: Option<String>,
    pub progress_fraction: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendingPieSlice {
    pub group_id: String,
    pub group_name: String,
    pub amount_milliunits: i64,
    pub amount_formatted: String,
    pub percentage: f64,
    pub slice_color_index: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn currency_glyphs_are_bounded_and_control_free() {
        // A hostile symbol must not survive as a long or control-bearing string.
        let hostile = r#"{
            "iso_code": "USD",
            "currency_symbol": "$(id > /tmp/pwned); echo \u0000",
            "decimal_digits": 2,
            "decimal_separator": ".",
            "symbol_first": true,
            "group_separator": ",",
            "display_symbol": true
        }"#;
        let parsed: YnabCurrencyFormat = serde_json::from_str(hostile).unwrap();
        assert_eq!(parsed.currency_symbol.chars().count(), 8);
        assert!(!parsed.currency_symbol.chars().any(|c| c.is_control()));
    }

    #[test]
    fn ordinary_currency_formats_are_untouched() {
        let normal = r#"{
            "iso_code": "EUR",
            "currency_symbol": "€",
            "decimal_digits": 2,
            "decimal_separator": ",",
            "symbol_first": false,
            "group_separator": ".",
            "display_symbol": true
        }"#;
        let parsed: YnabCurrencyFormat = serde_json::from_str(normal).unwrap();
        assert_eq!(parsed.currency_symbol, "€");
        assert_eq!(parsed.group_separator, ".");
    }

    #[test]
    fn display_names_are_bounded_and_control_free() {
        let hostile = format!(
            r#"{{ "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6", "name": {} }}"#,
            serde_json::to_string(&format!("Groceries\n\u{7}{}", "A".repeat(500))).unwrap()
        );
        let parsed: YnabBudgetSummary = serde_json::from_str(&hostile).unwrap();
        assert_eq!(parsed.name.chars().count(), MAX_DISPLAY_NAME_CHARS);
        assert!(!parsed.name.chars().any(|c| c.is_control()));
    }

    #[test]
    fn ordinary_display_names_are_untouched() {
        let normal = r#"{ "id": "3fa85f64-5717-4562-b3fc-2c963f66afa6", "name": "Caf\u00e9 & Dining \ud83c\udf7d" }"#;
        let parsed: YnabBudgetSummary = serde_json::from_str(normal).unwrap();
        assert_eq!(parsed.name, "Café & Dining 🍽");
    }
}
