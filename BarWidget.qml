import QtQuick
import qs.Commons
import qs.Ui

BarWidget {
  id: root
  moduleName: "io.github.elevate08.ynab-glance"

  function injectPanel() {
    var target = panelLoader.item
    if (!target) return
    if ("bar" in target) target.bar = root.bar
    if ("settings" in target) target.settings = root.settings
    if ("anchorItem" in target) target.anchorItem = button
    if ("hostWidget" in target) target.hostWidget = root
  }

  function refresh() {
    if (panelLoader.item && panelLoader.item.refresh) panelLoader.item.refresh()
  }

  function togglePanel() {
    if (panelLoader.item && panelLoader.item.toggle) panelLoader.item.toggle()
  }

  readonly property bool opened: panelLoader.item ? panelLoader.item.opened === true : false

  function open() {
    if (panelLoader.item && panelLoader.item.openFromHotkey) panelLoader.item.openFromHotkey()
    else if (panelLoader.item && panelLoader.item.open) panelLoader.item.open()
  }

  function close() {
    if (panelLoader.item && panelLoader.item.close) panelLoader.item.close()
  }

  readonly property bool popoutSwitchClosing: panelLoader.item ? panelLoader.item.popoutSwitchClosing === true : false

  function closeForPopoutSwitch() {
    if (panelLoader.item) panelLoader.item.closeForPopoutSwitch()
  }

  visible: true
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  onBarChanged: injectPanel()
  onSettingsChanged: injectPanel()

  Loader {
    id: panelLoader
    active: true
    source: Qt.resolvedUrl("Panel.qml")
    visible: false
    onLoaded: {
      root.injectPanel()
      Qt.callLater(root.injectPanel)
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    slotSize: Style.bar.statusSlot
    tooltipText: "YNAB Pulse"

    contentItem: Row {
      anchors.centerIn: parent
      spacing: Style.space(4)

      YnabIcon {
        anchors.verticalCenter: parent.verticalCenter
        size: Style.space(16)
        color: root.opened ? Color.accent : root.barForeground
      }

      Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: panelLoader.item && panelLoader.item.barLabel !== "" && root.setting("showAgeOfMoneyInBar", true)
        text: panelLoader.item ? panelLoader.item.barLabel : ""
        color: root.opened ? Color.accent : root.barForeground
        font.family: root.bar ? root.bar.fontFamily : Style.font.family
        font.pixelSize: Style.font.caption
        font.bold: true
      }
    }

    onPressed: function(b) {
      if (!root.bar) return
      if (b === Qt.RightButton) {
        if (panelLoader.item && panelLoader.item.overviewData) {
          var aom = panelLoader.item.overviewData.age_of_money ? panelLoader.item.overviewData.age_of_money.days : 0
          var net = panelLoader.item.overviewData.income_vs_spending ? panelLoader.item.overviewData.income_vs_spending.net_formatted : "$0"
          root.bar.run("omarchy-notification-send --app-name \"YNAB Pulse\" \"YNAB Pulse\" \"Age of Money: " + aom + " days | Net: " + net + "\"")
        }
      } else if (b === Qt.MiddleButton) {
        root.refresh()
      } else {
        root.togglePanel()
      }
    }
  }
}
