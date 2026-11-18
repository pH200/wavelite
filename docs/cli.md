# CLI

Inspect the connected device:

```sh
wavelite info
```

Common hardware commands:

```sh
wavelite dial input-gain
wavelite dial output-volume
wavelite dial monitor-mix       # Wave:3 MK.1 only
wavelite clipguard off          # MK.2 selects hard clip
wavelite clipguard on
wavelite clipguard-mode bypassed # MK.2 only
wavelite lowcut off
wavelite lowcut on
wavelite gain 20
wavelite output-volume -23.5
wavelite mute off
wavelite monitor-mix 0
```

MK.2 hardware DSP commands:

```sh
wavelite expander on
wavelite compressor on
wavelite voice-tune on
wavelite voice-tune-strength 50
wavelite eq enabled on
wavelite eq band 1 --enabled on --kind bell --gain-db 3 --frequency-hz 1000 --q 1
wavelite dynamics compressor --threshold-db -20 --ratio 4 --attack-ms 10 --release-ms 100 --makeup-gain-db 6
wavelite dynamics expander --threshold-db -44 --ratio 2 --attack-ms 10 --release-ms 200 --max-reduction-db 24
```

EQ accepts high-pass, low-shelf, bell, notch, high-shelf, and low-pass filters, four bands indexed 1–4, ±12 dB gain, 20–20000 Hz, and Q 0.1–10. EQ and dynamics writes require MK.2 firmware 0.2.3 or newer; core controls remain available on older firmware.

Save and restore complete configuration blocks around experiments. A snapshot can only be restored to the same model:

```sh
wavelite snapshot before-test.json
wavelite restore before-test.json
```
