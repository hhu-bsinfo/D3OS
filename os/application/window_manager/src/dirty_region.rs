use alloc::vec::Vec;
use drawer::rect_data::RectData;

pub struct DirtyRegion {
    pub rect: RectData,
}

impl DirtyRegion {
    pub fn new(rect: RectData) -> Self {
        Self { rect }
    }

    pub fn merge(&mut self, other: &DirtyRegion) {
        let self_top_left = self.rect.top_left;
        let self_bottom_right = self.rect.top_left.add(self.rect.width, self.rect.height);

        let other_top_left = other.rect.top_left;
        let other_bottom_right = other.rect.top_left.add(other.rect.width, other.rect.height);

        let new_top_left = self_top_left.min(other_top_left);
        let new_bottom_right = self_bottom_right.max(other_bottom_right);

        self.rect = RectData {
            top_left: new_top_left,
            width: new_bottom_right.x - new_top_left.x,
            height: new_bottom_right.y - new_top_left.y,
        };
    }
}

pub struct DirtyRegionList {
    regions: Vec<DirtyRegion>,
}

impl DirtyRegionList {
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    pub fn add(&mut self, new_region: DirtyRegion) {
        for region in self.regions.iter_mut() {
            if region.rect.intersects(&new_region.rect) {
                region.merge(&new_region);
                return;
            }
        }

        // no intersection found, add the region
        self.regions.push(new_region);
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }
}