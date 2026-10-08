use crate::storage::error;
use image::{AnimationDecoder, ImageDecoder, ImageReader};
use std::{
    collections::HashMap,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

pub struct Request {
    pub path: PathBuf,
    pub side: u32,
}
pub struct Decoded {
    pub path: PathBuf,
    pub side: u32,
    pub result: Result<(image::RgbaImage, Option<Duration>), String>,
}
pub struct Decoder {
    pub tx: mpsc::SyncSender<Request>,
    pub rx: mpsc::Receiver<Decoded>,
}

pub fn dimensions(path: &Path) -> Result<(u32, u32), String> {
    ImageReader::open(path)
        .map_err(error)?
        .with_guessed_format()
        .map_err(error)?
        .into_dimensions()
        .map_err(error)
}

struct Animation {
    frames: image::Frames<'static>,
    touched: Instant,
    bytes: u64,
}

pub fn memory_budget() -> u64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        // The API writes exactly the structure described by dwLength.
        if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
            return status.ullAvailPhys / 3;
        }
    }
    512 * 1024 * 1024
}

fn gif_frames(path: &Path) -> Result<image::Frames<'static>, String> {
    let file = BufReader::new(File::open(path).map_err(error)?);
    let mut decoder = image::codecs::gif::GifDecoder::new(file).map_err(error)?;
    let mut limits = image::Limits::no_limits();
    limits.max_alloc = Some(memory_budget());
    decoder.set_limits(limits).map_err(error)?;
    Ok(decoder.into_frames())
}

impl Decoder {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(4);
        let (results, rx) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            let mut animations: HashMap<PathBuf, Animation> = HashMap::new();
            while let Ok(request) = requests.recv() {
                animations.retain(|_, a| a.touched.elapsed() < Duration::from_secs(5));
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    decode(&request, &mut animations)
                }))
                .unwrap_or_else(|_| Err("圖片解碼失敗；請檢查來源檔案或可用記憶體".into()));
                if results
                    .send(Decoded {
                        path: request.path,
                        side: request.side,
                        result,
                    })
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { tx, rx }
    }
}

