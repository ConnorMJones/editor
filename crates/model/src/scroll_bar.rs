use crate::units::Px;

pub const WIDTH: Px = Px(10.0);
pub const LEAST_THUMB: Px = Px(24.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thumb {
    pub top: f32,
    pub length: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OnTrack {
    pub thumb: Thumb,
    pub shown_top: f32,
    pub shown_length: f32,
}

impl Thumb {
    pub fn of(start: usize, shown: usize, total: usize) -> Option<Thumb> {
        let total = Some(total as f32).filter(|total| *total > 0.0)?;
        Some(Thumb {
            top: start as f32 / total,
            length: shown as f32 / total,
        })
    }
}

impl OnTrack {
    pub fn of(thumb: Thumb, track: Px) -> Option<OnTrack> {
        let shown_length = thumb.length.max(LEAST_THUMB.0 / track.0).min(1.0);
        let room = Some(1.0 - shown_length).filter(|room| *room > 0.0)?;
        let travel = Some(1.0 - thumb.length).filter(|travel| *travel > 0.0)?;
        Some(OnTrack {
            thumb,
            shown_top: (thumb.top / travel).clamp(0.0, 1.0) * room,
            shown_length,
        })
    }

    pub fn grab(&self, along: f32) -> f32 {
        match along - self.shown_top {
            inside if (0.0..=self.shown_length).contains(&inside) => inside,
            _ => self.shown_length / 2.0,
        }
    }

    pub fn top_at(&self, along: f32) -> f32 {
        let room = 1.0 - self.shown_length;
        let travel = 1.0 - self.thumb.length;
        (along / room * travel).clamp(0.0, travel)
    }

    pub fn index_at(&self, along: f32, total: usize) -> usize {
        (self.top_at(along) * total as f32).round() as usize
    }
}
