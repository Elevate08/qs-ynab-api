use crate::models::{
    AgeOfMoneyMetric, BudgetSelectionItem, IncomeVsSpendingMetric, PluginCategoryGroup,
    PluginCategoryItem, PluginOverviewResponse, SpendingPieSlice, YnabBudgetSummary,
    YnabCategoryGroupWithCategories, YnabCurrencyFormat, YnabMonthDetail,
};
use chrono::Utc;

/// Upper bound on `decimal_digits` taken from the API response. The field is
/// remote input used as a `pow` exponent and a format width, so an absurd value
/// would overflow the exponent and make `format!` allocate gigabytes of padding.
const MAX_DECIMAL_DIGITS: u32 = 8;

/// Ceilings on how much of a response is turned into panel content. Real
/// budgets are far below these; the caps exist so a hostile or corrupted
/// response cannot make the shell build an unbounded number of delegates and
/// hang the whole desktop bar.
///
/// Per-list caps alone are not enough. The panel nests a category `Repeater`
/// inside a group `Repeater`, and both build every delegate eagerly, so the
/// work the shell does is the *product* of the two caps, not their sum: a cap
/// pair of 200 x 500 authorizes a hundred thousand long-lived shell objects
/// from a response comfortably under the 16 MiB transport limit - small enough
/// to arrive, large enough to hang the bar. A collaborator on a shared budget
/// controls how many groups and categories it contains, so that product has to
/// be bounded directly: `MAX_TOTAL_CATEGORIES` is the number that decides
/// whether the shell stays responsive, and the per-list caps only shape how
/// that budget is spent.
const MAX_BUDGETS: usize = 50;
const MAX_GROUPS: usize = 50;
const MAX_CATEGORIES_PER_GROUP: usize = 100;
const MAX_TOTAL_CATEGORIES: usize = 500;
const MAX_PIE_SLICES: usize = 50;
/// How far the trend graph looks back. Also a cap: `months_history` is remote
/// input, and every month past the sixth is a bar the graph has no room for.
const MAX_TREND_MONTHS: usize = 6;
/// The whole point of the total cap, enforced at compile time: it has to bind
/// before the per-list caps multiply, or it is decoration.
const _: () = assert!(MAX_TOTAL_CATEGORIES < MAX_GROUPS * MAX_CATEGORIES_PER_GROUP);
/// A month label is "Aug". Anything the API sends that is not a month we know
/// still has to fit where "Aug" goes.
const MAX_MONTH_LABEL_CHARS: usize = 8;

/// Formats YNAB milliunits (1/1000th unit, e.g. 1000 = $1.00) into currency string
pub fn format_currency(milliunits: i64, format: &YnabCurrencyFormat) -> String {
    let is_negative = milliunits < 0;
    let abs_milli = milliunits.saturating_abs();
    let divisor = 1000.0;
    let units = abs_milli as f64 / divisor;
    let decimal_digits = format.decimal_digits.min(MAX_DECIMAL_DIGITS);

    let formatted_number = if decimal_digits == 0 {
        let whole = units.round() as i64;
        format_integer_with_groups(whole, &format.group_separator)
    } else {
        let whole = units.floor() as i64;
        let frac_scale = 10u32.pow(decimal_digits) as f64;
        let frac = ((units - whole as f64) * frac_scale).round() as u64;
        let whole_str = format_integer_with_groups(whole, &format.group_separator);
        let frac_str = format!("{:0width$}", frac, width = decimal_digits as usize);
        format!("{}{}{}", whole_str, format.decimal_separator, frac_str)
    };

    let with_symbol = if format.display_symbol {
        if format.symbol_first {
            format!("{}{}", format.currency_symbol, formatted_number)
        } else {
            format!("{} {}", formatted_number, format.currency_symbol)
        }
    } else {
        formatted_number
    };

    if is_negative {
        format!("-{}", with_symbol)
    } else {
        with_symbol
    }
}

fn format_integer_with_groups(n: i64, sep: &str) -> String {
    let s = n.to_string();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut out = String::new();

    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push_str(sep);
        }
        out.push(ch);
    }
    out
}

