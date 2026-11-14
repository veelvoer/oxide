# Testing

* Unit tests: `cargo test --release`.
* Nested, scripted, frame-recorded UI tests: [tools/testbed](tools/testbed/README.md) and
  [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).
* Real hardware: run `oxide-session` from a TTY. If the screen stays black, `~/.cache/oxide/oxide.log` says why;
  `RUST_LOG=debug` and `OXIDE_DRM_DEVICE=/dev/dri/cardN` help. Please include `oxide-ctl doctor`.
