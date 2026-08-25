// Helper utilities for YNAB Overview Plugin

.pragma library

// Ceilings on how much of a helper payload the panel will render, mirroring the
// caps the backend applies in aggregator.rs.
//
// The backend is the authority; these exist because the panel must not depend
// on that being true. Every list here feeds a Repeater, and a Repeater builds
// every delegate up front and keeps it alive - there is no windowing, no
// recycling. Group and category Repeaters are nested, so their delegate count
// is the product of the two lists, which is how a response well under the
// 16 MiB transport cap can still ask the shell for tens of thousands of
// objects and hang it. MAX_TOTAL_CATEGORIES bounds that product directly.
//
// A payload can reach the panel from a cache written by an older build, or
// from a backend whose caps were loosened, so the panel re-applies them.
var MAX_BUDGETS = 50
var MAX_GROUPS = 50
var MAX_CATEGORIES_PER_GROUP = 100
var MAX_TOTAL_CATEGORIES = 500
var MAX_PIE_SLICES = 50
var MAX_TREND_MONTHS = 6

// Returns at most `limit` entries, and the original array untouched when it
// already fits - so the common case allocates nothing and keeps identity.
function boundList(list, limit) {
  if (!Array.isArray(list)) return []
  if (limit <= 0) return []
  return list.length > limit ? list.slice(0, limit) : list
}

function shallowCopy(obj) {
  var copy = {}
  for (var key in obj) copy[key] = obj[key]
  return copy
}

// The single choke point every helper payload passes through before the panel
// binds it. Bounding here rather than at each Repeater means a Repeater added
// later cannot quietly escape the caps: there is one place to audit, and it is
// the place the data arrives.
//
// Returns a bounded copy; the payload handed in is never mutated.
function boundOverview(payload) {
  if (!payload || typeof payload !== "object") return payload

  var bounded = shallowCopy(payload)
  bounded.budgets = boundList(payload.budgets, MAX_BUDGETS)
  bounded.spending_pie_chart = boundList(payload.spending_pie_chart, MAX_PIE_SLICES)
  bounded.monthly_trends = boundList(payload.monthly_trends, MAX_TREND_MONTHS)

  // Categories are spent from one shared allowance across all groups, because
  // that total is what decides how many delegates the nested Repeaters build.
  // A group left with no allowance is dropped rather than rendered empty.
  var groups = boundList(payload.category_groups, MAX_GROUPS)
  var kept = []
  var remaining = MAX_TOTAL_CATEGORIES
  for (var i = 0; i < groups.length && remaining > 0; i++) {
    var group = groups[i]
    if (!group || typeof group !== "object") continue
    var categories = boundList(group.categories, Math.min(MAX_CATEGORIES_PER_GROUP, remaining))
    remaining -= categories.length
    if (categories === group.categories) {
      kept.push(group)
    } else {
      var trimmed = shallowCopy(group)
      trimmed.categories = categories
      kept.push(trimmed)
    }
  }
  bounded.category_groups = kept

  return bounded
}

// Palette of colors for the Spending Pie Chart slices (theme-harmonious)
var PIE_PALETTE = [
  "#3b82f6", // Blue
  "#10b981", // Emerald
  "#f59e0b", // Amber
  "#8b5cf6", // Purple
  "#ec4899", // Pink
  "#06b6d4", // Cyan
  "#f97316", // Orange
  "#14b8a6", // Teal
  "#6366f1", // Indigo
  "#84cc16", // Lime
  "#a855f7", // Violet
  "#64748b"  // Slate
];

function getSliceColor(index, defaultColor) {
  if (index >= 0 && index < PIE_PALETTE.length) {
    return PIE_PALETTE[index];
  }
  return defaultColor || "#64748b";
}

function formatAgeOfMoney(days) {
  if (days === null || days === undefined || days < 0) return "N/A";
  if (days === 1) return "1 Day";
  return days + " Days";
}

function getStatusBadgeColor(status, colorObj) {
  switch (status) {
    case "green":
      return "#10b981"; // Success Green
    case "yellow":
      return "#f59e0b"; // Warning Amber
    case "red":
      return colorObj && colorObj.urgent ? colorObj.urgent : "#ef4444"; // Urgent Red
    case "muted":
    default:
      return colorObj && colorObj.muted ? colorObj.muted : "#64748b"; // Muted Gray
  }
}

function truncateString(str, maxLength) {
  if (!str) return "";
  if (str.length <= maxLength) return str;
  return str.substring(0, maxLength - 1) + "…";
}

// Path to the ynab-cli helper, resolved relative to this file's directory -
// which is the plugin directory, the same one `bin/` sits in.
//
// No environment override and no fallback path: nothing that can set your
// environment can substitute the binary that receives the Personal Access
// Token. If resolution ever fails, the helper simply does not run.
//
// Lives here rather than in each entry point because Panel.qml, Settings.qml,
// and YnabAuth.qml all need it, and a copy in each is a copy that can drift.
function cliPath() {
  return Qt.resolvedUrl("bin/ynab-cli").toString().replace(/^file:\/\//, "")
}

// Dispatches desktop notifications via argument list (never shell strings)
// so formatted currency strings cannot introduce shell injection.
function sendNotification(quickshell, summary, body) {
  if (!quickshell || typeof quickshell.execDetached !== "function") return;
  quickshell.execDetached([
    "omarchy-notification-send",
    "--app-name", "YNAB Pulse",
    summary || "YNAB Pulse",
    body
  ]);
}

// Qt's Text -- and every control built on one -- defaults to Text.AutoText,
// which sniffs the string and renders it as HTML the moment it looks like
// markup. Every Text this plugin owns pins `textFormat: Text.PlainText`, but
// the shared kit controls (Ui.Button's label and tooltip) build their own Text
// internally and expose no way to set the format, so a budget or category named
// "<img src=x onerror=...>" would be parsed as markup.
//
// So neutralize the string before it is handed over. A value with no "<" and
// no "&" cannot trip Qt's sniffer and passes through untouched -- which is
// nearly everything. Anything else is HTML-escaped and wrapped in a <span>:
// the escape means no character can be read as a tag, and the wrapper forces
// the rich-text path deterministically so those entities are decoded back to
// the literal characters instead of being shown raw. The white-space rule
// keeps the spacing plain text would have given.
function plainLabel(value) {
  var text = (value === undefined || value === null) ? "" : String(value)
  if (text.indexOf("<") < 0 && text.indexOf("&") < 0) return text
  return "<span style=\"white-space:pre-wrap\">"
    + text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    + "</span>"
}

