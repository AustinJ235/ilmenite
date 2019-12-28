extern crate basalt;

use basalt::Basalt;
use basalt::interface::bin::{self,BinStyle};

fn main() {
	let basalt = Basalt::new(
		basalt::Options::default()
			.ignore_dpi(true)
			.window_size(800, 54)
			.title("Basalt")
	).unwrap();
	
	basalt.spawn_app_loop();
	
	let test = basalt.interface_ref().new_bin();
	
	test.style_update(BinStyle {
		position_t: Some(bin::PositionTy::FromParent),
		pos_from_t: Some(10.0),
		pos_from_b: Some(10.0),
		pos_from_l: Some(10.0),
		pos_from_r: Some(10.0),
		text: String::from("The quick brown fox jumps over the lazy dog."),
		text_height: Some(36.0),
		text_color: Some(bin::Color::srgb_hex("ffffff")),
		.. BinStyle::default()
	});
	
	basalt.wait_for_exit().unwrap();
}

