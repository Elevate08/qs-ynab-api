import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

Item {
  id: root

  property var shell: null
  property var manifest: null
  property bool opened: false

  property string tokenDraft: ""
  property string statusMsg: ""
  property bool isError: false
  property bool loading: false

  readonly property color foreground: Color.foreground
  readonly property string fontFamily: Style.font.family
  readonly property string cliPath: {
    var resolved = Qt.resolvedUrl("bin/ynab-cli").toString().replace(/^file:\/\//, "")
    return resolved !== "" ? resolved : (Quickshell.env("HOME") + "/projects/qs-ynab-api/bin/ynab-cli")
  }

  function open() {
    opened = true
    tokenDraft = ""
    statusMsg = ""
    isError = false
    authStatusProc.running = true
  }

  function close() {
    opened = false
    if (shell && typeof shell.hide === "function") {
      shell.hide((manifest && manifest.id) || "io.github.elevate08.ynab-glance")
    }
  }

  function saveToken() {
    var trimmed = tokenDraft.trim()
    if (trimmed === "") {
      statusMsg = "Please enter a valid Personal Access Token"
      isError = true
      return
    }
    loading = true
    statusMsg = "Connecting to YNAB..."
    isError = false
    saveTokenProc.command = [cliPath, "auth", "set", trimmed]
    saveTokenProc.running = true
  }

  function clearToken() {
    loading = true
    statusMsg = "Revoking token..."
    clearTokenProc.running = true
  }

  Process {
    id: authStatusProc
    command: [cliPath, "auth", "status"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var parsed = JSON.parse(text)
          if (parsed.authenticated) {
            root.statusMsg = "Connected to YNAB (User ID: " + (parsed.user_id || "Active") + ")"
            root.isError = false
          } else {
            root.statusMsg = "Not connected. Personal Access Token required."
            root.isError = false
          }
        } catch (e) {
          root.statusMsg = "Status check failed"
        }
      }
    }
  }

  Process {
    id: saveTokenProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.loading = false
        try {
          var parsed = JSON.parse(text)
          if (parsed.ok) {
            root.statusMsg = "Successfully authenticated and saved to Secret Service keyring!"
            root.isError = false
            root.tokenDraft = ""
          } else {
            root.statusMsg = parsed.error || "Failed to authenticate"
            root.isError = true
          }
        } catch (e) {
          root.statusMsg = "Error saving token"
          root.isError = true
        }
      }
    }
    onExited: function(code) {
      root.loading = false
    }
  }

  Process {
    id: clearTokenProc
    command: [cliPath, "auth", "clear"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        root.loading = false
        root.statusMsg = "Token removed from Secret Service keyring."
        root.isError = false
      }
    }
    onExited: function(code) {
      root.loading = false
    }
  }

  BorderSurface {
    anchors.centerIn: parent
    width: Style.space(400)
    implicitHeight: settingsCol.implicitHeight + Style.space(32)
    color: Color.background
    radius: Style.cornerRadius
    borderSpec: Border.flat(Color.accent, 1)

    ColumnLayout {
      id: settingsCol
      anchors.fill: parent
      anchors.margins: Style.space(16)
      spacing: Style.space(12)

      RowLayout {
        Layout.fillWidth: true
        Text {
          text: "YNAB Pulse Settings"
          color: root.foreground
          font.family: root.fontFamily
          font.pixelSize: Style.font.subtitle
          font.bold: true
          Layout.fillWidth: true
        }
        Button {
          text: "✕"
          onClicked: root.close()
        }
      }

      Text {
        text: root.statusMsg
        color: root.isError ? Color.urgent : Color.accent
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        visible: root.statusMsg !== ""
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
      }

      Button {
        text: "󰖟 Open YNAB Developer Settings"
        onClicked: Quickshell.execDetached(["xdg-open", "https://app.ynab.com/settings/developer"])
      }

      Text {
        text: "Update Personal Access Token:"
        color: root.foreground
        font.family: root.fontFamily
        font.pixelSize: Style.font.caption
        font.bold: true
      }

      TextField {
        id: patInput
        Layout.fillWidth: true
        placeholderText: "Paste new API Token..."
        text: root.tokenDraft
        echoMode: TextInput.Password
        onTextChanged: root.tokenDraft = text
      }

      RowLayout {
        Layout.fillWidth: true
        spacing: Style.space(8)

        Button {
          text: root.loading ? "Saving..." : "Save Token"
          enabled: !root.loading && root.tokenDraft.trim() !== ""
          onClicked: root.saveToken()
        }

        Button {
          text: "Revoke Token"
          enabled: !root.loading
          onClicked: root.clearToken()
        }
      }

      Item { Layout.fillHeight: true }
    }
  }
}
