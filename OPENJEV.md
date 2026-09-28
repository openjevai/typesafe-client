# OpenJEV Support

This fork adds optional [OpenJEV](https://openjev.sh) support alongside the
original TypeSafe configuration. OpenJEV is a free community gateway to the
same Jev model built by [TypeSafe](https://typesafe.ai). TypeSafe remains the
default; anyone with a TypeSafe key sees zero behaviour change.

## What was added

| File | Change |
|---|---|
| `go/typesafe.go` | Added `OpenjevDefaultBaseURL`, `OpenjevDefaultModel`, `EnvOpenjevAPIKey`, `EnvJevProvider` constants |
| `go/client.go` | Added `resolveProvider` and provider-aware key/base-URL/model resolution in `NewClient`; `resolveAPIKey` now takes the key env var |
| `rust/src/lib.rs` | Added `OPENJEV_DEFAULT_BASE_URL` and `OPENJEV_DEFAULT_MODEL` constants |
| `rust/src/config.rs` | Added `OPENJEV_API_KEY_ENV`, `JEV_PROVIDER_ENV` constants; `resolve_api_key_with_env`, `resolve_base_url_with_default`, `resolve_default_model_with_default`, and `resolve_provider` functions |
| `rust/src/client.rs` | `ClientBuilder::build` uses `resolve_provider` for provider-aware key/base-URL/model resolution |
| `README.md` | OpenJEV note after the project intro |
| `go/README.md` | Provider selection note in the Configuration section |
| `rust/README.md` | Provider selection note in Configuration + Environment variables sections |

## Provider selection rule

1. **Explicit choice wins:** `JEV_PROVIDER=openjev` forces OpenJEV.
2. **TypeSafe if its key is set:** when `TYPESAFE_API_KEY` is set (or an API key
   is passed explicitly), TypeSafe is used with the original defaults
   (`https://api.typesafe.ai`, model `jev-latest`). This is the unchanged
   default.
3. **OpenJEV fallback:** when no TypeSafe key is set but `OPENJEV_API_KEY` is
   set, OpenJEV is used (`https://api.openjev.sh`, model `openjev`, key from
   `OPENJEV_API_KEY`).

Explicit builder/option values (Go: `WithAPIKey`, `WithBaseURL`, `WithModel`;
Rust: `.api_key()`, `.base_url()`, `.default_model()`) always take precedence
over the provider defaults.

## Configuration

### Go

```go
// Option 1: OpenJEV via env (auto-selected when TYPESAFE_API_KEY is unset)
os.Setenv("OPENJEV_API_KEY", "oj-...")
client, err := typesafe.NewClient()

// Option 2: Force OpenJEV even with a TypeSafe key present
os.Setenv("JEV_PROVIDER", "openjev")
os.Setenv("OPENJEV_API_KEY", "oj-...")
client, err := typesafe.NewClient()

// Option 3: Explicit (works with any provider)
client, err := typesafe.NewClient(
    typesafe.WithAPIKey("oj-..."),
    typesafe.WithBaseURL("https://api.openjev.sh"),
    typesafe.WithModel("openjev"),
)
```

### Rust

```rust
// Option 1: OpenJEV via env (auto-selected when TYPESAFE_API_KEY is unset)
// set OPENJEV_API_KEY=oj-...
let client = Client::from_env()?;

// Option 2: Force OpenJEV
// set JEV_PROVIDER=openjev and OPENJEV_API_KEY=oj-...
let client = Client::from_env()?;

// Option 3: Explicit
let client = Client::builder()
    .api_key("oj-...")
    .base_url("https://api.openjev.sh")
    .default_model("openjev")
    .build()?;
```

## Retry behaviour

HTTP 503 (OpenJEV's overload status) is already retried by both clients'
default retry policies, which retry all 5xx responses (500–599). TypeSafe's
529 Overloaded is likewise covered. No change was needed.

## Verification

A live `POST https://api.openjev.sh/v1/systemone` request with model `openjev`,
state `"ping"`, and one noul question returned HTTP 200 with a valid answer.
No repository code was executed during this port.

## Upstream

Original project: https://github.com/haileyok/typesafe-client by @haileyok
