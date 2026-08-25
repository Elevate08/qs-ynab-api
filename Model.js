// Helper utilities for YNAB Overview Plugin

.pragma library

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
