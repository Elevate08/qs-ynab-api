#!/usr/bin/env node
// YNAB budget and category values can be attacker-controlled text in shared budgets,
// and Qt renders text as HTML the moment it looks like markup (AutoText default).
// These tests pin both halves of the defense: the neutralizer used for shared
// kit controls, and the `textFormat` every Text in the plugin's own QML must declare.
//
//   node tests/rich-text.test.js

const fs = require("fs")
const path = require("path")
const Model = {}
new Function("exports", fs.readFileSync(path.join(__dirname, "..", "Model.js"), "utf8")
  .replace(/^\.pragma library\s*$/m, "") + `
  exports.plainLabel = plainLabel
`)(Model)

let pass = 0
const failures = []
const check = (l, ok, d) => ok ? pass++ : failures.push(`${l}\n    ${d}`)

// --- plainLabel ---
// Ordinary names without markup characters cannot trip Qt's sniffer, so they must survive byte for byte:
for (const name of ["Household Budget", "Rent and Mortgage", "Café / Dining 🍽", "e-mail (old)", "日本語", "", "a > b"]) {
  check(`plainLabel leaves ${JSON.stringify(name)} untouched`,
    Model.plainLabel(name) === name, JSON.stringify(Model.plainLabel(name)))
}
check("plainLabel maps null and undefined to an empty label",
  Model.plainLabel(null) === "" && Model.plainLabel(undefined) === "",
  JSON.stringify([Model.plainLabel(null), Model.plainLabel(undefined)]))

// Nothing that reaches the control may still read as a tag.
const markup = Model.plainLabel("<img src=x onerror=alert(1)>")
check("plainLabel escapes a tag out of existence",
  markup === '<span style="white-space:pre-wrap">&lt;img src=x onerror=alert(1)&gt;</span>', markup)
check("plainLabel escapes bold markup",
  Model.plainLabel("<b>Groceries</b>").indexOf("<b>") < 0, Model.plainLabel("<b>Groceries</b>"))

// Escaping alone is not enough: without the wrapper Qt may decide the escaped
// string is plain text and show the entities raw. The wrapper forces the
// rich-text path so "&" survives as "&".
const amp = Model.plainLabel("AT&T <holdings>")
check("plainLabel escapes ampersands and forces the rich-text path",
  amp === '<span style="white-space:pre-wrap">AT&amp;T &lt;holdings&gt;</span>', amp)
check("plainLabel neutralizes a value that is already entity-encoded",
  Model.plainLabel("&lt;script&gt;") === '<span style="white-space:pre-wrap">&amp;lt;script&amp;gt;</span>',
  Model.plainLabel("&lt;script&gt;"))
check("plainLabel is idempotent in the sense that re-running it cannot inject",
  Model.plainLabel(Model.plainLabel("<b>x</b>")).indexOf("<b>") < 0,
  Model.plainLabel(Model.plainLabel("<b>x</b>")))

// --- the QML side ---
// Text defaults to Text.AutoText. Category names, budget names, status error
// messages, and currency amounts all land in Text elements, so every one of them
// must pin textFormat: Text.PlainText.
const qmlFiles = fs.readdirSync(path.join(__dirname, ".."))
  .filter(f => f.endsWith(".qml"))

for (const file of qmlFiles) {
  const src = fs.readFileSync(path.join(__dirname, "..", file), "utf8").split("\n")
  const bare = []
  src.forEach((line, i) => {
    if (!/(?<![A-Za-z0-9_.])Text\s*\{/.test(line)) return
    let declared = false
    for (let j = i; j < Math.min(i + 15, src.length); j++) {
      if (src[j].includes("textFormat:")) { declared = true; break }
      if (/^\s*\}/.test(src[j])) break
    }
    if (!declared) bare.push(`${file}:${i + 1}`)
  })
  check(`every Text in ${file} pins textFormat`, bare.length === 0, bare.join(", "))
}

// The kit's Button builds its own Text and exposes no textFormat, so the
// dynamic strings we hand it must arrive already neutralized.
const panel = fs.readFileSync(path.join(__dirname, "..", "Panel.qml"), "utf8")
const dynamicBindings = [
  'active_budget_name ? root.overviewData.active_budget_name',
  'modelData.name + (modelData.id'
]
for (const binding of dynamicBindings) {
  const lines = panel.split("\n").filter(l => l.includes(binding) && /^\s*text:/.test(l))
  check(`button text with ${binding} goes through plainLabel`,
    lines.length > 0 && lines.every(l => l.includes("Model.plainLabel(")),
    JSON.stringify(lines))
}

console.log(`${pass} passed, ${failures.length} failed`)
if (failures.length) { console.error("\nFAILURES:\n  " + failures.join("\n  ")); process.exit(1) }
