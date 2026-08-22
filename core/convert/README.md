# singpanel-convert

Loopback helper: **Clash YAML / node URI list → sing-box outbounds JSON**.

Uses [xmdhs/clash2singbox](https://github.com/xmdhs/clash2singbox) (MIT).

## Build

```bash
cd core/convert
go mod tidy
go build -o singpanel-convert.exe .   # Windows
# go build -o singpanel-convert .
```

Place the binary next to the SingPanel app, under `bin/`, or leave it in `core/convert/` for dev.

## Protocol

1. Process prints one line to stdout:  
   `READY port=<n> token=<secret>`
2. Listens only on `127.0.0.1`.
3. `POST /v1/convert`  
   Header: `Authorization: Bearer <token>`  
   Body:
   ```json
   {
     "subscriptionBody": "...",
     "include": "",
     "exclude": ""
   }
   ```
4. Response:
   ```json
   {
     "ok": true,
     "outbounds": [ { "type": "shadowsocks", "tag": "n1", ... } ],
     "endpoints": [],
     "warnings": [ { "node": "...", "reason": "..." } ],
     "stats": { "inputNodes": 1, "converted": 1, "skipped": 0 }
   }
   ```

Does **not** fetch subscription URLs (the GUI downloads; reduces SSRF surface).
