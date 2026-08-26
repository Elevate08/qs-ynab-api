import QtQuick
import Quickshell
import Quickshell.Io
import "Model.js" as Model

Item {
  id: root

  property var shell: null
  property var manifest: null

  // Central state
  property var overviewData: null
  property bool authenticated: false
  property bool loading: false
  property string statusError: ""
  property string activeBudgetId: ""
  property int runtimeRefreshHours: 24

  readonly property string cliPath: Model.cliPath()

  signal requestOpen()
  signal requestClose()
  signal requestToggle()
  signal requestTab(int index)
  signal requestSettings()
  signal requestDrilldown(string groupId)

  function isValidBudgetId(id) {
    return typeof id === "string" && /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(id)
  }

  function executeFetch(force) {
    if (fetchProc.running) return
    var args = [cliPath, "fetch"]
    if (force) args.push("--force")
    if (isValidBudgetId(activeBudgetId)) {
      args.push("--budget-id", activeBudgetId)
    }
    fetchProc.command = args
    loading = true
    statusError = ""
    fetchProc.running = true
  }

  function refresh() { executeFetch(false) }
  function forceRefresh() { executeFetch(true) }

  function selectBudget(budgetId) {
    if (!isValidBudgetId(budgetId)) {
      statusError = "Ignoring malformed budget identifier"
      return
    }
    activeBudgetId = budgetId
    Quickshell.execDetached(["omarchy", "bar", "set", "io.github.elevate08.ynab-glance", "defaultBudgetId", budgetId])
    forceRefresh()
  }

  function setRefreshHours(hours) {
    var h = Math.max(1, Math.min(168, hours))
    runtimeRefreshHours = h
    Quickshell.execDetached(["omarchy", "bar", "set", "io.github.elevate08.ynab-glance", "refreshIntervalHours", h.toString(), "--json"])
  }

  // Auth delegation
  YnabAuth {
    id: auth
    onStatusChecked: function(authed, userId) {
      root.authenticated = authed
      if (authed) {
        root.refresh()
      } else {
        root.loading = false
      }
    }
    onTokenSaved: {
      root.authenticated = true
      root.statusError = ""
      root.forceRefresh()
    }
    onTokenCleared: function(cachePurged) {
      root.authenticated = false
      root.overviewData = null
      root.statusError = ""
      root.loading = false
    }
    onFailed: function(msg) {
      root.statusError = msg || "Authentication error"
      root.loading = false
    }
  }

  readonly property bool authBusy: auth.busy

  function checkAuthStatus() { auth.checkStatus() }
  function saveToken(token) {
    loading = true
    statusError = ""
    auth.saveToken(token)
  }
  function clearToken() {
    loading = true
    auth.clearToken()
  }

  function handleFetchResponse(text) {
    try {
      var parsed = JSON.parse(text)
      if (parsed.ok) {
        root.overviewData = Model.boundOverview(parsed)
        root.authenticated = true
        root.statusError = ""
        if (root.activeBudgetId === "" && root.isValidBudgetId(parsed.active_budget_id)) {
          root.activeBudgetId = parsed.active_budget_id
        }
      } else {
        root.authenticated = parsed.authenticated || false
        root.statusError = parsed.error || "Failed to fetch YNAB data"
      }
    } catch (e) {
      root.statusError = "Error parsing YNAB backend response"
    }
    root.loading = false
  }

  // Subprocess
  Process {
    id: fetchProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.handleFetchResponse(text)
    }
    onExited: function(code) {
      root.loading = false
      if (code !== 0 && root.statusError === "") {
        root.statusError = "Backend helper exited with error code " + code
      }
    }
  }

  // Periodic background timer
  Timer {
    interval: Math.max(1, root.runtimeRefreshHours) * 3600 * 1000
    running: root.authenticated
    repeat: true
    onTriggered: root.refresh()
  }

  Component.onCompleted: {
    root.checkAuthStatus()
  }

  // Headless IPC Endpoint
  IpcHandler {
    target: "io.github.elevate08.ynab-glance"

    function open(): void { root.requestOpen() }
    function close(): void { root.requestClose() }
    function toggle(): void { root.requestToggle() }
    function show(): void { root.requestOpen() }
    function hide(): void { root.requestClose() }
    function refresh(): string {
      root.refresh()
      return "ok"
    }
    function forceRefresh(): string {
      root.forceRefresh()
      return "ok"
    }
    function settings(): void { root.requestSettings() }
    function tab(index: int): void { root.requestTab(index) }
    function drilldown(): void {
      var groupId = ""
      if (root.overviewData && root.overviewData.spending_pie_chart && root.overviewData.spending_pie_chart.length > 1) {
        groupId = root.overviewData.spending_pie_chart[1].group_id || ""
      }
      root.requestDrilldown(groupId)
    }
    function selectBudget(budgetId: string): string {
      if (!root.isValidBudgetId(budgetId)) return "invalid_budget_id"
      root.selectBudget(budgetId)
      return "ok"
    }
    function status(): string {
      var aom = (root.overviewData && root.overviewData.age_of_money) ? root.overviewData.age_of_money.days : null
      var rta = root.overviewData ? root.overviewData.ready_to_assign_formatted : null
      return JSON.stringify({
        authenticated: root.authenticated,
        loading: root.loading,
        error: root.statusError,
        readyToAssign: rta,
        ageOfMoneyDays: aom
      })
    }
    function ping(): string {
      return "ok"
    }
  }
}
