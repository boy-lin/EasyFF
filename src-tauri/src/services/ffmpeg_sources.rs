use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshSummary {
    pub os: String,
    pub source: String,
    pub fetched: usize,
    pub updated: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegVersionItem {
    pub source: String,
    pub os: String,
    pub version: String,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
    pub arch: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FfmpegVersionListResult {
    pub list: Vec<FfmpegVersionItem>,
    pub total: u64,
    pub has_more: bool,
    pub next_offset: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum HostOs {
    Windows,
    Linux,
    Macos,
}

impl HostOs {
    pub fn as_str(&self) -> &'static str {
        match self {
            HostOs::Windows => "windows",
            HostOs::Linux => "linux",
            HostOs::Macos => "macos",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SourceKind {
    Ling,
    Gyan,
    Btbn,
    JohnVanSickle,
    Evermeet,
    Eugeneware
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceKind::Ling => "ling",
            SourceKind::Gyan => "gyan",
            SourceKind::Btbn => "btbn",
            SourceKind::JohnVanSickle => "johnvansickle",
            SourceKind::Evermeet => "evermeet",
            SourceKind::Eugeneware => "eugeneware",
        }
    }
}

pub fn normalize_os(input: Option<String>) -> HostOs {
    match input
        .unwrap_or_else(|| std::env::consts::OS.to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "linux" => HostOs::Linux,
        "macos" | "darwin" => HostOs::Macos,
        _ => HostOs::Windows,
    }
}

pub fn available_sources(os: HostOs) -> Vec<SourceKind> {
    match os {
        HostOs::Windows => vec![SourceKind::Ling, SourceKind::Gyan, SourceKind::Btbn],
        HostOs::Linux => vec![SourceKind::Ling, SourceKind::JohnVanSickle, SourceKind::Btbn],
        HostOs::Macos => vec![SourceKind::Ling, SourceKind::Evermeet, SourceKind::Btbn, SourceKind::Eugeneware],
    }
}

fn version_item(
    source: SourceKind,
    os: HostOs,
    version: &str,
    published_at: &str,
    download_url: &str,
    arch: Option<&str>,
) -> FfmpegVersionItem {
    FfmpegVersionItem {
        source: source.as_str().to_string(),
        os: os.as_str().to_string(),
        version: version.to_string(),
        published_at: Some(published_at.to_string()),
        download_url: Some(download_url.to_string()),
        arch: arch.map(|v| v.to_string()),
        updated_at: 0,
    }
}

pub fn fetch_versions(
    source: SourceKind,
    os: HostOs,
    arch: Option<String>,
) -> Result<Vec<FfmpegVersionItem>> {
    let list = match (source, os) {
        (SourceKind::Ling, HostOs::Windows) => vec![
            version_item(
                SourceKind::Ling,
                HostOs::Windows,
                "8.0.1",
                "2026-03-13",
                "https://tebi.2342342.xyz/static/ffmpeg/ffmpeg-release-full.7z",
                Some("x86_64"),
            ),
            version_item(
                SourceKind::Ling,
                HostOs::Windows,
                "7.1.1",
                "2026-03-13",
                "https://tebi.2342342.xyz/static/ffmpeg/ffmpeg-7.1.1-full_build.7z",
                Some("x86_64"),
            )
        ],
        (SourceKind::Ling, HostOs::Macos) => vec![
            version_item(
                SourceKind::Ling,
                HostOs::Macos,
                "8.0.0",
                "2026-03-13",
                "https://tebi.2342342.xyz/static/ffmpeg/mac/ffmpeg-8.0.7z",
                Some("x64"),
            ),
            version_item(
                SourceKind::Ling,
                HostOs::Macos,
                "6.1.1",
                "2025-06-18",
                "https://tebi.2342342.xyz/static/ffmpeg/mac/ffmpeg-darwin-arm64.gz",
                Some("arm64"),
            ),
            version_item(
                SourceKind::Ling,
                HostOs::Macos,
                "5.0.1",
                "2022-06-29",
                "https://tebi.2342342.xyz/static/ffmpeg/mac/b5.0.1-darwin-arm64.gz",
                Some("arm64"),
            ),
            
        ],
        (SourceKind::Btbn, HostOs::Windows) => vec![
            version_item(
                SourceKind::Btbn,
                HostOs::Windows,
                "8.0.1",
                "2026-03-07",
                "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-03-07-17-35/ffmpeg-n8.0.1-76-gfa4ee7ab3c-win64-lgpl-8.0.zip",
                Some("x86_64"),
            ),
            version_item(
                SourceKind::Btbn,
                HostOs::Windows,
                "8.0.1",
                "2026-03-07",
                "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-03-07-17-35/ffmpeg-n8.0.1-76-gfa4ee7ab3c-winarm64-lgpl-8.0.zip",
                Some("arm64"),
            ),
        ],
        (SourceKind::Eugeneware, HostOs::Windows) => vec![
            version_item(
                SourceKind::Eugeneware,
                HostOs::Windows,
                "6.1.1",
                "2025-11-15",
                "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-win32-x64.gz",
                Some("x64"),
            ),
        ],
        (SourceKind::Btbn, HostOs::Linux) => vec![
            version_item(
                SourceKind::Btbn,
                HostOs::Linux,
                "8.0.1",
                "2026-03-07",
                "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-03-07-17-35/ffmpeg-n8.0.1-76-gfa4ee7ab3c-linux64-lgpl-8.0.tar.xz",
                Some("x86_64"),
            ),
            version_item(
                SourceKind::Btbn,
                HostOs::Linux,
                "8.0.1",
                "2026-03-07",
                "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-03-07-17-35/ffmpeg-n8.0.1-76-gfa4ee7ab3c-linuxarm64-lgpl-8.0.tar.xz",
                Some("arm64"),
            ),
        ],
        (SourceKind::JohnVanSickle, HostOs::Linux) => vec![
            version_item(
                SourceKind::JohnVanSickle,
                HostOs::Linux,
                "7.0.2",
                "2025-11-04",
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",
                Some("amd64"),
            ),
            version_item(
                SourceKind::JohnVanSickle,
                HostOs::Linux,
                "7.0.2",
                "2025-11-04",
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-arm64-static.tar.xz",
                Some("arm64"),
            ),
            version_item(
                SourceKind::JohnVanSickle,
                HostOs::Linux,
                "7.0.2",
                "2025-11-04",
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-i686-static.tar.xz",
                Some("i686"),
            ),
            version_item(
                SourceKind::JohnVanSickle,
                HostOs::Linux,
                "7.0.2",
                "2025-11-04",
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-armhf-static.tar.xz",
                Some("armhf"),
            ),
            version_item(
                SourceKind::JohnVanSickle,
                HostOs::Linux,
                "7.0.2",
                "2025-11-04",
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-armel-static.tar.xz",
                Some("armel"),
            )
        ],
        (SourceKind::Evermeet, HostOs::Macos) => vec![
            version_item(
                SourceKind::Evermeet,
                HostOs::Macos,
                "8.0",
                "2026-03-12",
                "https://deolaha.ca/pub/ffmpeg/ffmpeg-8.0.7z",
                Some("x64"),
            ),
            version_item(
                SourceKind::Evermeet,
                HostOs::Macos,
                "7.1.1",
                "2026-03-12",
                "https://deolaha.ca/pub/ffmpeg/ffmpeg-7.1.1.7z",
                Some("x64"),
            ),
            version_item(
                SourceKind::Evermeet,
                HostOs::Macos,
                "6.1.1",
                "2026-03-12",
                "https://deolaha.ca/pub/ffmpeg/ffmpeg-6.1.1.7z",
                Some("x64"),
            ),
        ],
        (SourceKind::Eugeneware, HostOs::Macos) => vec![
            version_item(
                SourceKind::Eugeneware,
                HostOs::Macos,
                "6.1.1",
                "2025-06-18",
                "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-darwin-arm64.gz",
                Some("arm64"),
            ),
            version_item(
                SourceKind::Eugeneware,
                HostOs::Macos,
                "6.1.1",
                "2025-06-18",
                "https://github.com/eugeneware/ffmpeg-static/releases/download/b6.1.1/ffmpeg-darwin-x64.gz",
                Some("x64"),
            ),
            version_item(
                SourceKind::Eugeneware,
                HostOs::Macos,
                "5.0.1",
                "2022-06-29",
                "https://github.com/eugeneware/ffmpeg-static/releases/download/b5.0.1/darwin-arm64.gz",
                Some("arm64"),
            ),
            version_item(
                SourceKind::Eugeneware,
                HostOs::Macos,
                "5.0.1",
                "2022-06-29",
                "https://github.com/eugeneware/ffmpeg-static/releases/download/b5.0.1/darwin-x64.gz",
                Some("x64"),
            ),
            
        ],
        
        (SourceKind::Gyan, HostOs::Windows) => vec![
            version_item(
                SourceKind::Gyan,
                HostOs::Windows,
                "latest",
                "2025-11-20",
                "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-full.7z",
                Some("x86_64"),
            ),
            version_item(
                SourceKind::Gyan,
                HostOs::Windows,
                "7.1.1",
                "2025-11-04",
                "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-7.1.1-full_build.7z",
                Some("x86_64"),
            )
        ],
        _ => Vec::new(),
    };

    let arch = arch
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty());

    if let Some(arch) = arch {
        let filtered = list
            .into_iter()
            .filter(|item| {
                item.arch
                    .as_deref()
                    .map(|v| v.eq_ignore_ascii_case(arch.as_str()))
                    .unwrap_or(false)
            })
            .collect();
        return Ok(filtered);
    }

    Ok(list)
}

fn parse_source(input: &str) -> Option<SourceKind> {
    match input.trim().to_ascii_lowercase().as_str() {
        "gyan" => Some(SourceKind::Gyan),
        "btbn" => Some(SourceKind::Btbn),
        "johnvansickle" => Some(SourceKind::JohnVanSickle),
        "evermeet" => Some(SourceKind::Evermeet),
        _ => None,
    }
}

pub fn list_versions(
    source: Option<String>,
    os: Option<String>,
    keyword: Option<String>,
    limit: usize,
    offset: usize,
) -> FfmpegVersionListResult {
    let limit = limit.clamp(1, 200);
    let os_list: Vec<HostOs> = if let Some(raw) = os.filter(|v| !v.trim().is_empty()) {
        vec![normalize_os(Some(raw))]
    } else {
        vec![HostOs::Windows, HostOs::Linux, HostOs::Macos]
    };

    let mut list: Vec<FfmpegVersionItem> = Vec::new();
    for host_os in os_list {
        let target_sources: Vec<SourceKind> =
            if let Some(raw) = source.as_ref().filter(|v| !v.trim().is_empty()) {
                parse_source(raw).map(|s| vec![s]).unwrap_or_default()
            } else {
                available_sources(host_os)
            };

        for src in target_sources {
            if let Ok(mut rows) = fetch_versions(src, host_os, None) {
                list.append(&mut rows);
            }
        }
    }

    if let Some(k) = keyword
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
    {
        list.retain(|item| {
            item.version.to_ascii_lowercase().contains(&k)
                || item.source.to_ascii_lowercase().contains(&k)
                || item
                    .download_url
                    .as_ref()
                    .map(|v| v.to_ascii_lowercase().contains(&k))
                    .unwrap_or(false)
        });
    }

    list.sort_by(|a, b| {
        b.published_at
            .cmp(&a.published_at)
            .then_with(|| b.version.cmp(&a.version))
            .then_with(|| a.source.cmp(&b.source))
    });

    let total = list.len() as u64;
    let page = list
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let next_offset = (offset + page.len()) as u64;
    FfmpegVersionListResult {
        has_more: next_offset < total,
        next_offset,
        list: page,
        total,
    }
}
