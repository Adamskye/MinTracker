use egui::{FontFamily, RichText};

pub trait FontStylingEx {
    fn bold_ex(self) -> Self;
    fn subheading_ex(self) -> Self;
}

impl FontStylingEx for RichText {
    fn bold_ex(self) -> Self {
        self.family(FontFamily::Name("Bold".into()))
    }

    fn subheading_ex(self) -> Self {
        self.family(FontFamily::Name("Bold".into())).size(16.0)
    }
}
