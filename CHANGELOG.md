# CHANGELOG

## Unreleased

* Switch to [rumqttc](https://docs.rs/rumqttc/latest/rumqttc/)
* Show connection status in header
* Automatically reconnect if broker restarts
* `-q/--quit` do immediately disconnect on connection loss

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
