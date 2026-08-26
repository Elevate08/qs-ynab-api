#!/usr/bin/env node
// A shared budget's group and category counts are collaborator-controlled, and
// the panel renders them with nested Repeaters that build every delegate up
// front and keep it alive. The delegate count is therefore the *product* of the
// two lists, which is how a response well under the 16 MiB transport cap can
// still ask the shell for tens of thousands of long-lived objects and hang it.
// These tests pin the panel's half of the defense: the caps in Model.js, and
// the single choke point every payload has to pass through to reach a binding.
//
//   node tests/entity-bounds.test.js

const fs = require("fs")
const path = require("path")
const root = path.join(__dirname, "..")
const Model = {}
new Function("exports", fs.readFileSync(path.join(root, "Model.js"), "utf8")
  .replace(/^\.pragma library\s*$/m, "") + `
  exports.boundList = boundList
  exports.boundOverview = boundOverview
  exports.MAX_BUDGETS = MAX_BUDGETS
  exports.MAX_GROUPS = MAX_GROUPS
  exports.MAX_CATEGORIES_PER_GROUP = MAX_CATEGORIES_PER_GROUP
  exports.MAX_TOTAL_CATEGORIES = MAX_TOTAL_CATEGORIES
  exports.MAX_PIE_SLICES = MAX_PIE_SLICES
  exports.MAX_TREND_MONTHS = MAX_TREND_MONTHS
`)(Model)

let pass = 0
const failures = []
const check = (l, ok, d) => ok ? pass++ : failures.push(`${l}\n    ${d}`)

const list = (n, make) => Array.from({ length: n }, (_, i) => make(i))

// --- boundList ---
check("boundList truncates a list over its limit",
  JSON.stringify(Model.boundList([1, 2, 3, 4], 2)) === "[1,2]",
  JSON.stringify(Model.boundList([1, 2, 3, 4], 2)))
const fits = [1, 2]
check("boundList returns a list that fits unchanged, without copying",
  Model.boundList(fits, 5) === fits, "copied a list that already fit")
for (const junk of [null, undefined, "not an array", 7, {}, { length: 3 }]) {
  check(`boundList maps ${JSON.stringify(junk)} to an empty list`,
    Array.isArray(Model.boundList(junk, 5)) && Model.boundList(junk, 5).length === 0,
    JSON.stringify(Model.boundList(junk, 5)))
}
check("boundList treats a zero or negative limit as nothing renderable",
  Model.boundList([1, 2], 0).length === 0 && Model.boundList([1, 2], -1).length === 0,
  JSON.stringify([Model.boundList([1, 2], 0), Model.boundList([1, 2], -1)]))

// --- boundOverview ---
// A payload far larger than the caps on every axis at once - the shape a
// hostile shared budget would take.
const hostile = {
  ok: true,
  active_budget_id: "3fa85f64-5717-4562-b3fc-2c963f66afa6",
  active_budget_name: "B",
  budgets: list(Model.MAX_BUDGETS + 40, i => ({ id: `b-${i}`, name: `Budget ${i}` })),
  category_groups: list(Model.MAX_GROUPS + 150, g => ({
    id: `g-${g}`,
    name: `Group ${g}`,
    categories: list(Model.MAX_CATEGORIES_PER_GROUP + 400, c => ({ id: `g${g}-c${c}`, name: `Cat ${c}` })),
  })),
  spending_pie_chart: list(Model.MAX_PIE_SLICES + 20, i => ({ group_id: `g-${i}` })),
  monthly_trends: list(Model.MAX_TREND_MONTHS + 30, i => ({ month: `M${i}` })),
}
const bounded = Model.boundOverview(hostile)

check("boundOverview caps the budget list",
  bounded.budgets.length === Model.MAX_BUDGETS, bounded.budgets.length)
check("boundOverview caps the group list",
  bounded.category_groups.length <= Model.MAX_GROUPS, bounded.category_groups.length)
check("boundOverview caps categories within each group",
  bounded.category_groups.every(g => g.categories.length <= Model.MAX_CATEGORIES_PER_GROUP),
  JSON.stringify(bounded.category_groups.map(g => g.categories.length)))
check("boundOverview caps pie slices",
  bounded.spending_pie_chart.length === Model.MAX_PIE_SLICES, bounded.spending_pie_chart.length)
check("boundOverview caps trend months",
  bounded.monthly_trends.length === Model.MAX_TREND_MONTHS, bounded.monthly_trends.length)

// The one that matters: per-list caps multiply, the total does not.
const totalCategories = bounded.category_groups.reduce((n, g) => n + g.categories.length, 0)
check("boundOverview caps categories across all groups combined",
  totalCategories === Model.MAX_TOTAL_CATEGORIES, totalCategories)
check("the total cap is what bounds delegate count, not the per-list product",
  Model.MAX_TOTAL_CATEGORIES < Model.MAX_GROUPS * Model.MAX_CATEGORIES_PER_GROUP,
  `${Model.MAX_TOTAL_CATEGORIES} vs ${Model.MAX_GROUPS * Model.MAX_CATEGORIES_PER_GROUP}`)
