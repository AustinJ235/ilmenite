use allsorts::error::ParseError;
use allsorts::error::ShapingError;

#[derive(Clone,Debug,PartialEq)]
pub struct ImtError {
	pub src: ImtErrorSrc,
	pub ty: ImtErrorTy,
}

#[derive(Clone,Debug,PartialEq)]
pub enum ImtErrorSrc {
	Unknown,
	File,
	Cmap,
	Maxp,
	GDEF,
	GPOS,
	Hhea,
	Hmtx,
	Head,
	Loca,
	Glyf,
	Gsub,
	GsubInfo,
	Glyph,
	Bitmap,
	Vhea,
	Ilmenite,
	Shaper,
}

#[derive(Clone,Debug,PartialEq)]
pub enum ImtErrorTy {
	Unimplemented,
	FileRead,
	FileGeneric,
	FileBadEof,
	FileBadValue,
	FileBadVersion,
	FileBadOffset,
	FileBadIndex,
	FileLimitExceeded,
	FileMissingValue,
	FileCompressionError,
	FileUnsupportedFormat,
	FileMissingTable,
	FileMissingSubTable,
	MissingIndex,
	MissingGlyph,
	MissingFont,
	UnimplementedDataTy,
	Other(String),
}

impl ImtError {
	pub fn unimplemented() -> Self {
		Self::src_and_ty(
			ImtErrorSrc::Unknown,
			ImtErrorTy::Unimplemented
		)
	}

	pub fn src_and_ty(src: ImtErrorSrc, ty: ImtErrorTy) -> Self {
		ImtError {
			src,
			ty,
		}
	}
	
	pub fn allsorts_parse(src: ImtErrorSrc, err: ParseError) -> Self {
		ImtError {
			src: src,
			ty: match err {
				ParseError::BadEof => ImtErrorTy::FileBadEof,
				ParseError::BadValue => ImtErrorTy::FileBadValue,
				ParseError::BadVersion => ImtErrorTy::FileBadVersion,
				ParseError::BadOffset => ImtErrorTy::FileBadOffset,
				ParseError::BadIndex => ImtErrorTy::FileBadIndex,
				ParseError::LimitExceeded => ImtErrorTy::FileLimitExceeded,
				ParseError::MissingValue => ImtErrorTy::FileMissingValue,
				ParseError::CompressionError => ImtErrorTy::FileCompressionError,
				ParseError::NotImplemented => ImtErrorTy::FileGeneric,
			}
		}
	}
	
	// TODO: Implement mapping of ShapingError
	pub fn allsorts_shaping(src: ImtErrorSrc, err: ShapingError) -> Self {
		
		println!("Basalt Text: Returning unimplemented error! src: {:?}, err: {:?}", src, err);
		
		Self::src_and_ty(
			src,
			ImtErrorTy::Unimplemented
		)
	}
}
