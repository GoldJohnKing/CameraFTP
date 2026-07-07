# x-veon Neural Demosaic Models

Pre-trained ONNX models for neural-network demosaicing of Bayer (2×2 CFA) and
X-Trans (6×6 CFA) raw sensor data. Used by the RawAlchemy NN demosaic pipeline
(`NnDemosaicSession`) as the inference graph for the DirectML / QNN HTP / CPU
execution providers.

## Provenance

| Field | Value |
|---|---|
| **Upstream project** | [x-veon](https://github.com/naorunaoru/x-veon) — "neural network demosaicing for Bayer and X-Trans sensors" |
| **Author** | naorunaoru |
| **Source commit** | latest `HEAD` of `main` at fetch time (`git clone --depth 1`) |
| **Source paths** | `web/public/bayer.onnx`, `web/public/xtrans.onnx` |
| **Fetched** | Task 9 of the NN-demosaic implementation plan |
| **License** | **TBD** — the upstream repository ships **no LICENSE file** and states no license in its README. These weights are vendored for evaluation/integration pending a license determination with the author. Do **not** redistribute until the license is confirmed. The CameraFTP application itself is AGPL-3.0-or-later; this note does not override the upstream model's (yet-unknown) terms. |

## Model files

| File | Size | Weights | Parameters | I/O shape |
|---|---|---|---|---|
| `bayer.onnx` | 3.8 MB | 49 tensors | **1,941,289** (~1.94 M) | `[1, 4, 288, 288]` → `[1, 3, 288, 288]` |
| `xtrans.onnx` | 15 MB | 49 tensors | **7,760,457** (~7.76 M) | `[1, 4, 288, 288]` → `[1, 3, 288, 288]` |

**ONNX metadata:** IR version `10`, opset `ai.onnx:17`. Weights are stored as
**FP16** (`onnx.TensorProto.FLOAT16`, dtype 10) with INT64 shape tensors
(dtype 7) — hence the small on-disk footprint relative to parameter count
(~2 bytes/param).

**Input semantics:** 4-channel tensor = raw CFA mosaic value (channel 0) plus
3 binary color-mask channels indicating which color filter covers each pixel.
**Output:** 3-channel RGB image. Tile size is fixed at **288×288** (the
RawAlchemy dispatch loop tiles larger sensors and reassembles with overlap).

## Architecture (per upstream README)

A U-Net (encoder-decoder with skip connections) with 4 downsampling stages. The
encoder narrows to a 1024-channel bottleneck for the full-width (X-Trans) model.
Each stage is two 3×3 convolutions with BatchNorm and ReLU followed by 2×2 max
pooling; the decoder mirrors this with transposed-conv upsampling and skip
connections. A residual CFA skip broadcasts the raw mosaic value to all 3 output
channels as a baseline, so the network only learns color-correction deltas —
making it largely exposure-agnostic. The architecture is CFA-pattern-agnostic
(the same topology serves both Bayer and X-Trans; only the trained weights and
the input masks differ).

## Verification (how these numbers were obtained)

```bash
git clone --depth 1 https://github.com/naorunaoru/x-veon.git
python3 -c "import onnx, numpy as np; m=onnx.load('web/public/bayer.onnx'); \
  print('params=', sum(int(np.prod(i.dims)) for i in m.graph.initializer))"
```

SHA-256 at vendoring time:
- `bayer.onnx`:  `756e4e79e2e5476e56e4a6cddf9b1e162728de99c4761197fe60e62b21f1a3c6`
- `xtrans.onnx`: `45b1fa22b0027868fd5c20ec7b59234ed5aeb35de89fbc0950a4bec67f328500`
