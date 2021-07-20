# Unreleased

# Version 0.4.1 (July 20th, 2021)

- `ImtRasterOpts` now has `cpu_rasterization` option to specify if cpu should be used instead of gpu for rasterization. Nvidia seems to be broken currently for gpu rasterization, so as a result this currently defaults to `true`.
- `ImtGlyphBitmap` now has the method `raster_cpu` to rasterize on the cpu instead of gpu.

# Version 0.4.0 (July 4th, 2021)

- **breaking** Update dependency `vulkano` & `vulkano-shaders` to `0.24.0`.

# Version 0.3.0 (May 29th, 2021)

- **breaking** Update dependency `vulkano` & `vulkano-shaders` to `0.23.0`
- Update dependency `ordered-float` from `2.0` to `2.5`.

# Version 0.2.0 (January 31st, 2021)

- **breaking** Update dependency `vulkano` & `vulkano-shaders` to `0.20.0`.
- Update dependency `allsorts` to `0.5.1`.
- Update dependency `ordered-float` to `2.0.1`.
- Update dependency `parking_lot` to `0.11.1`.
- Update dependency `crossbeam` to `0.8.0`

# Version 0.1.0 (June 16th, 2020)

- **breaking** Renamed `ImtRasterOps` to `ImtRasterOpts` to improve naming consistency.
- **breaking** Added `align_whole_pixels` to `ImtShapeOpts` & `ImtRasterOps` which defaults to `true`.
