# Board stream drops are client-side network changes

- STATUS: CLOSED
- PRIORITY: 80
- TAGS: docs,ui

Serving the QMApp board showed "reconnecting…" and `netInterrupt` for `/events` after some time. The
question was whether `tatrs serve` closes streams in a busy tree (60 tasks, 19KB snapshots, publishes
every few minutes) or whether the client does.

## Acceptance

- [x] reproduced in QMApp with a raw socket client and with Firefox
- [x] server-side cause excluded: the raw client survives publishes, a 60-publish burst, and a suspend
- [x] trigger identified from the kernel/NetworkManager log
- [x] documented in `docs/DASHBOARD.md` §5

## Progress

2026-11-25:

- Instrumented `serve_events` (temporary, reverted) to log open/write-failure with peer and
  User-Agent. Over ~45 minutes the server never closed a stream: every failure was
  `Broken pipe` on a ping, i.e. the client's FIN, and the client was always Firefox or a test
  teardown. A Python client held one connection for 33 minutes, including a 25-minute suspend.
- Firefox aborts the stream: its nsHttp log shows `nsHttpConnection::Close drained 0 bytes` /
  `closed active connection due to no traffic` and the channel ends with `status=80004004`
  (`NS_ERROR_ABORT`), after which the page reconnects.
- Trigger: the machine suspends (`PM: suspend entry` 11:17:38-11:30:10, 12:30:59-12:56:03) and
  QMApp's compose stack churns Docker networks (`NetworkManager: new Veth device`, 244 today;
  `qmapp-ui-1`/`qmapp-api-1` up 20s at the time). Firefox cancels in-flight connections on a network
  state change, so the disconnect is the environment, not the board.
- Nothing to fix server-side; 0.2.1 already recovers, and the docs now say where to look instead of
  chasing a server timeout.
