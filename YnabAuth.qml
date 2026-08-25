import QtQuick
import Quickshell.Io
import "Model.js" as Model

Item {
  id: auth

  // Path to the ynab-cli helper, resolved relative to Model.js (the plugin directory).
  // No environment override and no fallback path: see SECURITY.md.
  readonly property string cliPath: Model.cliPath()
  readonly property bool busy: statusProc.running || setProc.running || clearProc.running

  signal statusChecked(bool authenticated, string userId)
  signal tokenSaved()
  signal tokenCleared(bool cachePurged)
  signal failed(string message)

  function checkStatus() {
    if (statusProc.running) return
    statusProc.running = true
  }

  function saveToken(token) {
    var trimmed = (typeof token === "string") ? token.trim() : ""
    if (trimmed === "") {
      auth.failed("Please enter a valid Personal Access Token")
      return
    }
    // Handed over stdin, never as an argument: argv is world-readable
    // through /proc/<pid>/cmdline for as long as the helper runs.
    setProc.pendingToken = trimmed
    setProc.stdinEnabled = true   // re-open stdin; onStarted closes it again
    setProc.running = true
  }

  function clearToken() {
    if (clearProc.running) return
    clearProc.running = true
  }

  // -------------------------------------------------------------------------
  // Subprocesses
  // -------------------------------------------------------------------------

  Process {
    id: statusProc
    property bool handled: false

    command: [auth.cliPath, "auth", "status"]
    onStarted: {
      handled = false
    }
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var parsed = JSON.parse(text)
          statusProc.handled = true
          if (parsed.authenticated) {
            auth.statusChecked(true, parsed.user_id || "")
          } else {
            if (parsed.error) {
              auth.failed(parsed.error)
            }
            auth.statusChecked(false, "")
          }
        } catch (e) {
          statusProc.handled = true
          auth.failed("Status check failed")
        }
      }
    }
    onExited: function(code) {
      if (code !== 0 && !statusProc.handled) {
        statusProc.handled = true
        auth.failed("Status check exited with error code " + code)
      }
    }
  }

  Process {
    id: setProc
    property string pendingToken: ""
    property bool handled: false

    command: [auth.cliPath, "auth", "set"]
    onStarted: {
      handled = false
      write(pendingToken + "\n")
      pendingToken = ""
      stdinEnabled = false  // closes stdin so the helper stops reading
    }
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var parsed = JSON.parse(text)
          setProc.handled = true
          if (parsed.ok) {
            auth.tokenSaved()
          } else {
            auth.failed(parsed.error || "Failed to authenticate")
          }
        } catch (e) {
          setProc.handled = true
          auth.failed("Error saving token")
        }
      }
    }
    onExited: function(code) {
      if (code !== 0 && !setProc.handled) {
        setProc.handled = true
        auth.failed("Authentication failed with exit code " + code)
      }
    }
  }

  Process {
    id: clearProc
    property bool handled: false

    command: [auth.cliPath, "auth", "clear"]
    onStarted: {
      handled = false
    }
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try {
          var parsed = JSON.parse(text)
          clearProc.handled = true
          if (parsed.ok) {
            auth.tokenCleared(parsed.cache_purged === true)
          } else {
            auth.failed(parsed.error || "Failed to clear token")
          }
        } catch (e) {
          clearProc.handled = true
          auth.failed("Error clearing token")
        }
      }
    }
    onExited: function(code) {
      if (code !== 0 && !clearProc.handled) {
        clearProc.handled = true
        auth.failed("Clear token exited with error code " + code)
      }
    }
  }
}
