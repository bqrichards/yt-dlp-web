mod cmd;
mod download;

pub use download::download_media;

#[derive(Debug, Clone)]
pub struct DownloadComplete {
    pub id: String,
    pub title: String,
    pub media_options: MediaOptions,
}

#[derive(Debug, Clone, Copy)]
pub enum VideoResolution {
    /// 1080p
    Fhd,
    /// 1440p (2K)
    Qhd,
    /// 2160p (4K)
    Uhd,
}

impl VideoResolution {
    pub fn height(&self) -> i32 {
        match self {
            Self::Fhd => 1080,
            Self::Qhd => 1440,
            Self::Uhd => 2160,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MediaOptions {
    Audio,
    Video { max_resolution: VideoResolution },
}
