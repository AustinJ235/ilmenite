# Unreleased

# Version 0.14.0 (Arpil 29th, 2023)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.33`.
- Update dependency `allsorts` to `0.14`.

# Version 0.13.0 (October 31st, 2022)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.32`.
- **BREAKING** `ImtParser::retreive_text` has had its spelling correct to `ImtParser::retrieve_text`.
- Added new method `has_font` to `Ilmenite` to query if a font has been loaded.
- Various types now implement `Copy`: `ImtWeight`, `ImtFillQuality`, `ImtSampleQuality`, `ImtVertAlign`, `ImtHoriAlign`, & `ImtTextWrap`.

# Version 0.12.0 (September 18th, 2022)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.31`.
- **BREAKING** Update dependency `allsorts` to `0.11`.
- **BREAKING** Changes to `ImtImageView`
  - `new` has been renamed to `from_storage`.
  - Added `from_attachment`.
  - Added `image_view_ref` to retreive the inner image view.
  - Fixed bug that caused needless image transitions.
- **BREAKING** `ImtFontProps` now has four additional fields.
  - `min_x`, `max_x`, `min_y`, `max_y` that are from the `head` header.
- Changes to rasterization (GPU Only for Now)
  - Gain curve is now applied to values resulting in clearer text.
  - Subpixels now have a boxy filter applied similar to freetype's default lcd filter.

# Version 0.11.0 (July 20th, 2022)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.30`.
- Update dependency `ordered-float` to `3.0`.
- Update dependency `allsorts` to `0.10`.
    - Covers CVE-2022-24713
    - Reduced features reducing dependencies.

# Version 0.10.0 (March 22nd, 2022)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.29`.
- Fixed issue where an empty buffer was created during raster(gpu) when a glyph didn't contain any countours but had a size.
    - GPU raster will now set `ImtBitmapData::Empty` when there are no countours. This was already set previously when size was zero.
    - CPU raster will now set `ImtBitmapData::Empty` when size is zero or there are no counters.

# Version 0.9.0 (January 5th, 2022)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.28`.

# Version 0.8.0 (December 7th, 2021)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.27.1`.

# Version 0.7.0 (Octoboer 3rd, 2021)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.26.0`.
- **BREAKING** `ImtRasterOpts` now has `raster_image_format` field to use a custom format for images.
    - This requires `shader_storage_image_write_without_format` feature.
        - Most desktop/laptop gpu's support this feature.
    - Added method to root of crate, `ilmenite_required_vk_features()`.

# Version 0.6.0 (August 1st, 2021)

- **BREAKING** Previous change "Bitmap data color componenents are now value normalized." was incorrect. This made values useless for the most part. Now color values will not be normalized to anything, but will rather represent their actual values. The alpha value will now be the max color component similar to how "other" font rasterizers output.

# Version 0.5.1 (July 30th, 2021)

- Specify vulkan & spirv version in shader. Resolves issue with current release of vulkano not detecting storage buffers correctly when spirv 1.0 is used. This also resolves issue with gpu acceleration not working on nvidia cards.

# Version 0.5.0 (July 28th, 2021)

- **BREAKING** `ImtGlyphBitmap` `data` field is now private. Bitmap data is now represented by `ImtBitmapData` enum which can be an image, raw data in the form of a vec, or empty in the case where a bitmap is applicable.
- **BREAKING** `ImtGlyphBitmap` `width`, `height`, `bearing_x`, `bearing_y` have been moved into `ImtBitmapMetrics` which can be obtained from the `ImtGlypyBitmap::metrics()` method.
- **BREAKING** `ImtGlyph` `bitmap` field now is an option of `ImtBitmapData` instead of a vec of the raw data.
- **BREAKING** Bitmap data color componenents are now value normalized. This is the same as `vec4(color.rgb / color.a, color.a)`. This behavior already existed in `Basalt` therefore `Basalt` users will not see any change from this other than a minor performance improvement.
- **BREAKING** `ImtRaster` now has two creation methods, `new_gpu` and `new_cpu`. This will select the rasterization backend used. `ImtFont` methods `from_file` & `from_bytes` have been split into `from_file_cpu`, `from_file_gpu`, `from_bytes_cpu`, & `from_bytes_gpu` to match this change.
- Added `ImtImageView` which is very similar to `BstImageView` from `Basalt`. This is an abstraction over `vulkano`'s `ImageView` that makes handling `ImageViews` more abstract.
- Added `raster_to_image` option to `ImtRasterOpts` which defaults to true. This option will enable/disable outputing to an image instead of raw data.
- Update dependencies `allsorts` to `0.6.0` & `ordered-float` to `2.7.0`.

# Version 0.4.1 (July 20th, 2021)

- `ImtRasterOpts` now has `cpu_rasterization` option to specify if cpu should be used instead of gpu for rasterization. Nvidia seems to be broken currently for gpu rasterization, so as a result this currently defaults to `true`.
- `ImtGlyphBitmap` now has the method `raster_cpu` to rasterize on the cpu instead of gpu.

# Version 0.4.0 (July 4th, 2021)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.24.0`.

# Version 0.3.0 (May 29th, 2021)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.23.0`
- Update dependency `ordered-float` from `2.0` to `2.5`.

# Version 0.2.0 (January 31st, 2021)

- **BREAKING** Update dependency `vulkano` & `vulkano-shaders` to `0.20.0`.
- Update dependency `allsorts` to `0.5.1`.
- Update dependency `ordered-float` to `2.0.1`.
- Update dependency `parking_lot` to `0.11.1`.
- Update dependency `crossbeam` to `0.8.0`

# Version 0.1.0 (June 16th, 2020)

- **BREAKING** Renamed `ImtRasterOps` to `ImtRasterOpts` to improve naming consistency.
- **BREAKING** Added `align_whole_pixels` to `ImtShapeOpts` & `ImtRasterOps` which defaults to `true`.
