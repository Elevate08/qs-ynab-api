import QtQuick
import QtQuick.Layouts
import qs.Commons
import qs.Ui

Item {
  id: root

  property var trends: []
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property int hoveredIndex: -1

  implicitWidth: Style.space(380)
  implicitHeight: Style.space(160)

  readonly property real maxAmount: {
    if (!trends || trends.length === 0) return 1
    var maxVal = 0
    for (var i = 0; i < trends.length; i++) {
      var inc = trends[i].income_milliunits || 0
      var spd = trends[i].spending_milliunits || 0
      if (inc > maxVal) maxVal = inc
      if (spd > maxVal) maxVal = spd
    }
    return maxVal > 0 ? maxVal : 1
  }

  ColumnLayout {
    anchors.fill: parent
    spacing: Style.space(6)

    // Chart Header with Legend and Active Tooltip
    RowLayout {
      Layout.fillWidth: true

      // Legend
      RowLayout {
        spacing: Style.space(8)

        RowLayout {
          spacing: Style.space(4)
          Rectangle {
            width: Style.space(8)
            height: Style.space(8)
            radius: width / 2
            color: "#10b981"
          }
          Text {
            text: "Income"
            color: Qt.darker(root.foreground, 1.3)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }

        RowLayout {
          spacing: Style.space(4)
          Rectangle {
            width: Style.space(8)
            height: Style.space(8)
            radius: width / 2
            color: Color.urgent
          }
          Text {
            text: "Spending"
            color: Qt.darker(root.foreground, 1.3)
            font.family: root.fontFamily
            font.pixelSize: Style.font.caption
          }
        }
      }

      Item { Layout.fillWidth: true }

      // Hovered Month Detail Readout
      Text {
        text: {
          if (root.hoveredIndex >= 0 && root.hoveredIndex < root.trends.length) {
            var item = root.trends[root.hoveredIndex]
            return item.month_label + ": Net " + item.net_formatted + " (" + item.savings_rate_percent + "%)"
          }
          return "6-Month Trend"
        }
        color: {
          if (root.hoveredIndex >= 0 && root.hoveredIndex < root.trends.length) {
            return root.trends[root.hoveredIndex].is_positive ? "#10b981" : Color.urgent
          }
          return Qt.darker(root.foreground, 1.4)
        }
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: root.hoveredIndex >= 0
      }
    }

    // Graph Bars Area
    BorderSurface {
      Layout.fillWidth: true
      Layout.fillHeight: true
      color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.02)
      radius: Style.cornerRadius
      borderSpec: Border.flat(Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08), 1)

      RowLayout {
        anchors.fill: parent
        anchors.margins: Style.space(8)
        spacing: Style.space(6)

        Repeater {
          model: root.trends ? root.trends : []

          delegate: Item {
            id: monthBarCol
            Layout.fillWidth: true
            Layout.fillHeight: true

            readonly property bool isHovered: root.hoveredIndex === index
            readonly property real incFraction: Math.max(0.02, Math.min(1.0, (modelData.income_milliunits || 0) / root.maxAmount))
            readonly property real spdFraction: Math.max(0.02, Math.min(1.0, (modelData.spending_milliunits || 0) / root.maxAmount))

            ColumnLayout {
              anchors.fill: parent
              spacing: Style.space(4)

              // Bars Container
              Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                Row {
                  anchors.bottom: parent.bottom
                  anchors.horizontalCenter: parent.horizontalCenter
                  spacing: Style.space(3)
                  height: parent.height

                  // Income Bar (Green)
                  Rectangle {
                    anchors.bottom: parent.bottom
                    width: Math.max(Style.space(8), Math.min(Style.space(16), (monthBarCol.width - Style.space(12)) / 2))
                    height: Math.max(Style.space(4), parent.height * monthBarCol.incFraction)
                    radius: Style.space(2)
                    color: "#10b981"
                    opacity: monthBarCol.isHovered ? 1.0 : 0.85
                  }

                  // Spending Bar (Red)
                  Rectangle {
                    anchors.bottom: parent.bottom
                    width: Math.max(Style.space(8), Math.min(Style.space(16), (monthBarCol.width - Style.space(12)) / 2))
                    height: Math.max(Style.space(4), parent.height * monthBarCol.spdFraction)
                    radius: Style.space(2)
                    color: Color.urgent
                    opacity: monthBarCol.isHovered ? 1.0 : 0.85
                  }
                }
              }

              // Month Label
              Text {
                text: modelData.month_label
                color: monthBarCol.isHovered ? Color.accent : Qt.darker(root.foreground, 1.4)
                font.family: root.fontFamily
                font.pixelSize: Style.font.caption
                font.bold: monthBarCol.isHovered
                Layout.alignment: Qt.AlignHCenter
              }
            }

            MouseArea {
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onEntered: root.hoveredIndex = index
              onExited: {
                if (root.hoveredIndex === index) root.hoveredIndex = -1
              }
            }
          }
        }
      }
    }
  }
}
