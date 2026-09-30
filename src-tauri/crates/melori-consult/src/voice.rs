pub struct VoiceEdge {
    speaking: bool,
}
impl Default for VoiceEdge {
    fn default() -> Self {
        Self { speaking: false }
    }
}
impl VoiceEdge {
    pub fn update(&mut self, speaking: bool) -> Option<bool> {
        if self.speaking == speaking {
            None
        } else {
            self.speaking = speaking;
            Some(speaking)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn edges_only() {
        let mut v = VoiceEdge::default();
        assert_eq!(v.update(false), None);
        assert_eq!(v.update(true), Some(true));
        assert_eq!(v.update(true), None);
        assert_eq!(v.update(false), Some(false));
    }
}
