#!/usr/bin/env node
// The popup is a KeyboardPanel with internal scrolling. Size must go through
// the panel's own clamp helpers (so geometry changes rebind), and every
// overflow surface must be the same Flickable — a leftover ScrollView on
// Income or Settings will grow the card the way the old buckets list did.
//
//   node tests/panel-layout.test.js

const fs = require("fs")
const path = require("path")
const root = path.join(__dirname, "..")
const panel = fs.readFileSync(path.join(root, "Panel.qml"), "utf8")
const bar = fs.readFileSync(path.join(root, "BarWidget.qml"), "utf8")

let pass = 0
const failures = []
const check = (l, ok, d) => ok ? pass++ : failures.push(`${l}\n    ${d}`)

check("card width uses KeyboardPanel.fittedContentWidth",
  /contentWidth:\s*keyboardPanel\.fittedContentWidth\(Style\.space\(480\)\)/.test(panel),
  "expected contentWidth: keyboardPanel.fittedContentWidth(Style.space(480))")
check("card height uses KeyboardPanel.cappedContentHeight",
  /contentHeight:\s*keyboardPanel\.cappedContentHeight\(Style\.space\(600\)\)/.test(panel),
  "expected contentHeight: keyboardPanel.cappedContentHeight(Style.space(600))")
check("panel does not reimplement availableCard* clamping",
  !/availableCardWidth/.test(panel) && !/availableCardHeight/.test(panel),
  "hand-rolled availableCard* clamp is still in Panel.qml")
check("no ScrollView remains in the panel",
  !/ScrollView\s*\{/.test(panel),
  "ScrollView still present; Income/Settings should use the shared Flickable")

const flickUses = panel.match(/VerticalFlick\s*\{/g) || []
check("settings, buckets, income, and spending share VerticalFlick",
  flickUses.length === 4, `found ${flickUses.length} VerticalFlick uses`)

check("bar widget does not clip the slot",
  !/^\s*clip:\s*true\s*$/m.test(bar),
  "BarWidget.qml still sets clip: true")
check("panel Loader stays 0x0 so it cannot leak into the bar",
  /width:\s*0/.test(bar) && /height:\s*0/.test(bar),
  bar)

console.log(`${pass} passed, ${failures.length} failed`)
if (failures.length) { console.error("\nFAILURES:\n  " + failures.join("\n  ")); process.exit(1) }
