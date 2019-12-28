**Many features are incomplete. This crate is a WIP!**

A rust library for shaping, placing, and rasterizing text primarily for Basalt. 

```rust
let ilmenite = Ilmenite::new();

ilmenite.add_font(ImtFont::from_file(
	"MyFont",
	ImtWeight::Normal, 
	ImtRasterOps::default(),
	device,
	queue,
	"MyFont.ttf"
).unwrap());

let glyphs = ilmenite.glyphs_for_text(
	"MyFont",
	ImtWeight::Normal,
	12.0,
	None,
	"Hello World!"
).unwrap();
```
