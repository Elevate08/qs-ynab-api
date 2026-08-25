import QtQuick
import qs.Commons

Item {
  id: root

  property color color: Color.foreground
  property real size: Style.bar.iconCanvas

  implicitWidth: size
  implicitHeight: size
  width: size
  height: size

  Item {
    anchors.centerIn: parent
    width: Math.max(10, Math.round(root.width * 0.68))
    height: Math.max(14, Math.round(root.height * 0.96))

    // Outer Vertical Bill Border
    Rectangle {
      anchors.fill: parent
      radius: Math.max(2, Style.space(2))
      color: "transparent"
      border.width: Math.max(1.5, Math.round(parent.width * 0.10))
      border.color: root.color

      // Top Security Band
      Rectangle {
        anchors.top: parent.top
        anchors.topMargin: Math.max(2, Math.round(parent.height * 0.10))
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.round(parent.width * 0.65)
        height: Math.max(1, Math.round(parent.height * 0.05))
        color: root.color
        opacity: 0.5
      }

      // Center Currency Circle
      Rectangle {
        anchors.centerIn: parent
        width: Math.max(8, Math.round(parent.width * 0.68))
        height: width
        radius: width / 2
        color: "transparent"
        border.width: Math.max(1, Math.round(parent.width * 0.08))
        border.color: root.color

        // Dollar Sign ($)
        Text {
          textFormat: Text.PlainText
          anchors.centerIn: parent
          anchors.verticalCenterOffset: -Math.max(0.5, parent.height * 0.04)
          text: "$"
          color: root.color
          font.family: Style.font.family
          font.pixelSize: Math.max(8, Math.round(parent.height * 0.82))
          font.bold: true
          renderType: Text.NativeRendering
        }
      }

      // Bottom Security Band
      Rectangle {
        anchors.bottom: parent.bottom
        anchors.bottomMargin: Math.max(2, Math.round(parent.height * 0.10))
        anchors.horizontalCenter: parent.horizontalCenter
        width: Math.round(parent.width * 0.65)
        height: Math.max(1, Math.round(parent.height * 0.05))
        color: root.color
        opacity: 0.5
      }
    }
  }
}