check("boundOverview renders no group emptied by the shared allowance",
  bounded.category_groups.every(g => g.categories.length > 0),
  JSON.stringify(bounded.category_groups.map(g => g.categories.length)))

// The caller keeps its own payload: the panel reads `parsed` again for
// active_budget_id right after binding the bounded copy.
check("boundOverview does not mutate the payload it was given",
  hostile.budgets.length === Model.MAX_BUDGETS + 40 &&
  hostile.category_groups.length === Model.MAX_GROUPS + 150 &&
  hostile.category_groups[0].categories.length === Model.MAX_CATEGORIES_PER_GROUP + 400,
  "the incoming payload was trimmed in place")
check("boundOverview carries the payload's other fields through",
  bounded.ok === true && bounded.active_budget_id === hostile.active_budget_id &&
  bounded.active_budget_name === "B",
  JSON.stringify({ ok: bounded.ok, id: bounded.active_budget_id, name: bounded.active_budget_name }))

// A payload already within the caps has to survive untouched, or the panel
// would silently hide part of a real budget.
const small = {
  budgets: list(3, i => ({ id: `b-${i}` })),
  category_groups: list(4, g => ({ id: `g-${g}`, categories: list(10, c => ({ id: `c-${c}` })) })),
  spending_pie_chart: list(4, i => ({ group_id: `g-${i}` })),
  monthly_trends: list(6, i => ({ month: `M${i}` })),
}
const smallBounded = Model.boundOverview(small)
check("boundOverview leaves a real-sized payload intact",
  smallBounded.budgets.length === 3 && smallBounded.category_groups.length === 4 &&
  smallBounded.category_groups.every(g => g.categories.length === 10) &&
  smallBounded.spending_pie_chart.length === 4 && smallBounded.monthly_trends.length === 6,
  JSON.stringify(smallBounded.category_groups.map(g => g.categories.length)))

// Missing and malformed lists reach the panel as empty arrays, never as
// undefined a Repeater would choke on.
for (const junk of [{}, { category_groups: null, budgets: "x" }, { category_groups: [null, 7, "g"] }]) {
  const out = Model.boundOverview(junk)
  check(`boundOverview normalizes ${JSON.stringify(junk)} into empty lists`,
    Array.isArray(out.budgets) && Array.isArray(out.category_groups) &&
    Array.isArray(out.spending_pie_chart) && Array.isArray(out.monthly_trends) &&
    out.category_groups.length === 0,
    JSON.stringify(out))
}
for (const junk of [null, undefined, "payload", 7]) {
  check(`boundOverview passes ${JSON.stringify(junk)} through rather than inventing a payload`,
    Model.boundOverview(junk) === junk, JSON.stringify(Model.boundOverview(junk)))
}

// --- The panel's caps mirror the backend's ---
// SECURITY.md tells readers the caps are applied twice over the same numbers.
// Nothing links the two files, so drift is silent: raise a cap in the helper
// and the panel starts truncating a budget the backend was happy to send,
// which reads as lost data rather than as a defense firing.
const rust = fs.readFileSync(path.join(root, "backend", "src", "aggregator.rs"), "utf8")
const rustCaps = {}
for (const [, name, value] of rust.matchAll(/^const\s+(MAX_[A-Z_]+):\s*usize\s*=\s*(\d+)\s*;/gm)) {
  rustCaps[name] = Number(value)
}
const shared = ["MAX_BUDGETS", "MAX_GROUPS", "MAX_CATEGORIES_PER_GROUP",
                "MAX_TOTAL_CATEGORIES", "MAX_PIE_SLICES", "MAX_TREND_MONTHS"]
check("the caps were actually read out of aggregator.rs",
  shared.every(name => typeof rustCaps[name] === "number"),
  JSON.stringify(rustCaps))
for (const name of shared) {
  check(`${name} agrees with the backend`,
    Model[name] === rustCaps[name], `Model.js ${Model[name]} vs aggregator.rs ${rustCaps[name]}`)
}

// --- Service.qml wiring ---
// Bounding at one choke point is only a defense if nothing bypasses it, so pin
// that every assignment to overviewData goes through boundOverview.
const service = fs.readFileSync(path.join(root, "Service.qml"), "utf8")
const assignments = service.match(/^\s*root\.overviewData\s*=\s*.+$/gm) || []
const unbounded = assignments.filter(l => !/Model\.boundOverview\(/.test(l) && !/=\s*null\s*$/.test(l))
check("every payload assigned to overviewData passes through Model.boundOverview",
  assignments.length >= 2 && unbounded.length === 0,
  `${assignments.length} assignments, unbounded: ${JSON.stringify(unbounded)}`)

console.log(`${pass} passed, ${failures.length} failed`)
if (failures.length) { console.error("\nFAILURES:\n  " + failures.join("\n  ")); process.exit(1) }
