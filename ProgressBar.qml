import QtQuick
import qs.Commons
import "Model.js" as Model

Item {
  id: root

  property real fraction: 0.0
  property string statusColor: "green"
  property int barHeight: Style.space(6)
  property color foreground: Color.foreground

  implicitWidth: Style.space(120)
  implicitHeight: barHeight

  readonly property color fillColor: Model.getStatusBadgeColor(statusColor, Color)

  Rectangle {
    id: backgroundTrack
    anchors.fill: parent
    radius: height / 2
    color: Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.12)

    Rectangle {
      id: fillIndicator
      anchors.left: parent.left
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      radius: parent.radius
      width: Math.max(0, Math.min(parent.width, parent.width * Math.max(0.0, Math.min(1.0, root.fraction))))
      color: root.fillColor

      Behavior on width {
        NumberAnimation { duration: 250; easing.type: Easing.OutCubic }
      }
      Behavior on color {
        ColorAnimation { duration: 200 }
      }
    }
  }
}
