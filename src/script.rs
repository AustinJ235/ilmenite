use allsorts::tag;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImtScript {
    Default,
}

impl ImtScript {
    pub(crate) fn tag(&self) -> u32 {
        match self {
            &ImtScript::Default => tag::from_string("DFLT").unwrap(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImtLang {
    Default,
}

impl ImtLang {
    pub(crate) fn tag(&self) -> u32 {
        match self {
            &ImtLang::Default => tag::from_string("dflt").unwrap(),
        }
    }
}