/// Evaluates category health status following official YNAB color conventions
pub fn determine_category_status(
    balance: i64,
    budgeted: i64,
    goal_target: Option<i64>,
    goal_percentage: Option<u32>,
) -> (&'static str, f64) {
    if balance < 0 {
        // Red: Overspent
        ("red", 1.0)
    } else if let Some(target) = goal_target {
        if target > 0 {
            let progress = (balance as f64 / target as f64).clamp(0.0, 1.0);
            if balance >= target || goal_percentage.unwrap_or(0) >= 100 {
                ("green", 1.0)
            } else if balance == 0 {
                ("yellow", 0.0)
            } else {
                ("yellow", progress)
            }
        } else if balance > 0 {
            ("green", 1.0)
        } else {
            ("muted", 0.0)
        }
    } else if balance > 0 {
        // Green: Funded and available
        let frac = if budgeted > 0 {
            (balance as f64 / budgeted as f64).clamp(0.0, 1.0)
        } else {
            1.0
        };
        ("green", frac)
    } else {
        // Gray: Zero available
        ("muted", 0.0)
    }
}

/// Aggregates Age of Money metric
pub fn aggregate_age_of_money(days_opt: Option<i64>) -> AgeOfMoneyMetric {
    match days_opt {
        Some(days) => {
            let (status, label) = if days >= 30 {
                ("great", "Healthy buffer (30+ days)")
            } else if days >= 15 {
                ("growing", "Growing buffer (15–29 days)")
            } else if days > 0 {
                ("warning", "Low buffer (<15 days)")
            } else {
                ("warning", "Starting out")
            };
            AgeOfMoneyMetric {
                days,
                status: status.to_string(),
                label: label.to_string(),
            }
        }
        None => AgeOfMoneyMetric {
            days: 0,
            status: "unknown".to_string(),
            label: "Not enough transaction history".to_string(),
        },
    }
}

/// Computes Income vs Spending for current month
pub fn aggregate_income_vs_spending(
    month: &YnabMonthDetail,
    currency: &YnabCurrencyFormat,
) -> IncomeVsSpendingMetric {
    let income = month.income.max(0);
    let spending = month.activity.saturating_abs(); // YNAB activity is negative for expenses
    let net = income.saturating_sub(spending);
    let is_positive = net >= 0;

    let savings_rate = if income > 0 {
        ((net as f64 / income as f64) * 100.0).clamp(-100.0, 100.0)
    } else {
        0.0
    };

    IncomeVsSpendingMetric {
        income_milliunits: income,
        income_formatted: format_currency(income, currency),
        spending_milliunits: spending,
        spending_formatted: format_currency(spending, currency),
        net_milliunits: net,
        net_formatted: format_currency(net, currency),
        savings_rate_percent: (savings_rate * 10.0).round() / 10.0,
        is_positive,
    }
}

/// Formats month string "2026-08-01" to short label "Aug"
pub fn format_month_label(month_str: &str) -> String {
    let parts: Vec<&str> = month_str.split('-').collect();
    if parts.len() >= 2 {
        match parts[1] {
            "01" => "Jan".to_string(),
            "02" => "Feb".to_string(),
            "03" => "Mar".to_string(),
            "04" => "Apr".to_string(),
            "05" => "May".to_string(),
            "06" => "Jun".to_string(),
            "07" => "Jul".to_string(),
            "08" => "Aug".to_string(),
            "09" => "Sep".to_string(),
            "10" => "Oct".to_string(),
            "11" => "Nov".to_string(),
            "12" => "Dec".to_string(),
            // Not a month we recognise. Sanitization bounds this to 120
            // chars at the boundary; a bar label wants far less.
            other => other.chars().take(MAX_MONTH_LABEL_CHARS).collect(),
        }
    } else {
        month_str.chars().take(MAX_MONTH_LABEL_CHARS).collect()
    }
}

