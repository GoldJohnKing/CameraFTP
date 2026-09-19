# FastDenoise v4 (RGB-domain denoise model)

`fastdenoise_v4_512_fp16.onnx` — the RGB-domain denoise model used by the
RawAlchemy C++ core's neural-variant RGB denoise path (`ra_set_nn_model`
kind=2), applied after decode and before lens correction (upstream pipeline
op order).

## Provenance & license

Vendored from the upstream reference project
[Raw-Alchemy](https://github.com/shenmintao/Raw-Alchemy) (studio branch,
`src/raw_alchemy/vendor/fastdenoise_v4_512_fp16.onnx`), which is licensed
under AGPL-3.0-or-later and self-describes the architecture as a
DirectML-friendly dense-conv design (1/4-resolution trunk, 6.1M params,
512px tiles / 64px overlap, σ-conditioned).

Unlike the `../xveon/` models (no upstream license — do not redistribute),
this model is AGPL-3.0 and CameraFTP is AGPL-3.0, so embedding and
redistribution are license-compatible. Keep the exact bytes bit-identical to
upstream so the C++ cross-validation parity numbers stay meaningful.

## Runtime notes

- Embedded at build time by `build.rs::compress_nn_models` (gzip →
  `OUT_DIR/nn_models/fastdenoise.onnx.gz`, empty placeholder in legacy
  variant builds), injected in-memory at startup via `ra_set_nn_model(2, ...)`.
- Strength semantics: C API `denoiseStrength`, 0 = off, (0,1] clipped to
  [0.01, 0.5] internally (upstream σ range; σ>0.5 shifts neutral grey).
