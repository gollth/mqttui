# Mqt-TUI

Explore & search through your MQTT traffic.

Inspired by [MQTT-Explorer](https://mqtt-explorer.com/), but with the opinionated
improvements:

* 📈 TUI
* 🎨 Pretty-print & syntax-highlight JSON messages
* 🔍 Search topics fuzzily
* ⏳ Show retained & stale topics
* 🪄 Filter JSON messages with [jq](https://jqlang.org/)
* 🕹️ VIM keybindings
* 🔟 CBOR support

## MQTTs

Do connect to a TLS enabled MQTT broker provide the protocol & basic auth
via the URL:

```console
mqttui mqtts://my-user:my-password@my-host:8883
```
