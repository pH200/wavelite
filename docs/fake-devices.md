# Fake Wave:3 devices and screenshots

The opt-in `demo` binary provides in-memory Wave:3 MK.1 and MK.2 devices. It uses fictional serials, never accesses USB hardware, and is excluded from normal builds and installations.

Run an interactive demo:

```sh
WAVELITE_FAKE_MODEL=wave3-mk1 cargo run --locked --features demo --bin wavelite-gui-demo
WAVELITE_FAKE_MODEL=wave3-mk2 cargo run --locked --features demo --bin wavelite-gui-demo
```

Set `WAVELITE_SCREENSHOT` to render the active window to a PNG and exit. Parent directories are created automatically, and an existing output file is replaced:

```sh
WAVELITE_FAKE_MODEL=wave3-mk1 \
WAVELITE_SCREENSHOT=target/screenshots/wave3-mk1.png \
cargo run --locked --features demo --bin wavelite-gui-demo

WAVELITE_FAKE_MODEL=wave3-mk2 \
WAVELITE_SCREENSHOT=target/screenshots/wave3-mk2.png \
cargo run --locked --features demo --bin wavelite-gui-demo
```

For MK.2, `WAVELITE_FAKE_WAVE_FX=dynamics` or `WAVELITE_FAKE_WAVE_FX=eq` opens that DSP page before capture:

```sh
WAVELITE_FAKE_MODEL=wave3-mk2 WAVELITE_FAKE_WAVE_FX=dynamics \
WAVELITE_SCREENSHOT=target/screenshots/wave3-mk2-dynamics.png \
cargo run --locked --features demo --bin wavelite-gui-demo

WAVELITE_FAKE_MODEL=wave3-mk2 WAVELITE_FAKE_WAVE_FX=eq \
WAVELITE_SCREENSHOT=target/screenshots/wave3-mk2-eq.png \
cargo run --locked --features demo --bin wavelite-gui-demo
```

A graphical session is required. Screenshots are generated through GTK itself; no screen-capture utility or synthetic desktop input is needed.
