import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "Model.js" as Model

Panel {
  id: root
  moduleName: "io.github.elevate08.ynab-glance"
  manageIpc: false

  property var hostWidget: null
  property var service: hostWidget && hostWidget.service ? hostWidget.service : (bar?.shell?.serviceFor("io.github.elevate08.ynab-glance"))
  property var anchorItem: hostWidget ? hostWidget.anchorItem : null

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  readonly property color foreground: bar ? bar.foreground : Color.foreground
  readonly property color barForeground: bar ? bar.barForeground : Color.foreground
  readonly property color urgent: bar ? bar.urgent : Color.urgent
  readonly property string fontFamily: bar ? bar.fontFamily : Style.font.family

  // Plugin state (bound to service singleton)
  readonly property var overviewData: service ? service.overviewData : null
  readonly property bool authenticated: service ? service.authenticated : false
  readonly property bool loading: service ? (service.loading || service.authBusy) : false
  readonly property string statusError: service ? service.statusError : ""
  property string activeBudgetId: service ? service.activeBudgetId : setting("defaultBudgetId", "")
  property int runtimeRefreshHours: service ? service.runtimeRefreshHours : setting("refreshIntervalHours", 24)

  property int activeTab: 0 // 0: Buckets, 1: Income & Age, 2: Spending Analysis
  property bool showSettings: false
  property bool showBudgetSelector: false
  property bool showSettingsBudgetDropdown: false
  property bool showShortcutsHelp: false
  property string selectedSpendingGroupId: ""

  readonly property var selectedSpendingGroup: {
    if (!root.overviewData || !root.overviewData.category_groups || root.selectedSpendingGroupId === "") return null
    for (var i = 0; i < root.overviewData.category_groups.length; i++) {
      if (root.overviewData.category_groups[i].id === root.selectedSpendingGroupId) {
        return root.overviewData.category_groups[i]
      }
    }
    return null
  }

  // Collapsed category groups mapping: { groupId: bool }
  property var collapsedGroups: ({})
  property bool allCollapsed: false

  // Onboarding draft token
  property string tokenDraft: ""

  readonly property bool showAgeOfMoneyInBar: setting("showAgeOfMoneyInBar", true)
  readonly property string iconGlyph: setting("iconGlyph", "\uf0d6")
  readonly property string barLabel: {
    if (!overviewData || !overviewData.age_of_money || overviewData.age_of_money.days === undefined) return ""
    return overviewData.age_of_money.days + "d"
  }

  onOpenedChanged: {
    if (root.opened) {
      showSettings = false
      showBudgetSelector = false
      showSettingsBudgetDropdown = false
      selectedSpendingGroupId = ""
      root.refresh()
    } else {
      // A half-typed Personal Access Token must not survive in a QML string
      // property after the panel is dismissed.
      tokenDraft = ""
    }
  }

  function refresh() {
    if (service) service.refresh()
  }

  function forceRefresh() {
    if (service) service.forceRefresh()
  }

  // YNAB budget ids are UUIDs. Anything else is refused before it reaches a
  // subprocess argument or a URL handed to xdg-open.
  function isValidBudgetId(id) {
    return typeof id === "string" && /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/.test(id)
  }

  function selectBudget(budgetId) {
    if (!isValidBudgetId(budgetId)) {
      return
    }
    showBudgetSelector = false
    showSettingsBudgetDropdown = false
    if (service) service.selectBudget(budgetId)
  }

  function setRefreshHours(hours) {
    if (service) service.setRefreshHours(hours)
  }

  function toggleGroupCollapse(groupId) {
    var copy = Object.assign({}, collapsedGroups)
    copy[groupId] = !copy[groupId]
    collapsedGroups = copy
  }

  function toggleAllCollapse() {
    allCollapsed = !allCollapsed
    var copy = {}
    if (overviewData && overviewData.category_groups) {
      for (var i = 0; i < overviewData.category_groups.length; i++) {
        copy[overviewData.category_groups[i].id] = allCollapsed
      }
    }
    collapsedGroups = copy
  }

  function saveToken() {
    var trimmed = tokenDraft.trim()
    if (trimmed === "") {
      return
    }
    if (service) {
      service.saveToken(trimmed)
      tokenDraft = ""
      showSettings = false
    }
  }

  function clearToken() {
    if (service) {
      service.clearToken()
      showSettings = false
    }
  }

  function openWebApp() {
    var url = "https://app.ynab.com"
    // active_budget_id arrives from the network; only append it once it is a
    // known-shape UUID, so a hostile response cannot steer xdg-open.
    if (overviewData && isValidBudgetId(overviewData.active_budget_id)) {
      url += "/" + overviewData.active_budget_id
    }
    Quickshell.execDetached(["xdg-open", url])
  }

  function openDeveloperSettings() {
    Quickshell.execDetached(["xdg-open", "https://app.ynab.com/settings/developer"])
  }

  // -------------------------------------------------------------------------
  // Status Bar Button (fallback if hosted standalone)
  // -------------------------------------------------------------------------
  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.iconGlyph
    slotSize: Style.bar.statusSlot
    tooltipText: root.authenticated ? "YNAB Pulse" : "YNAB Pulse (Setup Required)"

    onPressed: function(b) {
      if (b === Qt.RightButton) {
        if (root.overviewData) {
          var aom = root.overviewData.age_of_money ? root.overviewData.age_of_money.days : 0
          var rta = root.overviewData.ready_to_assign_formatted || "$0"
          Model.sendNotification(Quickshell, "YNAB Pulse", "Ready to Assign: " + rta + " | Age of Money: " + aom + "d")
        } else {
          root.toggle()
        }
      } else if (b === Qt.MiddleButton) {
        root.forceRefresh()
      } else {
        root.toggle()
      }
    }
  }

  // -------------------------------------------------------------------------
  // Popup Window (KeyboardPanel)
  // -------------------------------------------------------------------------
  KeyboardPanel {
    id: keyboardPanel
    anchorItem: root.anchorItem || button
    bar: root.bar
    owner: root
    open: root.opened
    contentWidth: Style.space(420)
    contentHeight: Style.space(580)
    focusTarget: keyCatcher

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: tokenInput.activeFocus

        onCloseRequested: {
          if (root.selectedSpendingGroupId !== "") {
            root.selectedSpendingGroupId = ""
          } else if (root.showBudgetSelector) {
            root.showBudgetSelector = false
          } else if (root.showSettingsBudgetDropdown) {
            root.showSettingsBudgetDropdown = false
          } else if (root.showSettings) {
            root.showSettings = false
          } else {
            root.close()
          }
        }

        onTabRequested: function(direction) {
          if (!root.showSettings && root.authenticated) {
            var next = root.activeTab + direction
            if (next < 0) next = 2
            if (next > 2) next = 0
            root.activeTab = next
          }
        }

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          // Escape handling
          if (event.key === Qt.Key_Escape) {
            if (root.selectedSpendingGroupId !== "") {
              root.selectedSpendingGroupId = ""
              event.accepted = true
              return
            }
            if (root.showBudgetSelector) {
              root.showBudgetSelector = false
              event.accepted = true
              return
            }
            if (root.showSettingsBudgetDropdown) {
              root.showSettingsBudgetDropdown = false
              event.accepted = true
              return
            }
            if (root.showSettings) {
              root.showSettings = false
              event.accepted = true
              return
            }
            root.close()
            event.accepted = true
            return
          }

          // Alt Modifier Shortcuts
          if (event.modifiers & Qt.AltModifier) {
            // Alt+1 or Alt+B: Buckets tab
            if (event.key === Qt.Key_1 || event.key === Qt.Key_B) {
              root.activeTab = 0
              root.showSettings = false
              root.showBudgetSelector = false
              event.accepted = true
              return
            }
            // Alt+2 or Alt+I: Income & Age tab
            if (event.key === Qt.Key_2 || event.key === Qt.Key_I) {
              root.activeTab = 1
              root.showSettings = false
              root.showBudgetSelector = false
              event.accepted = true
              return
            }
            // Alt+3 or Alt+P: Spending Analysis tab
            if (event.key === Qt.Key_3 || event.key === Qt.Key_P) {
              root.activeTab = 2
              root.showSettings = false
              root.showBudgetSelector = false
              event.accepted = true
              return
            }
            // Alt+D: Drilldown into first category group in Tab 2
            if (event.key === Qt.Key_D) {
              if (root.activeTab === 2) {
                if (root.selectedSpendingGroupId === "" && root.overviewData && root.overviewData.spending_pie_chart && root.overviewData.spending_pie_chart.length > 1) {
                  root.selectedSpendingGroupId = root.overviewData.spending_pie_chart[1].group_id
                } else {
                  root.selectedSpendingGroupId = ""
                }
                event.accepted = true
                return
              }
            }
            // Alt+S or Alt+,: Toggle Settings
            if (event.key === Qt.Key_S || event.key === Qt.Key_Comma) {
              root.showSettings = !root.showSettings
              root.showBudgetSelector = false
              event.accepted = true
              return
            }
            // Alt+M or Alt+U: Toggle Budget Switcher
            if (event.key === Qt.Key_M || event.key === Qt.Key_U) {
              root.showBudgetSelector = !root.showBudgetSelector
              event.accepted = true
              return
            }
            // Alt+R: Force Refresh
            if (event.key === Qt.Key_R) {
              root.forceRefresh()
              event.accepted = true
              return
            }
            // Alt+W or Alt+O: Open Web App
            if (event.key === Qt.Key_W || event.key === Qt.Key_O) {
              root.openWebApp()
              event.accepted = true
              return
            }
            // Alt+D: Open Developer Settings (on onboarding)
            if (event.key === Qt.Key_D) {
              root.openDeveloperSettings()
              event.accepted = true
              return
            }
            // Alt+C: Connect / Save Token
            if (event.key === Qt.Key_C) {
              root.saveToken()
              event.accepted = true
              return
            }
            // Alt+X or Alt+Delete: Clear / Revoke Token
            if (event.key === Qt.Key_X || event.key === Qt.Key_Delete) {
              if (root.showSettings) {
                root.clearToken()
                event.accepted = true
                return
              }
            }
          }
        }

        ColumnLayout {
          anchors.fill: parent
          anchors.margins: Style.spacing.popupPadding
          spacing: Style.space(8)

          // ================= TOP HEADER =================
          RowLayout {
            Layout.fillWidth: true
            spacing: Style.space(6)

            RowLayout {
              spacing: Style.space(4)
              Text {
                textFormat: Text.PlainText
                text: root.iconGlyph
                color: Color.accent
                font.family: root.fontFamily
                font.pixelSize: Style.font.subtitle
                renderType: Text.NativeRendering
              }

              Text {
                textFormat: Text.PlainText
                text: "YNAB Pulse"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.subtitle
                font.bold: true
              }
            }

            Item { Layout.fillWidth: true }

            // Budget Switcher Dropdown Button
            Button {
              visible: root.authenticated && root.overviewData && root.overviewData.budgets && root.overviewData.budgets.length > 1
              text: Model.plainLabel((root.overviewData && root.overviewData.active_budget_name ? root.overviewData.active_budget_name : "Budget") + " 󰁥")
              tooltipText: "Switch Budget (Alt+M)"
              selected: root.showBudgetSelector
              onClicked: root.showBudgetSelector = !root.showBudgetSelector
            }

            // Web App Launch Button
            Button {
              text: "󰖟"
              tooltipText: "Open YNAB Web App in Browser (Alt+W)"
              onClicked: root.openWebApp()
              visible: root.authenticated
            }

            // Refresh Button
            Button {
              text: root.loading ? "…" : "󰑐"
              tooltipText: "Refresh budget data (Alt+R)"
              onClicked: root.forceRefresh()
              visible: root.authenticated
            }

            // Settings Button
            Button {
              text: "󰒓"
              tooltipText: root.showSettings ? "Exit Settings (Alt+S or Esc)" : "Settings (Alt+S)"
              selected: root.showSettings
              onClicked: {
                root.showSettings = !root.showSettings
                root.showBudgetSelector = false
              }
            }
          }

          // ================= HEADER BUDGET SWITCHER DROPDOWN =================
          BorderSurface {
            Layout.fillWidth: true
            visible: root.showBudgetSelector
            color: Color.popups.background
            radius: Style.cornerRadius
            borderSpec: Border.flat(Color.accent, 1)
            implicitHeight: budgetCol.implicitHeight + Style.space(16)

            ColumnLayout {
              id: budgetCol
              anchors.fill: parent
              anchors.margins: Style.space(12)
              spacing: Style.space(6)

              Text {
                textFormat: Text.PlainText
                text: "Select YNAB Budget:"
                color: root.foreground
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                font.bold: true
              }

              Repeater {
                model: root.overviewData && root.overviewData.budgets ? root.overviewData.budgets : []

                delegate: Button {
                  Layout.fillWidth: true
                  leftAlign: true
                  text: Model.plainLabel(modelData.name + (modelData.id === (root.overviewData ? root.overviewData.active_budget_id : "") ? "  ✓ (Active)" : ""))
                  selected: modelData.id === (root.overviewData ? root.overviewData.active_budget_id : "")
                  onClicked: root.selectBudget(modelData.id)
                }
              }
            }
          }

          // ================= ERROR BANNER =================
          BorderSurface {
            Layout.fillWidth: true
            visible: root.statusError !== ""
            color: Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.15)
            radius: Style.cornerRadius
            borderSpec: Border.flat(root.urgent, 1)
            implicitHeight: errorText.implicitHeight + Style.space(12)

            Text {
              id: errorText
              textFormat: Text.PlainText
              anchors.fill: parent
              anchors.margins: Style.space(6)
              text: root.statusError
              color: root.urgent
              font.family: root.fontFamily
              font.pixelSize: Style.font.caption
              wrapMode: Text.WordWrap
            }
          }

          // ================= ONBOARDING VIEW (UNAUTHENTICATED) =================
          ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: !root.authenticated && !root.showSettings
            spacing: Style.space(12)

            BorderSurface {
              Layout.fillWidth: true
              Layout.fillHeight: true
              color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
              radius: Style.cornerRadius
              borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

              ColumnLayout {
                anchors.fill: parent
                anchors.margins: Style.space(16)
                spacing: Style.space(12)

                Text {
                  textFormat: Text.PlainText
                  text: "Connect your YNAB Account"
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.subtitle
                  font.bold: true
                }

                Text {
                  textFormat: Text.PlainText
                  text: "To display your budget buckets, age of money, and spending charts, please generate a Personal Access Token from your YNAB Developer Settings."
                  color: Qt.darker(root.foreground, 1.4)
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.body
                  wrapMode: Text.WordWrap
                  Layout.fillWidth: true
                }

                Button {
                  text: "󰖟 Open YNAB Developer Settings"
                  tooltipText: "Open developer token page (Alt+D)"
                  onClicked: root.openDeveloperSettings()
                }

                PanelSeparator { Layout.fillWidth: true }

                Text {
                  textFormat: Text.PlainText
                  text: "Personal Access Token:"
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.caption
                  font.bold: true
                }

                TextField {
                  id: tokenInput
                  Layout.fillWidth: true
                  placeholderText: "Paste your YNAB API Token here..."
                  text: root.tokenDraft
                  echoMode: TextInput.Password
                  onTextChanged: root.tokenDraft = text
                  Keys.onReturnPressed: root.saveToken()
                  Keys.onEscapePressed: {
                    if (root.showSettings) root.showSettings = false
                    else root.close()
                  }
                }

                RowLayout {
                  Layout.fillWidth: true
                  spacing: Style.space(8)

                  Button {
                    text: root.loading ? "Connecting..." : "Save & Connect"
                    tooltipText: "Save token to keyring (Alt+C or Enter)"
                    enabled: !root.loading && root.tokenDraft.trim() !== ""
                    onClicked: root.saveToken()
                  }
                }

                Item { Layout.fillHeight: true }
              }
            }
          }

          // ================= SETTINGS VIEW (BEAUTIFUL GROUP CARDS) =================
          Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.showSettings

            ScrollView {
              id: settingsScroll
              anchors.fill: parent
              clip: true
              ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

              Column {
                id: settingsColumn
                width: settingsScroll.width - Style.space(16)
                spacing: Style.space(12)

                // Top Settings Title
                Text {
                  textFormat: Text.PlainText
                  text: "YNAB Pulse Settings"
                  color: root.foreground
                  font.family: root.fontFamily
                  font.pixelSize: Style.font.subtitle
                  font.bold: true
                  topPadding: Style.space(2)
                  leftPadding: Style.space(4)
                }

                // ---------------- GROUP 1: ACTIVE BUDGET ----------------
                BorderSurface {
                  width: parent.width
                  implicitHeight: g1Col.implicitHeight + Style.space(24)
                  color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                  radius: Style.cornerRadius
                  borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

                  Column {
                    id: g1Col
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Style.space(12)
                    spacing: Style.space(8)

                    Text {
                      textFormat: Text.PlainText
                      text: "Active Budget"
                      color: root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.body
                      font.bold: true
                    }

                    Text {
                      textFormat: Text.PlainText
                      text: "Choose which YNAB budget is tracked in your status bar and overview."
                      color: Qt.darker(root.foreground, 1.4)
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                      width: parent.width
                      wrapMode: Text.WordWrap
                    }

                    Button {
                      width: parent.width
                      leftAlign: true
                      text: Model.plainLabel((root.overviewData && root.overviewData.active_budget_name ? root.overviewData.active_budget_name : "Select Budget") + "  " + (root.showSettingsBudgetDropdown ? "󰅀" : "󰅂"))
                      selected: root.showSettingsBudgetDropdown
                      onClicked: root.showSettingsBudgetDropdown = !root.showSettingsBudgetDropdown
                    }

                    Column {
                      width: parent.width
                      visible: root.showSettingsBudgetDropdown
                      spacing: Style.space(4)

                      Repeater {
                        model: root.overviewData && root.overviewData.budgets ? root.overviewData.budgets : []

                        delegate: Button {
                          width: parent.width
                          leftAlign: true
                          text: Model.plainLabel(modelData.name + (modelData.id === (root.overviewData ? root.overviewData.active_budget_id : "") ? "  ✓" : ""))
                          selected: modelData.id === (root.overviewData ? root.overviewData.active_budget_id : "")
                          onClicked: {
                            root.selectBudget(modelData.id)
                            root.showSettingsBudgetDropdown = false
                          }
                        }
                      }
                    }
                  }
                }

                // ---------------- GROUP 2: BACKGROUND REFRESH (SLIDER) ----------------
                BorderSurface {
                  width: parent.width
                  implicitHeight: g2Col.implicitHeight + Style.space(24)
                  color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                  radius: Style.cornerRadius
                  borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

                  Column {
                    id: g2Col
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Style.space(12)
                    spacing: Style.space(8)

                    RowLayout {
                      width: parent.width

                      Text {
                        textFormat: Text.PlainText
                        text: "Background Refresh"
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.body
                        font.bold: true
                        Layout.fillWidth: true
                      }

                      Text {
                        textFormat: Text.PlainText
                        text: root.runtimeRefreshHours + " hour" + (root.runtimeRefreshHours === 1 ? "" : "s") + (root.runtimeRefreshHours === 24 ? " (1 day)" : (root.runtimeRefreshHours === 48 ? " (2 days)" : (root.runtimeRefreshHours === 72 ? " (3 days)" : "")))
                        color: Color.accent
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.body
                        font.bold: true
                      }
                    }

                    Text {
                      textFormat: Text.PlainText
                      text: "Frequency at which YNAB Pulse fetches updated balances in the background (configured in hours)."
                      color: Qt.darker(root.foreground, 1.4)
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                      width: parent.width
                      wrapMode: Text.WordWrap
                    }

                    PanelSlider {
                      width: parent.width
                      bar: root.bar
                      minimum: 1
                      maximum: 72
                      step: 1
                      integer: true
                      value: root.runtimeRefreshHours
                      onMoved: function(val) { root.runtimeRefreshHours = Math.round(val) }
                      onReleased: function(val) { root.setRefreshHours(Math.round(val)) }
                    }

                    RowLayout {
                      width: parent.width

                      Text {
                        textFormat: Text.PlainText
                        text: "1 hour (Frequent)"
                        color: Qt.darker(root.foreground, 1.5)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                      }

                      Item { Layout.fillWidth: true }

                      Text {
                        textFormat: Text.PlainText
                        text: "72 hours (3 Days)"
                        color: Qt.darker(root.foreground, 1.5)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                      }
                    }
                  }
                }

                // ---------------- GROUP 3: API TOKEN & SECURITY ----------------
                BorderSurface {
                  width: parent.width
                  implicitHeight: g3Col.implicitHeight + Style.space(24)
                  color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                  radius: Style.cornerRadius
                  borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

                  Column {
                    id: g3Col
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Style.space(12)
                    spacing: Style.space(8)

                    Text {
                      textFormat: Text.PlainText
                      text: "YNAB API Token & Security"
                      color: root.foreground
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.body
                      font.bold: true
                    }

                    Text {
                      textFormat: Text.PlainText
                      text: "Your token is encrypted with a key held on this machine, then stored in your Linux Secret Service keyring."
                      color: Qt.darker(root.foreground, 1.4)
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.caption
                      width: parent.width
                      wrapMode: Text.WordWrap
                    }

                    RowLayout {
                      width: parent.width
                      spacing: Style.space(8)

                      Button {
                        text: "󰖟 Developer Settings"
                        tooltipText: "Open YNAB developer token settings in browser"
                        onClicked: root.openDeveloperSettings()
                      }

                      Button {
                        text: "Revoke Stored Token"
                        tooltipText: "Remove PAT from keyring (Alt+X)"
                        onClicked: root.clearToken()
                      }
                    }
                  }
                }

                // ---------------- GROUP 4: HELP & KEYBOARD SHORTCUTS ----------------
                BorderSurface {
                  width: parent.width
                  implicitHeight: g4Col.implicitHeight + Style.space(24)
                  color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                  radius: Style.cornerRadius
                  borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

                  Column {
                    id: g4Col
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Style.space(12)
                    spacing: Style.space(8)

                    MouseArea {
                      width: parent.width
                      implicitHeight: helpHeaderRow.implicitHeight
                      cursorShape: Qt.PointingHandCursor
                      onClicked: root.showShortcutsHelp = !root.showShortcutsHelp

                      RowLayout {
                        id: helpHeaderRow
                        anchors.fill: parent

                        Text {
                          textFormat: Text.PlainText
                          text: "󰋖 Help & Keyboard Shortcuts"
                          color: root.foreground
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          font.bold: true
                          Layout.fillWidth: true
                        }

                        Text {
                          textFormat: Text.PlainText
                          text: root.showShortcutsHelp ? "󰅀" : "󰅂"
                          color: Color.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          font.bold: true
                        }
                      }
                    }

                    Column {
                      width: parent.width
                      visible: root.showShortcutsHelp
                      spacing: Style.space(4)

                      Text {
                        textFormat: Text.PlainText
                        text: "• Alt+1 / Alt+B : Switch to Buckets tab\n• Alt+2 / Alt+I : Switch to Income & Age tab\n• Alt+3 / Alt+P : Switch to Spending Analysis tab\n• Alt+M         : Open Budget Switcher dropdown\n• Alt+R         : Force refresh data from YNAB\n• Alt+W         : Open YNAB web app in browser\n• Alt+S / Esc   : Toggle / exit settings\n• Esc           : Close panel popup"
                        color: Qt.darker(root.foreground, 1.4)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                        width: parent.width
                      }
                    }
                  }
                }

                Item { width: parent.width; height: Style.space(8) }
              }
            }
          }

          // ================= MAIN TABS VIEW (AUTHENTICATED) =================
          ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.authenticated && !root.showSettings
            spacing: Style.space(8)

            // ---------------- TAB 0: BUCKETS ----------------
            Item {
              Layout.fillWidth: true
              Layout.fillHeight: true
              visible: root.activeTab === 0

              ColumnLayout {
                anchors.fill: parent
                spacing: Style.space(8)

                // 1. READY TO ASSIGN BANNER
                BorderSurface {
                  Layout.fillWidth: true
                  implicitHeight: rtaRow.implicitHeight + Style.space(20)
                  color: {
                    if (!root.overviewData) return Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                    if (root.overviewData.ready_to_assign_status === "positive") return Qt.rgba(0.06, 0.72, 0.50, 0.14)
                    if (root.overviewData.ready_to_assign_status === "negative") return Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.15)
                    return Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                  }
                  radius: Style.cornerRadius
                  borderSpec: Border.flat(
                    root.overviewData && root.overviewData.ready_to_assign_status === "positive"
                      ? "#10b981"
                      : (root.overviewData && root.overviewData.ready_to_assign_status === "negative" ? root.urgent : Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)),
                    1
                  )

                  RowLayout {
                    id: rtaRow
                    anchors.fill: parent
                    anchors.margins: Style.space(12)
                    spacing: Style.space(10)

                    Text {
                      textFormat: Text.PlainText
                      text: root.overviewData && root.overviewData.ready_to_assign_status === "negative" ? "󰀦" : root.iconGlyph
                      color: root.overviewData && root.overviewData.ready_to_assign_status === "positive"
                        ? "#10b981"
                        : (root.overviewData && root.overviewData.ready_to_assign_status === "negative" ? root.urgent : root.foreground)
                      font.family: root.fontFamily
                      font.pixelSize: Style.space(22)
                      renderType: Text.NativeRendering
                      Layout.alignment: Qt.AlignVCenter
                    }

                    ColumnLayout {
                      Layout.fillWidth: true
                      Layout.alignment: Qt.AlignVCenter
                      spacing: Style.space(2)

                      Text {
                        textFormat: Text.PlainText
                        text: root.overviewData && root.overviewData.ready_to_assign_status === "negative"
                          ? "Over-Assigned Funds"
                          : "Ready to Assign"
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                        font.bold: true
                      }

                      Text {
                        textFormat: Text.PlainText
                        text: root.overviewData && root.overviewData.ready_to_assign_status === "positive"
                          ? "Give every dollar a job"
                          : (root.overviewData && root.overviewData.ready_to_assign_status === "negative"
                              ? "You assigned more money than you have!"
                              : "All funds are assigned")
                        color: Qt.darker(root.foreground, 1.4)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                      }
                    }

                    Text {
                      textFormat: Text.PlainText
                      text: root.overviewData ? root.overviewData.ready_to_assign_formatted : "$0.00"
                      color: root.overviewData && root.overviewData.ready_to_assign_status === "positive"
                        ? "#10b981"
                        : (root.overviewData && root.overviewData.ready_to_assign_status === "negative" ? root.urgent : root.foreground)
                      font.family: root.fontFamily
                      font.pixelSize: Style.font.title
                      font.bold: true
                      Layout.alignment: Qt.AlignVCenter
                    }
                  }
                }

                // 2. UNAPPROVED TRANSACTIONS & OVERSPENT QUICK ALERTS
                RowLayout {
                  Layout.fillWidth: true
                  spacing: Style.space(8)
                  visible: root.overviewData && (root.overviewData.unapproved_transactions_count > 0 || root.overviewData.overspent_categories_count > 0)

                  // Unapproved Transactions Banner
                  BorderSurface {
                    Layout.fillWidth: true
                    visible: root.overviewData && root.overviewData.unapproved_transactions_count > 0
                    color: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.12)
                    radius: Style.cornerRadius
                    borderSpec: Border.flat(Color.accent, 1)
                    implicitHeight: Style.space(34)

                    MouseArea {
                      anchors.fill: parent
                      cursorShape: Qt.PointingHandCursor
                      onClicked: root.openWebApp()

                      RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: Style.space(10)
                        anchors.rightMargin: Style.space(10)
                        spacing: Style.space(6)

                        Text {
                          textFormat: Text.PlainText
                          text: "󰄬"
                          color: Color.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          renderType: Text.NativeRendering
                          Layout.alignment: Qt.AlignVCenter
                        }

                        Text {
                          textFormat: Text.PlainText
                          text: (root.overviewData ? root.overviewData.unapproved_transactions_count : 0) + " to review"
                          color: Color.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          font.bold: true
                          Layout.fillWidth: true
                          Layout.alignment: Qt.AlignVCenter
                        }

                        Text {
                          textFormat: Text.PlainText
                          text: "Open 󰖟"
                          color: Qt.darker(Color.accent, 1.2)
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          Layout.alignment: Qt.AlignVCenter
                        }
                      }
                    }
                  }

                  // Overspent Alert Banner
                  BorderSurface {
                    Layout.fillWidth: true
                    visible: root.overviewData && root.overviewData.overspent_categories_count > 0
                    color: Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.14)
                    radius: Style.cornerRadius
                    borderSpec: Border.flat(root.urgent, 1)
                    implicitHeight: Style.space(34)

                    RowLayout {
                      anchors.fill: parent
                      anchors.leftMargin: Style.space(10)
                      anchors.rightMargin: Style.space(10)
                      spacing: Style.space(6)

                      Text {
                        textFormat: Text.PlainText
                        text: "󰀦"
                        color: root.urgent
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.body
                        renderType: Text.NativeRendering
                        Layout.alignment: Qt.AlignVCenter
                      }

                      Text {
                        textFormat: Text.PlainText
                        text: (root.overviewData ? root.overviewData.overspent_categories_count : 0) + " category overspent"
                        color: root.urgent
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                        font.bold: true
                        Layout.fillWidth: true
                        Layout.alignment: Qt.AlignVCenter
                      }
                    }
                  }
                }

                // 3. COLLAPSE / EXPAND ALL ACTION BAR
                RowLayout {
                  Layout.fillWidth: true
                  spacing: Style.space(6)

                  Text {
                    textFormat: Text.PlainText
                    text: "Category Buckets"
                    color: root.foreground
                    font.family: root.fontFamily
                    font.pixelSize: Style.font.body
                    font.bold: true
                    Layout.fillWidth: true
                  }

                  Button {
                    text: root.allCollapsed ? "Expand All 󰅀" : "Collapse All 󰅂"
                    tooltipText: "Toggle all category groups"
                    onClicked: root.toggleAllCollapse()
                  }
                }

                // 4. SCROLLABLE CATEGORY GROUPS WITH COLLAPSIBLE HEADERS
                ScrollView {
                  Layout.fillWidth: true
                  Layout.fillHeight: true
                  clip: true

                  ColumnLayout {
                    width: parent.width - Style.space(16)
                    spacing: Style.space(10)

                    Repeater {
                      model: root.overviewData && root.overviewData.category_groups ? root.overviewData.category_groups : []

                      delegate: BorderSurface {
                        id: groupCard
                        Layout.fillWidth: true
                        implicitHeight: groupContent.implicitHeight + Style.space(16)
                        color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.03)
                        radius: Style.cornerRadius
                        borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08), 1)

                        ColumnLayout {
                          id: groupContent
                          anchors.fill: parent
                          anchors.margins: Style.space(10)
                          spacing: Style.space(8)

                          // Clickable Collapsible Group Header
                          MouseArea {
                            Layout.fillWidth: true
                            implicitHeight: groupHeaderRow.implicitHeight + Style.space(8)
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.toggleGroupCollapse(modelData.id)

                            RowLayout {
                              id: groupHeaderRow
                              anchors.fill: parent
                              spacing: Style.space(6)

                              Text {
                                textFormat: Text.PlainText
                                text: root.collapsedGroups[modelData.id] ? "󰅂" : "󰅀"
                                color: Color.accent
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.body
                                font.bold: true
                                Layout.alignment: Qt.AlignVCenter
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: modelData.name + " (" + modelData.categories.length + ")"
                                color: root.foreground
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.body
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                                Layout.alignment: Qt.AlignVCenter
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: modelData.balance_formatted
                                color: root.foreground
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                                font.bold: true
                                Layout.alignment: Qt.AlignVCenter
                              }
                            }
                          }

                          // Category items (collapsed when toggled)
                          ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Style.space(8)
                            visible: !root.collapsedGroups[modelData.id]

                            Repeater {
                              model: modelData.categories

                              delegate: BorderSurface {
                                Layout.fillWidth: true
                                implicitHeight: catCol.implicitHeight + Style.space(16)
                                color: modelData.is_overspent
                                  ? Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.10)
                                  : "transparent"
                                radius: Style.cornerRadius
                                borderSpec: Border.flat(
                                  modelData.is_overspent ? Qt.rgba(root.urgent.r, root.urgent.g, root.urgent.b, 0.5) : "transparent",
                                  modelData.is_overspent ? 1 : 0
                                )

                                ColumnLayout {
                                  id: catCol
                                  anchors.fill: parent
                                  anchors.leftMargin: Style.space(10)
                                  anchors.rightMargin: Style.space(10)
                                  anchors.topMargin: Style.space(8)
                                  anchors.bottomMargin: Style.space(8)
                                  spacing: Style.space(6)

                                  RowLayout {
                                    Layout.fillWidth: true

                                    Text {
                                      textFormat: Text.PlainText
                                      text: (modelData.is_overspent ? "󰀦 " : "") + modelData.name
                                      color: modelData.is_overspent ? root.urgent : root.foreground
                                      font.family: root.fontFamily
                                      font.pixelSize: Style.font.caption
                                      font.bold: modelData.is_overspent
                                      elide: Text.ElideRight
                                      Layout.fillWidth: true
                                      Layout.alignment: Qt.AlignVCenter
                                    }

                                    Text {
                                      textFormat: Text.PlainText
                                      text: modelData.is_overspent
                                        ? "Overspent: " + modelData.balance_formatted
                                        : modelData.balance_formatted
                                      color: Model.getStatusBadgeColor(modelData.status_color, Color)
                                      font.family: root.fontFamily
                                      font.pixelSize: Style.font.caption
                                      font.bold: true
                                      Layout.alignment: Qt.AlignVCenter
                                    }
                                  }

                                  YnabProgressBar {
                                    Layout.fillWidth: true
                                    fraction: modelData.progress_fraction
                                    statusColor: modelData.status_color
                                    barHeight: Style.space(5)
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }

            // ---------------- TAB 1: INCOME & AGE OF MONEY ----------------
            Item {
              Layout.fillWidth: true
              Layout.fillHeight: true
              visible: root.activeTab === 1

              ScrollView {
                anchors.fill: parent
                clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                ColumnLayout {
                  width: parent.width - Style.space(16)
                  spacing: Style.space(10)

                  // 1. Age of Money Card
                  BorderSurface {
                    Layout.fillWidth: true
                    implicitHeight: aomCol.implicitHeight + Style.space(24)
                    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                    radius: Style.cornerRadius
                    borderSpec: Border.flat(Color.accent, 1)

                    ColumnLayout {
                      id: aomCol
                      anchors.fill: parent
                      anchors.margins: Style.space(12)
                      spacing: Style.space(6)

                      RowLayout {
                        Layout.fillWidth: true

                        Text {
                          textFormat: Text.PlainText
                          text: "Age of Money"
                          color: Qt.darker(root.foreground, 1.3)
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          font.bold: true
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                          textFormat: Text.PlainText
                          text: root.overviewData && root.overviewData.age_of_money ? root.overviewData.age_of_money.status.toUpperCase() : ""
                          color: Color.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          font.bold: true
                        }
                      }

                      Text {
                        textFormat: Text.PlainText
                        text: root.overviewData && root.overviewData.age_of_money ? Model.formatAgeOfMoney(root.overviewData.age_of_money.days) : "--"
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.title
                        font.bold: true
                      }

                      Text {
                        textFormat: Text.PlainText
                        text: root.overviewData && root.overviewData.age_of_money ? root.overviewData.age_of_money.label : ""
                        color: Qt.darker(root.foreground, 1.4)
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.caption
                      }
                    }
                  }

                  // 2. Income vs Spending (Current Month) Card
                  BorderSurface {
                    Layout.fillWidth: true
                    implicitHeight: ivsCol.implicitHeight + Style.space(24)
                    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.03)
                    radius: Style.cornerRadius
                    borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

                    ColumnLayout {
                      id: ivsCol
                      anchors.fill: parent
                      anchors.margins: Style.space(12)
                      spacing: Style.space(10)

                      Text {
                        textFormat: Text.PlainText
                        text: "Income vs. Spending (Current Month)"
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.subtitle
                        font.bold: true
                      }

                      RowLayout {
                        Layout.fillWidth: true

                        ColumnLayout {
                          Layout.fillWidth: true
                          Text {
                            textFormat: Text.PlainText
                            text: "Total Inflow (Income)"
                            color: Qt.darker(root.foreground, 1.4)
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.caption
                          }
                          Text {
                            textFormat: Text.PlainText
                            text: root.overviewData && root.overviewData.income_vs_spending ? root.overviewData.income_vs_spending.income_formatted : "$0.00"
                            color: "#10b981"
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.body
                            font.bold: true
                          }
                        }

                        ColumnLayout {
                          Layout.fillWidth: true
                          Text {
                            textFormat: Text.PlainText
                            text: "Total Outflow (Spending)"
                            color: Qt.darker(root.foreground, 1.4)
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.caption
                          }
                          Text {
                            textFormat: Text.PlainText
                            text: root.overviewData && root.overviewData.income_vs_spending ? root.overviewData.income_vs_spending.spending_formatted : "$0.00"
                            color: root.urgent
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.body
                            font.bold: true
                          }
                        }
                      }

                      PanelSeparator { Layout.fillWidth: true }

                      RowLayout {
                        Layout.fillWidth: true

                        Text {
                          textFormat: Text.PlainText
                          text: "Net Savings:"
                          color: root.foreground
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          font.bold: true
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                          textFormat: Text.PlainText
                          text: root.overviewData && root.overviewData.income_vs_spending ? root.overviewData.income_vs_spending.net_formatted : "$0.00"
                          color: root.overviewData && root.overviewData.income_vs_spending && root.overviewData.income_vs_spending.is_positive ? "#10b981" : root.urgent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.body
                          font.bold: true
                        }
                      }

                      RowLayout {
                        Layout.fillWidth: true

                        Text {
                          textFormat: Text.PlainText
                          text: "Savings Rate:"
                          color: Qt.darker(root.foreground, 1.4)
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                        }

                        Item { Layout.fillWidth: true }

                        Text {
                          textFormat: Text.PlainText
                          text: (root.overviewData && root.overviewData.income_vs_spending ? root.overviewData.income_vs_spending.savings_rate_percent : 0) + "%"
                          color: Color.accent
                          font.family: root.fontFamily
                          font.pixelSize: Style.font.caption
                          font.bold: true
                        }
                      }
                    }
                  }

                  // 3. Multi-Month Income vs. Spending Trend Graph
                  BorderSurface {
                    Layout.fillWidth: true
                    implicitHeight: trendCol.implicitHeight + Style.space(24)
                    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.03)
                    radius: Style.cornerRadius
                    borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

                    ColumnLayout {
                      id: trendCol
                      anchors.fill: parent
                      anchors.margins: Style.space(12)
                      spacing: Style.space(8)

                      Text {
                        textFormat: Text.PlainText
                        text: "Income vs. Expenses Trend"
                        color: root.foreground
                        font.family: root.fontFamily
                        font.pixelSize: Style.font.subtitle
                        font.bold: true
                      }

                      TrendGraph {
                        Layout.fillWidth: true
                        trends: root.overviewData && root.overviewData.monthly_trends ? root.overviewData.monthly_trends : []
                        foreground: root.foreground
                        fontFamily: root.fontFamily
                      }
                    }
                  }

                  Item { implicitHeight: Style.space(8) }
                }
              }
            }

            // ---------------- TAB 2: SPENDING PIE CHART & DRILL-DOWN ----------------
            Item {
              Layout.fillWidth: true
              Layout.fillHeight: true
              visible: root.activeTab === 2

              ColumnLayout {
                anchors.fill: parent
                spacing: Style.space(8)

                // Interactive Pie Chart Visual
                PieChart {
                  Layout.alignment: Qt.AlignHCenter
                  slices: root.overviewData && root.overviewData.spending_pie_chart ? root.overviewData.spending_pie_chart : []
                  selectedGroupId: root.selectedSpendingGroupId
                  centerLabel: root.overviewData && root.overviewData.income_vs_spending ? root.overviewData.income_vs_spending.spending_formatted : "$0.00"
                  centerSublabel: "Total Spent"
                  onSliceClicked: function(idx, slice) {
                    root.selectedSpendingGroupId = (root.selectedSpendingGroupId === slice.group_id ? "" : slice.group_id)
                  }
                  onCenterClicked: {
                    root.selectedSpendingGroupId = ""
                  }
                }

                // Interactive Drill-Down or All Groups Legend
                ScrollView {
                  Layout.fillWidth: true
                  Layout.fillHeight: true
                  clip: true
                  ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

                  ColumnLayout {
                    width: parent.width - Style.space(16)
                    spacing: Style.space(6)

                    // Mode A: Detail View when a Group is Selected
                    ColumnLayout {
                      Layout.fillWidth: true
                      visible: root.selectedSpendingGroupId !== "" && root.selectedSpendingGroup !== null
                      spacing: Style.space(8)

                      // Drilldown Header & Back Navigation Card
                      BorderSurface {
                        Layout.fillWidth: true
                        implicitHeight: drillHeader.implicitHeight + Style.space(12)
                        color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
                        radius: Style.cornerRadius
                        borderSpec: Border.flat(Color.accent, 1)

                        RowLayout {
                          id: drillHeader
                          anchors.fill: parent
                          anchors.margins: Style.space(8)
                          spacing: Style.space(8)

                          Button {
                            text: "󰅁 All Groups"
                            tooltipText: "Return to all spending groups overview"
                            onClicked: root.selectedSpendingGroupId = ""
                          }

                          Text {
                            textFormat: Text.PlainText
                            text: root.selectedSpendingGroup ? root.selectedSpendingGroup.name : ""
                            color: root.foreground
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.body
                            font.bold: true
                            elide: Text.ElideRight
                            Layout.fillWidth: true
                          }

                          Text {
                            textFormat: Text.PlainText
                            text: root.selectedSpendingGroup ? root.selectedSpendingGroup.activity_formatted : ""
                            color: Color.urgent
                            font.family: root.fontFamily
                            font.pixelSize: Style.font.body
                            font.bold: true
                          }
                        }
                      }

                      // Subcategory detail items matching full-width overview cards
                      Repeater {
                        model: root.selectedSpendingGroup ? root.selectedSpendingGroup.categories : []

                        delegate: BorderSurface {
                          Layout.fillWidth: true
                          implicitHeight: subCatCol.implicitHeight + Style.space(16)
                          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.03)
                          radius: Style.cornerRadius
                          borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08), 1)

                          ColumnLayout {
                            id: subCatCol
                            anchors.fill: parent
                            anchors.margins: Style.space(10)
                            spacing: Style.space(6)

                            RowLayout {
                              Layout.fillWidth: true

                              Text {
                                textFormat: Text.PlainText
                                text: modelData.name
                                color: root.foreground
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.body
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: "Spent: " + modelData.activity_formatted
                                color: Color.urgent
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.body
                                font.bold: true
                              }
                            }

                            RowLayout {
                              Layout.fillWidth: true

                              Text {
                                textFormat: Text.PlainText
                                text: "Budgeted: " + modelData.budgeted_formatted
                                color: Qt.darker(root.foreground, 1.4)
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                              }

                              Item { Layout.fillWidth: true }

                              Text {
                                textFormat: Text.PlainText
                                text: "Balance: " + modelData.balance_formatted
                                color: Model.getStatusBadgeColor(modelData.status_color, Color)
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                                font.bold: true
                              }
                            }

                            YnabProgressBar {
                              Layout.fillWidth: true
                              fraction: modelData.progress_fraction
                              statusColor: modelData.status_color
                              barHeight: Style.space(5)
                            }
                          }
                        }
                      }
                    }

                    // Mode B: All Groups Overview List
                    ColumnLayout {
                      Layout.fillWidth: true
                      visible: root.selectedSpendingGroupId === ""
                      spacing: Style.space(6)

                      Repeater {
                        model: root.overviewData && root.overviewData.spending_pie_chart ? root.overviewData.spending_pie_chart : []

                        delegate: BorderSurface {
                          Layout.fillWidth: true
                          implicitHeight: sliceRow.implicitHeight + Style.space(12)
                          color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.03)
                          radius: Style.cornerRadius
                          borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08), 1)

                          MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.selectedSpendingGroupId = modelData.group_id

                            RowLayout {
                              id: sliceRow
                              anchors.fill: parent
                              anchors.margins: Style.space(8)
                              spacing: Style.space(8)

                              Rectangle {
                                width: Style.space(10)
                                height: Style.space(10)
                                radius: width / 2
                                color: Model.getSliceColor(modelData.slice_color_index, Color.accent)
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: modelData.group_name
                                color: root.foreground
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                                font.bold: true
                                elide: Text.ElideRight
                                Layout.fillWidth: true
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: modelData.percentage + "%"
                                color: Qt.darker(root.foreground, 1.4)
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: modelData.amount_formatted
                                color: root.foreground
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                                font.bold: true
                              }

                              Text {
                                textFormat: Text.PlainText
                                text: "󰅂"
                                color: Qt.darker(root.foreground, 1.5)
                                font.family: root.fontFamily
                                font.pixelSize: Style.font.caption
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }

            // ================= BOTTOM TAB NAVIGATION BAR =================
            BorderSurface {
              Layout.fillWidth: true
              implicitHeight: tabRow.implicitHeight + Style.space(8)
              color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.04)
              radius: Style.cornerRadius
              borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.1), 1)

              RowLayout {
                id: tabRow
                anchors.centerIn: parent
                spacing: Style.space(6)

                Button {
                  text: "Buckets"
                  tooltipText: "Budget Buckets (Alt+1 or Alt+B)"
                  selected: root.activeTab === 0
                  onClicked: root.activeTab = 0
                }

                Button {
                  text: "Income & Age"
                  tooltipText: "Income vs Spending & Age of Money (Alt+2 or Alt+I)"
                  selected: root.activeTab === 1
                  onClicked: root.activeTab = 1
                }

                Button {
                  text: "Spending Analysis"
                  tooltipText: "Spending Pie Chart & Breakdown (Alt+3 or Alt+P)"
                  selected: root.activeTab === 2
                  onClicked: root.activeTab = 2
                }
              }
            }
          }
        }
      }
    }
  }
