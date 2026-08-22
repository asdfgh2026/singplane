# singpanel-core

Shared **control plane** crate. Not a sing-box kernel.

| Consumer | How |
|---|---|
| `core/host` (`singpanel-host`) | Desktop HTTP process. Depends on this crate. |
| Android (`mobile/`) | Kotlin `ControlPlane` today. Later UniFFI over this crate. |

Public modules: `assemble`, `check`, `convert`, `engine`, `fetch`, `clash`, `helper`.

```powershell
cd core\lib
cargo test
```
