---
title: Topic
description: Ocypode topic specification
---

Ocypode uses topics for message exchange between the server and clients.
While it supports multi-layered topics and wildcard subscriptions similar to MQTT,
it imposes stricter constraints to maximize performance.

## Topic layers

The `/` character is used to separate topic layers.

```bash
sensor/data
sensor/data/temperature
sensor/data/wind-speed
```

### Layer constraints

- A topic must not have a leading or trailing slash.
- The maximum number of topic layers is 8.
- The maximum topic length is 256 bytes.

## Wildcards

Ocypode provides two wildcards that can take the place of one or more layers in a slash-separated topic.
Publishers always send messages to fully specified topics (no wildcards).
Subscribers may use wildcards to subscribe to multiple topics with a single subscription.

### Single-layer wildcard

The `+` is the single-layer wildcard.

```bash
sensor/+/data

# Matches
sensor/1/data
sensor/2/data
```

### Multiple-layer wildcard

The `#` is the multiple-layer wildcard.

```bash
sensor/data/#

# Matches
sensor/data/temperature
sensor/data/alert/warning
```

### Wildcard constraints

- Wildcards may be used only when subscribing.
- The multiple-layer wildcard must be in terminal position.
- Wildcard topic matching impacts system throughput.
- Topics whose first layer is `$SYS` are reserved for system use.
