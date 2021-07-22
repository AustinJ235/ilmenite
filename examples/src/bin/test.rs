extern crate basalt;

use basalt::Basalt;
use basalt::interface::bin::{self,BinStyle,BinPosition,Color};

fn main() {
	Basalt::initialize(
		basalt::Options::default()
			.ignore_dpi(true)
			.window_size(800, 210)
			.title("Ilmenite Test")
			.gpu_accelered_text(true)
			.app_loop(),
		Box::new(move |basalt_res| {
			let basalt = basalt_res.unwrap();
			let mut tests = basalt.interface_ref().new_bins(8);
			let back = tests.pop().unwrap();
			tests.iter().for_each(|t| back.add_child(t.clone()));
			
			back.style_update(BinStyle {
				pos_from_t: Some(0.0),
				pos_from_b: Some(0.0),
				pos_from_l: Some(0.0),
				pos_from_r: Some(0.0),
				back_color: Some(Color::srgb_hex("303030")),
				.. BinStyle::default()
			});
	
			tests[0].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(36.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(36.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			tests[1].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0 + 36.0 + 10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(24.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(24.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			tests[2].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0 + 36.0 + 10.0 + 24.0 + 10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(18.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(18.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			tests[3].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0 + 36.0 + 10.0 + 24.0 + 10.0 + 18.0 + 10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(16.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(16.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			tests[4].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0 + 36.0 + 10.0 + 24.0 + 10.0 + 18.0 + 10.0 + 16.0 + 10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(14.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(14.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			tests[5].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0 + 36.0 + 10.0 + 24.0 + 10.0 + 18.0 + 10.0 + 16.0 + 10.0 + 14.0 + 10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(12.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(12.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			tests[6].style_update(BinStyle {
				position: Some(BinPosition::Parent),
				pos_from_t: Some(10.0 + 36.0 + 10.0 + 24.0 + 10.0 + 18.0 + 10.0 + 16.0 + 10.0 + 14.0 + 10.0 + 12.0 + 10.0),
				pos_from_l: Some(10.0),
				pos_from_r: Some(10.0),
				height: Some(10.0),
				text: String::from("The quick brown fox jumps over the lazy dog."),
				text_height: Some(10.0),
				text_color: Some(bin::Color::srgb_hex("ffffff")),
				.. BinStyle::default()
			});
			
			let mut t: f32 = 0.0;
			
			loop {
				if basalt.wants_exit() {
					break;
				}
				
				let mut color = Color {
					r: ((0.12*t).sin() + 1.0) / 2.0,
					g: ((0.34*t).sin() + 1.0) / 2.0,
					b: ((0.56*t).sin() + 1.0) / 2.0,
					a: 1.0
				};
				
				color.clamp();
				
				let inv_color = Color {
					r: 1.0 - color.r,
					g: 1.0 - color.g,
					b: 1.0 - color.b,
					a: 1.0
				};
				
				back.style_update(BinStyle {
					back_color: Some(color),
					.. back.style_copy()
				});
				
				tests.iter().for_each(|b| b.style_update(BinStyle {
					text_color: Some(inv_color.clone()),
					.. b.style_copy()
				}));
				
				t += 0.015;
				::std::thread::sleep(::std::time::Duration::from_millis(15));
			}
	
			basalt.wait_for_exit().unwrap();
		})
	);
}
