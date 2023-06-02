# Manual Test for `workers-rs` Socket Support

1. Update `git` repo / branch in `Cargo.toml` _and_ `wrangler.toml`.
2. Publish Worker to Cloudflare
3. Response status should be 200 and display something like:

```
[SUCCESS] NO_SSL
[SUCCESS] SSL
[SUCCESS] StartTls
[SUCCESS] ALLOW_HALF_OPEN
[SUCCESS] DISALLOW_HALF_OPEN
```
