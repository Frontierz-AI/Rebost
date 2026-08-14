# Tesseract traineddata

`eng.traineddata`, `spa.traineddata`, and `cat.traineddata` come from
[tesseract-ocr/tessdata](https://github.com/tesseract-ocr/tessdata)
and are licensed under the Apache License 2.0.

https://www.apache.org/licenses/LICENSE-2.0

Rebost copies these files into app data at first run. English data is also
pulled in through the `xberg` crate (`bundle-tessdata-eng`).
