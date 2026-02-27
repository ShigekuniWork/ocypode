---
title: Ocypode Protocol
description: Ocypode protocol specification
---

Ocypode communicates using a custom binary format designed to ensure stable
communication in any environment. This achieves both reduced bandwidth usage
and high-performance communication.

## Client Protocol

The communication protocol between the Ocypode server and clients follows a
simple Pub/Sub style. The Ocypode protocol is built on top of `QUIC`, which
provides encrypted communication via `TLS 1.3` by default and more stable
communication than `TCP`-based protocols.

### Protocol Conventions

TODO: Document protocol conventions (Topics, etc.)

### Protocol Messages

| Command | No | Sender | Description |
| --- | --- | --- | --- |
| `INFO` | `0x1` | Server | Sent to the client after a `QUIC` connection is established. |
| `CONNECT` | `0x2` | Client | Sent to the server to establish a connection. |
| `PUB` | `0x3` | Client | Used to publish a message to a specified topic. |
| `SUB` | `0x4` | Client | Used to subscribe to a topic or wildcard topic. |
| `UNSUB` | `0x5` | Client | Used to end a subscription. |
| `MSG` | `0x6` | Server | Delivers a message to clients subscribed to a topic. |
| `PING` | `0x7` | Both | Used as a keep-alive message. |
| `PONG` | `0x8` | Both | Response to a keep-alive message. |
| `OK` | `0x9` | Server | Notifies that a message was received in `verbose` mode. |
| `ERR` | `0xA` | Server | Sent when a protocol error occurs. May disconnect the client depending on the error. |

### Fixed Header

A header attached to every protocol message. The first byte encodes the
command type in the upper 4 bits and command-specific flags in the lower 4 bits.

| Field | Description | Size |
| --- | --- | --- |
| `command` | Command type | 4 bits (upper 4 bits of byte 1) |
| `flags` | Flags defined per command. See each command's section. Unused bits must be set to `0`. | 4 bits (lower 4 bits of byte 1) |
| `remaining_length` | Total size of the variable header and payload. Variable-length integer where the MSB of each byte is a continuation flag and the lower 7 bits encode the length. | 1–4 bytes |

### INFO (WIP)

TODO: Define specification

The client initiates a connection over `QUIC`. After the connection between
server and client is established, the server sends the configuration and
security requirements needed for exchanging messages.

#### INFO Flags

All reserved.

| Field | Description | Size |
| --- | --- | --- |
| `version` | Ocypode server protocol version | 1 byte |
| `max_payload` | Maximum payload size | 4 bytes |
| `server_id_length` | Length of the server ID | 1 byte |
| `server_id` | Server ID | `server_id_length` |
| `server_name_length` | Length of the server name | 1 byte |
| `server_name` | Server name | `server_name_length` |
| `auth_required` | Flag indicating whether authentication is required | 1 bit |
| `headers` | Whether the server supports headers | 1 bit |

`max_payload` defaults to a maximum of `1 MiB`.

### CONNECT

After establishing communication with the server and receiving an `INFO`
message, the client sends a `CONNECT` message to convey detailed information
and security credentials about the current connection.

#### CONNECT Flags

- bit 0: `verbose` — Enables `OK` responses from the server.
- bit 1: `has_auth` — Indicates presence of authentication fields.
- bits 2–3: reserved

#### CONNECT Variable Header

| Field | Description | Size |
| --- | --- | --- |
| `version` | Ocypode client protocol version | 1 byte |
| `auth_type` | Authentication method (only when `has_auth=1`). `1`: password auth, `2`: JWT auth | 1 byte |
| `auth_payload_length` | Size of the authentication payload (only when `has_auth=1`) | variable |
| `auth_payload` | Authentication payload (only when `has_auth=1`). Password auth (`1`): `username_length` (1 byte) + `username` (max 255 bytes) + `password_length` (1 byte) + `password` (max 255 bytes). JWT auth (`2`): `token_length` (2 bytes) + `token` | `auth_payload_length` |

When `has_auth=0`, fields from `auth_type` onward are absent.

### PUB

Used to publish a message to a specified topic.

