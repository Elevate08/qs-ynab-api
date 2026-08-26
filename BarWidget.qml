import QtQuick
import Quickshell
import qs.Commons
import qs.Ui
import "Model.js" as Model

BarWidget {
  id: root
  moduleName: "io.github.elevate08.ynab-glance"

  readonly property var service: bar?.shell?.serviceFor("io.github.elevate08.ynab-glance")
  readonly property var overviewData: service ? service.overviewData : null
  readonly property bool authenticated: service ? service.authenticated : false
  readonly property string iconGlyph: setting("iconGlyph", "\uf0d6")

  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false
  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
    if ("service" in target) target.service = root.service
  }

  function refresh() {
    if (service) service.refresh()
  }

  function forceRefresh() {
    if (service) service.forceRefresh()
  }

  function togglePanel() {
    if (panelLoader.item && panelLoader.item.toggle) panelLoader.item.toggle()
  }

  function open() {
    if (panelLoader.item && panelLoader.item.openFromHotkey) panelLoader.item.openFromHotkey()
    else if (panelLoader.item && panelLoader.item.open) panelLoader.item.open()
  }

  function close() {
    if (panelLoader.item && panelLoader.item.close) panelLoader.item.close()
  }

  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  visible: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()
  onServiceChanged: injectPanel()

  Connections {
    target: root.service
    ignoreUnknownSignals: true
    function onRequestOpen() { root.open() }
    function onRequestClose() { root.close() }
    function onRequestToggle() { root.togglePanel() }
    function onRequestTab(index) {
      var p = panelLoader.item
      if (p) {
        p.showSettings = false
        p.selectedSpendingGroupId = ""
        p.activeTab = Math.max(0, Math.min(2, index))
        root.open()
      }
    }
    function onRequestSettings() {
      var p = panelLoader.item
      if (p) {
        p.showSettings = true
        p.showBudgetSelector = false
        p.showSettingsBudgetDropdown = false
        root.open()
      }
    }
    function onRequestDrilldown(groupId) {
      var p = panelLoader.item
      if (p) {
        p.showSettings = false
        p.activeTab = 2
        if (groupId) p.selectedSpendingGroupId = groupId
        root.open()
      }
    }
  }

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: root.injectPanel()
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.iconGlyph
    slotSize: Style.bar.statusSlot
    tooltipText: root.authenticated ? "YNAB Pulse" : "YNAB Pulse (Setup Required)"

    onPressed: function(b) {
      if (!root.bar) return
      if (b === Qt.RightButton) {
        if (root.overviewData) {
          var aom = root.overviewData.age_of_money ? root.overviewData.age_of_money.days : 0
          var rta = root.overviewData.ready_to_assign_formatted || "$0"
          Model.sendNotification(Quickshell, "YNAB Pulse", "Ready to Assign: " + rta + " | Age of Money: " + aom + "d")
        } else {
          root.togglePanel()
        }
      } else if (b === Qt.MiddleButton) {
        root.forceRefresh()
      } else {
        root.togglePanel()
      }
    }
  }
}
