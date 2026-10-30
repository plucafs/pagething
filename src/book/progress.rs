#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStage {
    Opening,
    ReadingChapters,
    ReadingImages,
    ConvertingChapters,
    Typesetting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadProgress {
    pub stage: LoadStage,
    pub current: usize,
    pub total: usize,
}

impl LoadProgress {
    pub fn fraction(self) -> f32 {
        let stage_fraction = if self.total == 0 {
            1.0
        } else {
            (self.current as f32 / self.total as f32).clamp(0.0, 1.0)
        };
        let (start, end) = match self.stage {
            LoadStage::Opening => return 0.0,
            LoadStage::ReadingChapters => (0.0, 0.40),
            LoadStage::ReadingImages => (0.40, 0.70),
            LoadStage::ConvertingChapters => (0.70, 0.95),
            LoadStage::Typesetting => return 0.95,
        };
        start + (end - start) * stage_fraction
    }

    pub fn label(self) -> String {
        match self.stage {
            LoadStage::Opening => "Opening EPUB…".to_owned(),
            LoadStage::ReadingChapters => {
                format!("Reading chapters: {} / {}", self.current, self.total)
            }
            LoadStage::ReadingImages => {
                format!("Reading images: {} / {}", self.current, self.total)
            }
            LoadStage::ConvertingChapters => {
                format!("Converting chapters: {} / {}", self.current, self.total)
            }
            LoadStage::Typesetting => "Typesetting pages…".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_progress_advances_by_stage_and_reserves_typesetting() {
        let before_images = LoadProgress {
            stage: LoadStage::ReadingChapters,
            current: 10,
            total: 10,
        };
        let images_start = LoadProgress {
            stage: LoadStage::ReadingImages,
            current: 0,
            total: 4,
        };
        let typesetting = LoadProgress {
            stage: LoadStage::Typesetting,
            current: 0,
            total: 1,
        };

        assert_eq!(before_images.fraction(), 0.40);
        assert_eq!(images_start.fraction(), 0.40);
        assert_eq!(typesetting.fraction(), 0.95);
        assert!(typesetting.label().contains("Typesetting"));
    }
}
