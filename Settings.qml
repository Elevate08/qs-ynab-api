import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
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
  readonly property bool loading: auth.busy

  readonly property color foreground: Color.foreground
  readonly property string fontFamily: Style.font.family

  YnabAuth {
    id: auth

    onStatusChecked: function(authenticated, userId) {
      if (authenticated) {
        root.statusMsg = "Connected to YNAB (User ID: " + (userId || "Active") + ")"
        root.isError = false
      } else {
        root.statusMsg = "Not connected. Personal Access Token required."
        root.isError = false
      }
    }

    onTokenSaved: {
      root.statusMsg = "Successfully authenticated and saved to Secret Service keyring!"
      root.isError = false
      root.tokenDraft = ""
    }

    onTokenCleared: function(cachePurged) {
      root.statusMsg = cachePurged
        ? "Token removed from Secret Service keyring and cache purged."
        : "Token removed from Secret Service keyring."
      root.isError = false
    }

    onFailed: function(message) {
      root.statusMsg = message || "Operation failed"
      root.isError = true
    }
  }

  function open() {
    opened = true
    tokenDraft = ""
    statusMsg = ""
    isError = false
    auth.checkStatus()
  }

  function close() {
    opened = false
    // Drop any half-typed Personal Access Token rather than leaving it in a
    // QML string property after the dialog is dismissed.
    tokenDraft = ""
    if (shell && typeof shell.hide === "function" && manifest) {
      shell.hide(manifest.id)
    }
  }

  function saveToken() {
    var trimmed = tokenDraft.trim()
    if (trimmed === "") {
      statusMsg = "Please enter a valid Personal Access Token"
      isError = true
      return
    }
    statusMsg = "Connecting to YNAB..."
    isError = false
    auth.saveToken(trimmed)
  }

  function clearToken() {
    statusMsg = "Revoking token..."
    isError = false
    auth.clearToken()
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
          textFormat: Text.PlainText
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
        textFormat: Text.PlainText
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
        textFormat: Text.PlainText
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