fn decode(
    request: &Request,
    animations: &mut HashMap<PathBuf, Animation>,
) -> Result<(image::RgbaImage, Option<Duration>), String> {
    let mut reader = ImageReader::open(&request.path)
        .map_err(error)?
        .with_guessed_format()
        .map_err(error)?;
    let (width, height) = dimensions(&request.path)?;
    let estimated = (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(16);
    let budget = memory_budget();
    if estimated > budget {
        return Err(format!(
            "可用記憶體不足：此圖片解碼預估需要 {} MB，目前作業預算 {} MB；請釋放記憶體後重試",
            estimated / 1048576,
            budget / 1048576
        ));
    }
    while animations.values().map(|a| a.bytes).sum::<u64>() > budget / 2 {
        if let Some(oldest) = animations
            .iter()
            .min_by_key(|(_, a)| a.touched)
            .map(|(p, _)| p.clone())
        {
            animations.remove(&oldest);
        } else {
            break;
        }
    }
    let (picture, delay) = if reader.format() == Some(image::ImageFormat::Gif) {
        if !animations.contains_key(&request.path) {
            animations.insert(
                request.path.clone(),
                Animation {
                    frames: gif_frames(&request.path)?,
                    touched: Instant::now(),
                    bytes: estimated,
                },
            );
        }
        let animation = animations.get_mut(&request.path).unwrap();
        animation.touched = Instant::now();
        let frame = match animation.frames.next() {
            Some(frame) => frame.map_err(error)?,
            None => {
                animation.frames = gif_frames(&request.path)?;
                animation
                    .frames
                    .next()
                    .ok_or("GIF 沒有影格")?
                    .map_err(error)?
            }
        };
        let (num, den) = frame.delay().numer_denom_ms();
        let delay = Duration::from_secs_f64((num as f64 / den.max(1) as f64 / 1000.0).max(0.02));
        (
            image::DynamicImage::ImageRgba8(frame.into_buffer()),
            Some(delay),
        )
    } else {
        // No dimension/count restriction; allocation exhaustion is reported by the decoder.
        reader.no_limits();
        (reader.decode().map_err(error)?, None)
    };
    let picture = if picture.width().max(picture.height()) > request.side {
        picture.resize(
            request.side,
            request.side,
            image::imageops::FilterType::Triangle,
        )
    } else {
        picture
    };
    Ok((picture.to_rgba8(), delay))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "explicit 60-second sustained animation decoding check"]
    fn sustained_gif_decoding_keeps_stream_state_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base.gif");
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(File::create(&base).unwrap());
            for color in [[255, 0, 0, 255], [0, 255, 0, 255]] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(128, 128, image::Rgba(color)),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(20, 1),
                    ))
                    .unwrap();
            }
        }
        let paths: Vec<_> = (0..128)
            .map(|i| {
                let path = dir.path().join(format!("{i}.gif"));
                std::fs::copy(&base, &path).unwrap();
                path
            })
            .collect();
        let mut animations = HashMap::new();
        let start = Instant::now();
        let mut frames = 0;
        while start.elapsed() < Duration::from_secs(60) {
            for path in &paths {
                let (picture, delay) = decode(
                    &Request {
                        path: path.clone(),
                        side: 64,
                    },
                    &mut animations,
                )
                .unwrap();
                assert_eq!(picture.dimensions(), (64, 64));
                assert_eq!(delay, Some(Duration::from_millis(20)));
                let expected = if (frames / paths.len()) % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 255, 0, 255]
                };
                assert_eq!(picture.get_pixel(0, 0).0, expected);
                frames += 1;
            }
            assert_eq!(animations.len(), paths.len());
            assert_eq!(
                animations.values().map(|a| a.bytes).sum::<u64>(),
                128 * 128 * 16 * 128
            );
        }
        std::fs::create_dir_all("test-output").unwrap();
        std::fs::write("test-output/gif-stress.txt", format!("sources=128\nframes={frames}\nseconds={:.2}\nstream_states={}\nestimated_stream_bytes={}\n", start.elapsed().as_secs_f64(), animations.len(), animations.values().map(|a|a.bytes).sum::<u64>())).unwrap();
    }

    #[test]
    fn formats_and_large_width_decode_without_dimension_cap() {
        let dir = tempfile::tempdir().unwrap();
        for ext in ["png", "jpg", "bmp", "webp"] {
            let p = dir.path().join(format!("test.{ext}"));
            image::RgbImage::from_pixel(80, 40, image::Rgb([90, 150, 210]))
                .save(&p)
                .unwrap();
            let (pixels, delay) =
                decode(&Request { path: p, side: 32 }, &mut HashMap::new()).unwrap();
            assert_eq!(pixels.dimensions(), (32, 16));
            assert!(delay.is_none());
        }
        let p = dir.path().join("wide.png");
        image::RgbImage::new(20000, 10).save(&p).unwrap();
        assert_eq!(dimensions(&p).unwrap(), (20000, 10));
        assert_eq!(
            decode(
                &Request {
                    path: p,
                    side: 2048
                },
                &mut HashMap::new()
            )
            .unwrap()
            .0
            .width(),
            2048
        );
    }
    #[test]
    fn gif_advances_frames_and_repeats_without_collecting_frames() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("test.gif");
        {
            let mut encoder = image::codecs::gif::GifEncoder::new(File::create(&p).unwrap());
            for color in [[255, 0, 0, 255], [0, 255, 0, 255]] {
                encoder
                    .encode_frame(image::Frame::from_parts(
                        image::RgbaImage::from_pixel(20, 20, image::Rgba(color)),
                        0,
                        0,
                        image::Delay::from_numer_denom_ms(100, 1),
                    ))
                    .unwrap();
            }
        }
        let mut state = HashMap::new();
        let r = Request { path: p, side: 32 };
        let a = decode(&r, &mut state).unwrap();
        let b = decode(&r, &mut state).unwrap();
        let c = decode(&r, &mut state).unwrap();
        assert_ne!(a.0, b.0);
        assert_eq!(a.0, c.0);
        assert_eq!(a.1, Some(Duration::from_millis(100)));
    }
}
