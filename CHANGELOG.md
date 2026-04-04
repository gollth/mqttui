# CHANGELOG

## UNRELEASED

* Don't flip flop for multiple instances
* Fix bug where "retain" message sometimes not show correctly

## 1.5.0

* Render CBOR messages
* Show data format next to topic name

## 1.4.0

* Support MQTTs
* Group JQ history items by topic suffix
* Fix left & right keys in message history
* Fix proper amount of crumbs in message history
* Update to Rust 1.92

## 1.3.0

* Add JQ history with `<UP>`/`<DOWN>`
* Remove message counter in topics overview
* Reuse state from MQTT broker on restart (no clean session)
* Keep JQ filter on each apply

## 1.2.0

* Make broker URL positional argument
* Fix bug where color for retained messages is lost
* Fix bug where `<host>:<port>` was not allowed
* Add logging to `~/.cache/mqttui/mqttui.log`
* Add message history
* Allow scrolling with '{' & '}' like in VIM

## 1.1.0

* Switch to [rumqttc](https://docs.rs/rumqttc/latest/rumqttc/)
* Show connection status in header
* Automatically reconnect if broker restarts
* `-q/--quit` do immediately disconnect on connection loss
* Allow non-JSON messages but show warning

## 1.0.0

* Show topics in selectable list
* Fuzzy search through topics
* Allow for negative search in topics
* Copy selected topic on 'y' to clipboard
* Inspect messages of selected topic with 'ENTER'
* Copy current message on 'y' to clipboard
* JSON syntax highlighting & pretty printing
* Filter through message with 'jq' syntax
* Configuration via XDG_CONFIG
* Provide Broker URL via CLI
