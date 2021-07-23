# Unreleased

- **breaking** `ImtGlyphBitmap` `data` field is now private. Bitmap data is now represented by `ImtBitmapData` enum which can be an image, raw data in the form of a vec, or empty in the case where a bitmap is applicable.
- **breaking** `ImtGlyphBitmap` `width`, `height`, `bearing_x`, `bearing_y` have been moved into `ImtBitmapMetrics` which can be obtained from the `ImtGlypyBitmap::metrics()` method.
- **breaking** `ImtGlyph` `bitmap` field now is an option of `ImtBitmapData` instead of a vec of the raw data.
- **breaking** Bitmap data color componenents are now value normalized. This is the same as `vec4(color.rgb / color.a, color.a)`. This behavior already existed in `Basalt` therefore `Basalt` users will not see any change from this other than a minor performance improvement.
- **breaking** `ImtRaster` now has two creation methods, `new_gpu` and `new_cpu`. This will select the rasterization backend used. `ImtFont` methods `from_file` & `from_bytes` have been split into `from_file_cpu`, `from_file_gpu`, `from_bytes_cpu`, & `from_bytes_gpu` to match this change.
- Added `ImtImageView` which is very similar to `BstImageView` from `Basalt`. This is an abstraction over `vulkano`'s `ImageView` that makes handling `ImageViews` more abstract.
- Added `raster_to_image` option to `ImtRasterOpts` which defaults to true. This option will enable/disable outputing to an image instead of raw data.
- Update dependencies `allsorts` to `0.6.0` & `ordered-float` to `2.7.0`.

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
