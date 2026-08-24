import QtQuick
import qs.Commons
import "Model.js" as Model

Item {
  id: root

  property var slices: []
  property string selectedGroupId: ""
  property string centerLabel: ""
  property string centerSublabel: "Total Spent"
  property color foreground: Color.foreground
  property string fontFamily: Style.font.family
  property int hoveredIndex: -1

  signal sliceHovered(int index, var sliceData)
  signal sliceClicked(int index, var sliceData)
  signal centerClicked()

  implicitWidth: Style.space(220)
  implicitHeight: Style.space(220)

  // Computed center labels based on selected group or hovered slice
  readonly property var activeSlice: {
    if (root.hoveredIndex >= 0 && root.hoveredIndex < root.slices.length) {
      return root.slices[root.hoveredIndex]
    }
    if (root.selectedGroupId !== "") {
      for (var i = 0; i < root.slices.length; i++) {
        if (root.slices[i].group_id === root.selectedGroupId) return root.slices[i]
      }
    }
    return null
  }

  readonly property string displayLabel: activeSlice ? activeSlice.amount_formatted : (root.centerLabel !== "" ? root.centerLabel : "$0.00")
  readonly property string displaySublabel: activeSlice ? activeSlice.group_name : root.centerSublabel

  Canvas {
    id: canvas
    anchors.fill: parent
    antialiasing: true

    onPaint: {
      var ctx = getContext("2d");
      ctx.reset();
      ctx.clearRect(0, 0, width, height);

      var cx = width / 2;
      var cy = height / 2;
      var outerRadius = Math.min(width, height) / 2 - Style.space(10);
      var innerRadius = outerRadius * 0.65; // Doughnut hole

      if (!root.slices || root.slices.length === 0) {
        // Draw empty state circle
        ctx.beginPath();
        ctx.arc(cx, cy, outerRadius, 0, 2 * Math.PI);
        ctx.arc(cx, cy, innerRadius, 2 * Math.PI, 0, true);
        ctx.closePath();
        ctx.fillStyle = Qt.rgba(root.foreground.r, root.foreground.g, root.foreground.b, 0.08);
        ctx.fill();
        return;
      }

      var currentAngle = -Math.PI / 2; // Start from top 12 o'clock

      for (var i = 0; i < root.slices.length; i++) {
        var slice = root.slices[i];
        var pct = slice.percentage / 100.0;
        if (pct <= 0) continue;

        var sweepAngle = pct * 2 * Math.PI;
        var endAngle = currentAngle + sweepAngle;
        var isHovered = (root.hoveredIndex === i);
        var isSelected = (root.selectedGroupId !== "" && slice.group_id === root.selectedGroupId);
        var rOut = (isHovered || isSelected) ? outerRadius + Style.space(6) : outerRadius;

        ctx.beginPath();
        ctx.arc(cx, cy, rOut, currentAngle, endAngle);
        ctx.arc(cx, cy, innerRadius, endAngle, currentAngle, true);
        ctx.closePath();

        var colorHex = Model.getSliceColor(slice.slice_color_index || i, Color.accent);
        ctx.fillStyle = colorHex;
        ctx.fill();

        // Subtle slice separator line
        ctx.lineWidth = isSelected ? Style.space(3) : Style.space(2);
        ctx.strokeStyle = isSelected ? Color.accent : Color.background;
        ctx.stroke();

        currentAngle = endAngle;
      }
    }
  }

  // Center text labels inside the doughnut hole
  Column {
    anchors.centerIn: parent
    spacing: Style.space(2)
    width: parent.width * 0.55

    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      text: root.displaySublabel
      color: root.activeSlice ? Color.accent : Qt.darker(root.foreground, 1.4)
      font.family: root.fontFamily
      font.pixelSize: Style.font.caption
      font.bold: root.activeSlice !== null
      elide: Text.ElideRight
      maximumLineCount: 1
    }

    Text {
      anchors.horizontalCenter: parent.horizontalCenter
      text: root.displayLabel
      color: root.foreground
      font.family: root.fontFamily
      font.pixelSize: Style.font.subtitle
      font.bold: true
      elide: Text.ElideRight
      maximumLineCount: 1
    }
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor

    onPositionChanged: function(mouse) {
      if (!root.slices || root.slices.length === 0) return;
      var cx = width / 2;
      var cy = height / 2;
      var dx = mouse.x - cx;
      var dy = mouse.y - cy;
      var dist = Math.sqrt(dx * dx + dy * dy);
      var outerRadius = Math.min(width, height) / 2;
      var innerRadius = outerRadius * 0.60;

      if (dist >= innerRadius && dist <= outerRadius) {
        var angle = Math.atan2(dy, dx) + Math.PI / 2;
        if (angle < 0) angle += 2 * Math.PI;

        var accum = 0;
        var found = -1;
        for (var i = 0; i < root.slices.length; i++) {
          var sweep = (root.slices[i].percentage / 100.0) * 2 * Math.PI;
          if (angle >= accum && angle <= accum + sweep) {
            found = i;
            break;
          }
          accum += sweep;
        }
        if (root.hoveredIndex !== found) {
          root.hoveredIndex = found;
          canvas.requestPaint();
          if (found >= 0) root.sliceHovered(found, root.slices[found]);
        }
      } else {
        if (root.hoveredIndex !== -1) {
          root.hoveredIndex = -1;
          canvas.requestPaint();
        }
      }
    }

    onExited: {
      if (root.hoveredIndex !== -1) {
        root.hoveredIndex = -1;
        canvas.requestPaint();
      }
    }

    onClicked: function(mouse) {
      var cx = width / 2;
      var cy = height / 2;
      var dx = mouse.x - cx;
      var dy = mouse.y - cy;
      var dist = Math.sqrt(dx * dx + dy * dy);
      var outerRadius = Math.min(width, height) / 2;
      var innerRadius = outerRadius * 0.60;

      if (dist < innerRadius) {
        // Center clicked - clear selection
        root.selectedGroupId = "";
        root.centerClicked();
        canvas.requestPaint();
        return;
      }

      if (root.hoveredIndex >= 0 && root.hoveredIndex < root.slices.length) {
        var clicked = root.slices[root.hoveredIndex];
        if (root.selectedGroupId === clicked.group_id) {
          root.selectedGroupId = "";
        } else {
          root.selectedGroupId = clicked.group_id;
        }
        root.sliceClicked(root.hoveredIndex, clicked);
        canvas.requestPaint();
      }
    }
  }

  onSlicesChanged: canvas.requestPaint()
  onSelectedGroupIdChanged: canvas.requestPaint()
  onForegroundChanged: canvas.requestPaint()
  onHoveredIndexChanged: canvas.requestPaint()
}
