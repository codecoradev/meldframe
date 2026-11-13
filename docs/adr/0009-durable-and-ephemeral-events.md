---
status: accepted
---
# Live events are either durable or ephemeral

Frames sent to live clients are split explicitly. Durable frames are the accepted ops: each has a sequence number and can be replayed from a cursor after a reconnect. Ephemeral frames (agent touch hints, capture requests, progress) carry no sequence number, are best-effort and are never replayed. Every stream has a bounded, non-blocking queue; when it is full the frame is dropped and a resync marker is sent, so a slow client never slows the room. The number of concurrent streams is capped and heartbeats are tested against a real clock.

## Considered options

- One undifferentiated message stream: rejected; replay and loss rules would be implicit and different per message type.

## Consequences

The protocol types gain a durable/ephemeral marker and the editor must resync on the marker. This matches the shape of the spike's WebSocket ops and the capture relay.