#### PUB Flags

- bit 0: `has_reply_to` — Indicates presence of a reply-to topic.
- bit 1: `has_header` — Indicates presence of a header.
- bits 2–3: reserved

#### PUB Variable Header

| Field | Description | Size |
| --- | --- | --- |
| `topic_length` | Length of the topic name | 2 bytes |
| `topic` | Topic name (max 256 bytes) | `topic_length` |
| `reply_to_length` | Length of the reply-to topic name (only when `has_reply_to=1`) | 2 bytes |
| `reply_to` | Reply-to topic name (only when `has_reply_to=1`, max 256 bytes) | `reply_to_length` |
| `header_size` | Header size (only when `has_header=1`) | 2 bytes |
| `header` | Header (KV string format, only when `has_header=1`) | `header_size` |
| `payload_size` | Payload size. Variable-length integer where the MSB of each byte is a continuation flag and the lower 7 bits encode the length. | 1–4 bytes |
| `payload` | Payload | `payload_size` |

### SUB

Used to subscribe to a topic or wildcard topic.

#### SUB Flags

- bit 0: `has_queue_group` — Indicates whether joining a queue group.
- bits 1–3: reserved

#### SUB Variable Header

| Field | Description | Size |
| --- | --- | --- |
| `topic_length` | Length of the topic name | 2 bytes |
| `topic` | Topic name (wildcard supported, max 256 bytes) | `topic_length` |
| `subscription_id_length` | Size of the subscription ID | 2 bytes |
| `subscription_id` | Subscription ID assigned by the client | `subscription_id_length` |
| `queue_group_length` | Length of the queue group name (only when `has_queue_group=1`) | 1 byte |
| `queue_group` | Queue group name (only when `has_queue_group=1`) | `queue_group_length` |

### UNSUB

Used to end a subscription.

#### UNSUB Flags

All reserved.

#### UNSUB Variable Header

| Field | Description | Size |
| --- | --- | --- |
| `subscription_id_length` | Size of the subscription ID | 2 bytes |
| `subscription_id` | Subscription ID assigned by the client | `subscription_id_length` |

### MSG

Delivers a message to clients subscribed to a topic.

#### MSG Flags

- bit 0: `has_reply_to` — Indicates presence of a reply-to topic.
- bit 1: `has_header` — Indicates presence of a header.
- bits 2–3: reserved

#### MSG Variable Header

| Field | Description | Size |
| --- | --- | --- |
| `topic_length` | Length of the topic name | 2 bytes |
| `topic` | Topic name (max 256 bytes) | `topic_length` |
| `subscription_id_length` | Size of the subscription ID | 2 bytes |
| `subscription_id` | Subscription ID assigned by the client | `subscription_id_length` |
| `reply_to_length` | Length of the reply-to topic name (only when `has_reply_to=1`) | 2 bytes |
| `reply_to` | Reply-to topic name (only when `has_reply_to=1`, max 256 bytes) | `reply_to_length` |
| `header_size` | Header size (only when `has_header=1`) | 2 bytes |
| `header` | Header (KV string format, only when `has_header=1`) | `header_size` |
| `payload_size` | Payload size. Variable-length integer where the MSB of each byte is a continuation flag and the lower 7 bits encode the length. | 1–4 bytes |
| `payload` | Payload | `payload_size` |

### PING

Can be sent by either the client or server. Sent at regular intervals to
verify the connection is alive; the receiver must respond with `PONG`.

#### PING Flags

All reserved.

No variable header or payload.

### PONG

Response to a `PING` message. The receiver of a `PING` must promptly return a
`PONG`. Failure to respond within a certain time may result in the connection
being dropped.

#### PONG Flags

All reserved.

No variable header or payload.

### OK

Sent by the server to notify the client that a message was successfully
received, when `verbose=1` is set in the `CONNECT` flags.

#### OK Flags

All reserved.

No variable header or payload.

### ERR

Sent by the server when a protocol violation or invalid message is detected.
Depending on the error, the client connection may be terminated after sending.

#### ERR Flags

All reserved.

TODO: Define error codes and variable header