/// Aggregates multi-month historical trends (last 6 months)
pub fn aggregate_monthly_trends(
    months_raw: &[crate::models::YnabMonthSummary],
    currency: &crate::models::YnabCurrencyFormat,
) -> Vec<crate::models::MonthlyTrendItem> {
    let mut valid_months: Vec<_> = months_raw
        .iter()
        .filter(|m| !m.deleted)
        .collect();

    valid_months.sort_by(|a, b| a.month.cmp(&b.month));

    let count = valid_months.len();
    let start_idx = count.saturating_sub(MAX_TREND_MONTHS);
    let slice = &valid_months[start_idx..];

    slice
        .iter()
        .map(|m| {
            let spending_milliunits = m.activity.saturating_abs();
            let net_milliunits = m.income.saturating_sub(spending_milliunits);
            let is_positive = net_milliunits >= 0;
            let savings_rate_percent = if m.income > 0 {
                let rate = (net_milliunits as f64 / m.income as f64) * 100.0;
                (rate * 10.0).round() / 10.0
            } else {
                0.0
            };

            crate::models::MonthlyTrendItem {
                month: m.month.clone(),
                month_label: format_month_label(&m.month),
                income_milliunits: m.income,
                income_formatted: format_currency(m.income, currency),
                spending_milliunits,
                spending_formatted: format_currency(spending_milliunits, currency),
                net_milliunits,
                net_formatted: format_currency(net_milliunits, currency),
                savings_rate_percent,
                is_positive,
            }
        })
        .collect()
}

