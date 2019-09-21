[![builds.sr.ht status](https://builds.sr.ht/~kirikaza/postgread.svg)](https://builds.sr.ht/~kirikaza/postgread?)

Postgread is a proxy for PostgreSQL frontend/backend protocol version 3.0 (implemented in PostgreSQL 7.4 and later, see [official documentation](https://www.postgresql.org/docs/current/protocol.html)).

The project is at early stage of development. Now postgread proxies unencrypted PostgreSQL messages in both directions, supports multiple simultaneous connections, and logs some of proxied messages. The first milestone is to log *all* types of messages and to support encrypted traffic.