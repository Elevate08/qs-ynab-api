use serde::{Deserialize, Serialize};

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
    pub iso_code: String,
    pub currency_symbol: String,
    pub decimal_digits: u32,
    pub decimal_separator: String,
    pub symbol_first: bool,
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
    #[serde(default)]
    pub category_group_name: Option<String>,
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
    pub memo: Option<String>,
    pub approved: bool,
    pub cleared: String,
    pub category_id: Option<String>,
    pub payee_name: Option<String>,
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
    #[serde(default)]
    pub user_id: Option<String>,
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
