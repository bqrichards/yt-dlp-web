pub struct Port(u16);

impl Port {
    /// Creates a port object from environment. Fallback to `default`.
    pub fn env(default: u16) -> Self {
        Self(
            std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(default),
        )
    }

    pub fn get(&self) -> u16 {
        self.0
    }
}
