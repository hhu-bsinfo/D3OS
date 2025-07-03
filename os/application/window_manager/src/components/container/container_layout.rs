#[derive(Copy, Clone, PartialEq)]
pub enum HorDirection {
    Left,
    Right,
}

#[derive(Copy, Clone, PartialEq)]
pub enum VertDirection {
    Top,
    Bottom,
}

#[derive(Copy, Clone, PartialEq)]
pub enum AlignmentMode {
    None,
    Horizontal(HorDirection),
    Vertical(VertDirection),
    Grid(u32),
}

#[derive(Copy, Clone, PartialEq)]
pub enum StretchMode {
    None,
    Fill,
}

#[derive(Copy, Clone, PartialEq)]
pub enum FitMode {
    None,
    GrowAndShrink,
}

#[derive(Clone, Copy)]
pub struct ContainerLayout {
    pub alignment: AlignmentMode,
    pub stretch: StretchMode,
    pub fit: FitMode,
}

impl Default for ContainerLayout {
    fn default() -> Self {
        ContainerLayoutBuilder::new().build()
    }
}

pub struct ContainerLayoutBuilder {
    alignment: Option<AlignmentMode>,
    stretch: Option<StretchMode>,
    fit: Option<FitMode>,
}

impl ContainerLayoutBuilder {
    pub fn new() -> Self {
        Self {
            alignment: None,
            stretch: None,
            fit: None,
        }
    }

    pub fn alignment(&mut self, alignment: AlignmentMode) -> &mut Self {
        self.alignment = Some(alignment);
        self
    }

    pub fn stretch(&mut self, stretch: StretchMode) -> &mut Self {
        self.stretch = Some(stretch);
        self
    }

    pub fn fit(&mut self, fit: FitMode) -> &mut Self {
        self.fit = Some(fit);
        self
    }

    pub fn build(&mut self) -> ContainerLayout {
        ContainerLayout {
            alignment: self.alignment.unwrap_or(AlignmentMode::None),
            stretch: self.stretch.unwrap_or(StretchMode::None),
            fit: self.fit.unwrap_or(FitMode::None),
        }
    }
}
