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
* 📬 Custom serialization protocols

## MQTTs

Do connect to a TLS enabled MQTT broker provide the protocol & basic auth
via the URL:

```console
mqttui mqtts://my-user:my-password@my-host:8883
```

## Serialization Protocols

By default, each message is tried to parse as JSON. It will be pretty-printed
automatically if valid. Alternatively CBOR messages which can be converted back
into JSON are also supported.

If your data is serialized in a different format (like protobuf), you can
register your own "protocol":

```toml
# ~/.config/mqttui/config.toml

[[protocols]]
label = "PROTO"          # Short label (<=6 chars), will be displayed in top right
program = "python3"      # Executable to call to decode the message
args = ["my-decoder.py"] # Arguments to `program`
topic = "^foo/bar/.*"    # Regex for the topic(s) where this protocol is acting on
```

MQTTUI will call this program with the given arguments for each message whose
topic matches the regex and then will pass the message bytes via `stdin`. The program
can output arbitrary JSON to `stdout`, which MQTTUI will then render (pretty print
& highlight). If the parsing fails, the program can write any errors to `stderr`
and indicate this via an non-zero exit code.

> [!IMPORTANT]
> Since this program will be called for each message, make sure that it runs
> reasonably fast, otherwise you will block the whole TUI
