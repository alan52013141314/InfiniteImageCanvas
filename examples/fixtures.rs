use image::{Delay, Frame, ImageBuffer, Rgba};
fn main() {
    let dir = std::path::Path::new("test-output/fixtures");
    std::fs::create_dir_all(dir).unwrap();
    for (name, color) in [
        ("coral.png", [229u8, 127, 106, 255]),
        ("teal.webp", [67, 160, 155, 255]),
        ("gold.bmp", [226, 186, 87, 255]),
    ] {
        let image = ImageBuffer::from_fn(480, 320, |x, y| {
            if (x / 40 + y / 40) % 2 == 0 {
                Rgba(color)
            } else {
                Rgba([color[0] / 2, color[1] / 2, color[2] / 2, 255])
            }
        });
        image.save(dir.join(name)).unwrap();
    }
    image::RgbImage::from_pixel(480, 320, image::Rgb([110, 140, 220]))
        .save(dir.join("blue.jpg"))
        .unwrap();
    let mut gif = image::codecs::gif::GifEncoder::new(
        std::fs::File::create(dir.join("animated.gif")).unwrap(),
    );
    gif.set_repeat(image::codecs::gif::Repeat::Infinite)
        .unwrap();
    for color in [
        [180, 70, 140, 255],
        [80, 130, 220, 255],
        [80, 200, 120, 255],
    ] {
        gif.encode_frame(Frame::from_parts(
            ImageBuffer::from_pixel(320, 240, Rgba(color)),
            0,
            0,
            Delay::from_numer_denom_ms(500, 1),
        ))
        .unwrap();
    }
    // Wider than common GPU texture limits. Original remains intact while display uses a level of detail.
    ImageBuffer::from_fn(20000, 128, |x, _| Rgba([(x % 256) as u8, 110, 210, 255]))
        .save(dir.join("wide-20000.png"))
        .unwrap();
    println!("{}", dir.canonicalize().unwrap().display());
}
