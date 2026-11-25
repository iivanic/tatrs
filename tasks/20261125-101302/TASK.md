# Serve board recovers when the event stream drops

- STATUS: CLOSED
- PRIORITY: 90
- TAGS: bug,ui

The board's event stream could die and never come back: `EventSource` gives up for good on a non-2xx
response (for example a 401 after `tatrs serve` restarts with a new token), and the page silently sat
on "reconnecting…". The stream authenticated only through a cookie, which every server on
`127.0.0.1` shares regardless of port, so a foreign `tatrs_token` broke reconnects. A stream write
also had no timeout, so a client that stopped reading pinned its thread forever.

## Acceptance

- [x] `/events` sends `retry: 3000`, so the browser's own reconnect delay is ours.
- [x] a stream write that blocks for 30s releases the thread instead of leaking it.
- [x] the page passes its URL token in the `/events` query and recreates a closed stream with capped
      backoff.
- [x] a rejected token is terminal and says so, instead of "reconnecting…" forever.
- [x] an anonymous `/events` request is still refused, and the integration suite covers both.

## Progress

2026-11-25:

- Repro: killing `tatrs serve` and starting it again on the same port leaves the page on
  "reconnecting…" forever, because the stream reconnects with the old token and `EventSource` stops
  retrying after the 401. Confirmed in Firefox through WebDriver.
- `serve_events` now writes `retry: 3000`, sets a 30s write timeout, and recovers a poisoned
  subscriber mutex instead of hanging up every future stream.
- The dashboard builds `/events?t=<token>` from its own URL, and when `EventSource` reaches
  `CLOSED` it probes `/api/status` and recreates the stream with capped backoff. A 401 shows
  "token rejected · reopen printed URL" and keeps probing every 30s instead of spinning.
- Long-running `curl` clients (5+ min) never lost the stream, so the server was not closing it; the
  console's `netInterrupt` was a client-side abort and the CSP `file://` lines came from a document
  reload, not from the board.