/// Builds aggregated overview payload
pub fn build_overview_payload(
    budgets: &[YnabBudgetSummary],
    active_budget: &YnabBudgetSummary,
    month: &YnabMonthDetail,
    months_history: &[crate::models::YnabMonthSummary],
    category_groups_raw: &[YnabCategoryGroupWithCategories],
    unapproved_transactions_count: usize,
    server_knowledge: Option<i64>,
) -> PluginOverviewResponse {
    let currency = active_budget
        .currency_format
        .clone()
        .unwrap_or_default();

    let budget_list: Vec<BudgetSelectionItem> = budgets
        .iter()
        .take(MAX_BUDGETS)
        .map(|b| BudgetSelectionItem {
            id: b.id.clone(),
            name: b.name.clone(),
            last_modified_on: b.last_modified_on.clone(),
        })
        .collect();

    let age_of_money = aggregate_age_of_money(month.age_of_money);
    let income_vs_spending = aggregate_income_vs_spending(month, &currency);
    let monthly_trends = aggregate_monthly_trends(months_history, &currency);

    // Ready to Assign metric
    let ready_to_assign_milliunits = month.to_be_budgeted;
    let ready_to_assign_formatted = format_currency(ready_to_assign_milliunits, &currency);
    let ready_to_assign_status = if ready_to_assign_milliunits > 0 {
        "positive".to_string()
    } else if ready_to_assign_milliunits < 0 {
        "negative".to_string()
    } else {
        "zero".to_string()
    };

    // Group categories and compute bucket details
    let mut category_groups = Vec::new();
    let mut pie_slices_raw = Vec::new();
    let mut total_spending_milli: i64 = 0;
    let mut overspent_categories_count: usize = 0;
    let mut total_categories: usize = 0;

    for group in category_groups_raw {
        if category_groups.len() >= MAX_GROUPS || total_categories >= MAX_TOTAL_CATEGORIES {
            break;
        }
        if group.hidden || group.deleted {
            continue;
        }

        // Exclude system internal groups like "Internal Master Category", "Credit Card Payments" if hidden
        if group.name.starts_with("Internal Master") || group.name.starts_with("Inflow:") {
            continue;
        }

        let mut group_budgeted: i64 = 0;
        let mut group_activity: i64 = 0;
        let mut group_balance: i64 = 0;
        let mut items = Vec::new();

        for cat in &group.categories {
            if items.len() >= MAX_CATEGORIES_PER_GROUP || total_categories >= MAX_TOTAL_CATEGORIES {
                break;
            }
            if cat.hidden || cat.deleted {
                continue;
            }

            group_budgeted = group_budgeted.saturating_add(cat.budgeted);
            group_activity = group_activity.saturating_add(cat.activity);
            group_balance = group_balance.saturating_add(cat.balance);

            let (status_color, progress_fraction) = determine_category_status(
                cat.balance,
                cat.budgeted,
                cat.goal_target,
                cat.goal_percentage_complete,
            );

            let is_overspent = cat.balance < 0;
            if is_overspent {
                overspent_categories_count += 1;
            }

            let overspent_amount_formatted = if is_overspent {
                Some(format_currency(cat.balance.saturating_abs(), &currency))
            } else {
                None
            };

            let goal_target_formatted = cat
                .goal_target
                .map(|t| format_currency(t, &currency));

            items.push(PluginCategoryItem {
                id: cat.id.clone(),
                name: cat.name.clone(),
                budgeted_milliunits: cat.budgeted,
                budgeted_formatted: format_currency(cat.budgeted, &currency),
                activity_milliunits: cat.activity,
                activity_formatted: format_currency(cat.activity, &currency),
                balance_milliunits: cat.balance,
                balance_formatted: format_currency(cat.balance, &currency),
                goal_target_milliunits: cat.goal_target,
                goal_target_formatted,
                goal_percentage: cat.goal_percentage_complete,
                status_color: status_color.to_string(),
                is_overspent,
                overspent_amount_formatted,
                progress_fraction,
            });
            total_categories += 1;
        }

        if !items.is_empty() {
            let group_spending = group_activity.saturating_abs();
            if group_activity < 0 {
                total_spending_milli = total_spending_milli.saturating_add(group_spending);
                pie_slices_raw.push((group.id.clone(), group.name.clone(), group_spending));
            }

            category_groups.push(PluginCategoryGroup {
                id: group.id.clone(),
                name: group.name.clone(),
                budgeted_milliunits: group_budgeted,
                budgeted_formatted: format_currency(group_budgeted, &currency),
                activity_milliunits: group_activity,
                activity_formatted: format_currency(group_activity, &currency),
                balance_milliunits: group_balance,
                balance_formatted: format_currency(group_balance, &currency),
                categories: items,
            });
        }
    }

    // Sort and calculate Pie Chart slices
    pie_slices_raw.sort_by_key(|slice| std::cmp::Reverse(slice.2));
    pie_slices_raw.truncate(MAX_PIE_SLICES);
    let mut spending_pie_chart = Vec::new();
    for (idx, (gid, gname, amount)) in pie_slices_raw.into_iter().enumerate() {
        let pct = if total_spending_milli > 0 {
            ((amount as f64 / total_spending_milli as f64) * 1000.0).round() / 10.0
        } else {
            0.0
        };

        spending_pie_chart.push(SpendingPieSlice {
            group_id: gid,
            group_name: gname,
            amount_milliunits: amount,
            amount_formatted: format_currency(amount, &currency),
            percentage: pct,
            slice_color_index: idx,
        });
    }

    PluginOverviewResponse {
        ok: true,
        authenticated: true,
        error: None,
        budgets: budget_list,
        active_budget_id: Some(active_budget.id.clone()),
        active_budget_name: Some(active_budget.name.clone()),
        currency,
        current_month: month.month.clone(),
        ready_to_assign_milliunits,
        ready_to_assign_formatted,
        ready_to_assign_status,
        overspent_categories_count,
        unapproved_transactions_count,
        age_of_money,
        income_vs_spending,
        monthly_trends,
        category_groups,
        spending_pie_chart,
        server_knowledge,
        fetched_at: Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_currency() {
        let fmt = YnabCurrencyFormat::default();
        assert_eq!(format_currency(123450, &fmt), "$123.45");
        assert_eq!(format_currency(1000000, &fmt), "$1,000.00");
        assert_eq!(format_currency(-50250, &fmt), "-$50.25");
        assert_eq!(format_currency(0, &fmt), "$0.00");
    }

    #[test]
    fn test_category_status_colors() {
        // Negative balance = red (overspent)
        let (c, _) = determine_category_status(-5000, 10000, None, None);
        assert_eq!(c, "red");

        // Positive balance with no goal = green
        let (c, _) = determine_category_status(25000, 25000, None, None);
        assert_eq!(c, "green");

        // Positive balance with goal met = green
        let (c, _) = determine_category_status(100000, 100000, Some(100000), Some(100));
        assert_eq!(c, "green");

        // Positive balance with goal underfunded = yellow
        let (c, frac) = determine_category_status(40000, 40000, Some(100000), Some(40));
        assert_eq!(c, "yellow");
        assert!((frac - 0.4).abs() < 0.001);

        // Zero balance = muted
        let (c, _) = determine_category_status(0, 0, None, None);
        assert_eq!(c, "muted");
    }

    #[test]
    fn test_format_currency_survives_hostile_api_values() {
        // decimal_digits is remote input; an absurd value must be clamped
        // rather than used as a pow exponent and a format width.
        let fmt = YnabCurrencyFormat {
            decimal_digits: u32::MAX,
            ..Default::default()
        };
        let out = format_currency(123450, &fmt);
        assert!(out.len() < 64, "padding was not clamped: {} chars", out.len());

        // i64::MIN has no positive counterpart; abs() must not overflow.
        let fmt = YnabCurrencyFormat::default();
        let _ = format_currency(i64::MIN, &fmt);
    }

    // Fixtures for the cap tests below. Building a payload needs four shapes
    // filled in, and only one or two fields ever matter to a given test, so
    // they live here rather than being spelled out per test.
    fn sample_category(id: &str, hidden: bool) -> crate::models::YnabCategory {
        crate::models::YnabCategory {
            id: id.to_string(),
            category_group_id: "g".to_string(),
            category_group_name: None,
            name: format!("Category {}", id),
            hidden,
            budgeted: 1000,
            activity: -1000,
            balance: 0,
            goal_type: None,
            goal_target: None,
            goal_percentage_complete: None,
            goal_target_month: None,
            deleted: false,
        }
    }

    fn sample_group(
        index: usize,
        categories: Vec<crate::models::YnabCategory>,
    ) -> crate::models::YnabCategoryGroupWithCategories {
        crate::models::YnabCategoryGroupWithCategories {
            id: format!("group-{}", index),
            name: format!("Group {}", index),
            hidden: false,
            deleted: false,
            categories,
        }
    }

    fn sample_budget() -> YnabBudgetSummary {
        YnabBudgetSummary {
            id: "3fa85f64-5717-4562-b3fc-2c963f66afa6".to_string(),
            name: "B".to_string(),
            last_modified_on: None,
            first_month: None,
            last_month: None,
            currency_format: None,
        }
    }

    fn sample_month() -> YnabMonthDetail {
        YnabMonthDetail {
            month: "2026-08-01".to_string(),
            income: 0,
            budgeted: 0,
            activity: 0,
            to_be_budgeted: 0,
            age_of_money: None,
            categories: vec![],
        }
    }

    /// Builds a payload from `groups` alone, with everything else at its
    /// smallest valid value.
    fn payload_from_groups(
        groups: &[crate::models::YnabCategoryGroupWithCategories],
    ) -> PluginOverviewResponse {
        let budget = sample_budget();
        build_overview_payload(
            std::slice::from_ref(&budget),
            &budget,
            &sample_month(),
            &[],
            groups,
            0,
            None,
        )
    }

    fn total_categories(out: &PluginOverviewResponse) -> usize {
        out.category_groups.iter().map(|g| g.categories.len()).sum()
    }

    #[test]
    fn test_payload_entity_counts_are_capped() {
        // A response far larger than any real budget, on every axis at once.
        let groups: Vec<_> = (0..MAX_GROUPS + 50)
            .map(|g| {
                sample_group(
                    g,
                    (0..MAX_CATEGORIES_PER_GROUP + 25)
                        .map(|c| sample_category(&format!("g{}-c{}", g, c), false))
                        .collect(),
                )
            })
            .collect();

        let budget = sample_budget();
        let budgets: Vec<_> = (0..MAX_BUDGETS + 10).map(|_| budget.clone()).collect();
        let months: Vec<_> = (0..MAX_TREND_MONTHS + 30)
            .map(|m| crate::models::YnabMonthSummary {
                month: format!("2026-{:02}-01", (m % 12) + 1),
                income: 1000,
                budgeted: 1000,
                activity: -1000,
                to_be_budgeted: 0,
                age_of_money: None,
                deleted: false,
            })
            .collect();

        let out = build_overview_payload(
            &budgets,
            &budget,
            &sample_month(),
            &months,
            &groups,
            0,
            None,
        );

        assert_eq!(out.budgets.len(), MAX_BUDGETS);
        assert!(out.category_groups.len() <= MAX_GROUPS);
        assert!(out
            .category_groups
            .iter()
            .all(|g| g.categories.len() <= MAX_CATEGORIES_PER_GROUP));
        assert!(out.spending_pie_chart.len() <= MAX_PIE_SLICES);
        assert!(out.monthly_trends.len() <= MAX_TREND_MONTHS);

        // The one that matters: the panel's nested Repeaters build a delegate
        // per category across every group, so the total - not the per-group
        // count - is what the shell has to survive. Per-list caps multiply;
        // this one does not.
        assert_eq!(total_categories(&out), MAX_TOTAL_CATEGORIES);
    }

    #[test]
    fn test_total_cap_is_spent_across_groups_not_per_group() {
        // Many small groups reach the total cap without any single group
        // coming close to the per-group one - the case a per-group cap alone
        // lets through, and the reason the total exists.
        let per_group = 5;
        let groups: Vec<_> = (0..MAX_GROUPS)
            .map(|g| {
                sample_group(
                    g,
                    (0..per_group)
                        .map(|c| sample_category(&format!("g{}-c{}", g, c), false))
                        .collect(),
                )
            })
            .collect();

        let out = payload_from_groups(&groups);

        assert!(total_categories(&out) <= MAX_TOTAL_CATEGORIES);
        assert!(out
            .category_groups
            .iter()
            .all(|g| g.categories.len() <= per_group));
    }

    #[test]
    fn test_hidden_categories_do_not_consume_the_total_allowance() {
        // The cap counts what the panel renders. Hidden and deleted categories
        // never become delegates, so spending the allowance on them would
        // truncate a budget that costs the shell nothing.
        let groups: Vec<_> = (0..20)
            .map(|g| {
                let mut categories = vec![sample_category(&format!("g{}-visible", g), false)];
                categories.extend(
                    (0..80).map(|c| sample_category(&format!("g{}-hidden-{}", g, c), true)),
                );
                sample_group(g, categories)
            })
            .collect();

        let out = payload_from_groups(&groups);

        assert_eq!(out.category_groups.len(), 20);
        assert_eq!(total_categories(&out), 20);
    }

    #[test]
    fn test_a_realistic_budget_survives_the_caps_intact() {
        // The caps only exist to bound a hostile response. A budget larger
        // than any the maintainers have seen must still render in full, or
        // the defense has quietly become a bug that hides someone's money.
        let groups: Vec<_> = (0..15)
            .map(|g| {
                sample_group(
                    g,
                    (0..20)
                        .map(|c| sample_category(&format!("g{}-c{}", g, c), false))
                        .collect(),
                )
            })
            .collect();

        let out = payload_from_groups(&groups);

        assert_eq!(out.category_groups.len(), 15);
        assert!(out.category_groups.iter().all(|g| g.categories.len() == 20));
        assert_eq!(total_categories(&out), 300);
    }

    #[test]
    fn test_age_of_money() {
        let healthy = aggregate_age_of_money(Some(45));
        assert_eq!(healthy.status, "great");
        assert_eq!(healthy.days, 45);

        let growing = aggregate_age_of_money(Some(20));
        assert_eq!(growing.status, "growing");

        let warn = aggregate_age_of_money(Some(7));
        assert_eq!(warn.status, "warning");

        let none = aggregate_age_of_money(None);
        assert_eq!(none.status, "unknown");
    }
}
