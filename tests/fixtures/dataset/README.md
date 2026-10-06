# Dataset audit contract fixture

Authored for KazeNhanh on 2026-10-05. CC0-1.0.

Four synthetic OCR/ASR-like rows exercise train/calibration/test/development grouping and confirmation status. No recognizer was run and no audio/image exists. `verified` here is a simulated annotation state for contract tests, not verification of a real source. These rows are not language-quality, recognition-quality, training, calibration or acceptance data.

The manifest pins the exact UTF-8/LF JSONL bytes. Keep the hash synchronized when editing this fixture. Source files are included in the example's unit tests; all data processing remains offline.