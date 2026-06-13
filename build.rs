use std::path::PathBuf;

const ICON_SRC: &str = "assets/app-icon.png";

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn prepare_linux_icon() {
    use image::{ImageReader, imageops};

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");

    // Resize once at build time so the embedded PNG is small; X11 reads the
    // raw RGBA from this, so a 256px source covers all plausible taskbar sizes.
    let image = ImageReader::open(ICON_SRC)
        .expect("opening app icon")
        .with_guessed_format()
        .expect("guessing app icon format")
        .decode()
        .expect("decoding app icon");
    let resized = image.resize_to_fill(256, 256, imageops::FilterType::Lanczos3);

    let out_path = PathBuf::from(&out_dir).join("app_icon.png");
    resized.save(&out_path).expect("writing resized app icon");
    println!("cargo:rerun-if-changed={ICON_SRC}");
}

// Embed the icon as a PE resource on Windows so the .exe carries it (Explorer,
// taskbar, window title bar all read it). No runtime decode needed.
#[cfg(target_os = "windows")]
fn prepare_windows_icon() {
    use image::codecs::ico::IcoEncoder;
    use image::ImageReader;
    use std::io::Cursor;

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR not set");
    let image = ImageReader::open(ICON_SRC)
        .expect("opening app icon")
        .with_guessed_format()
        .expect("guessing app icon format")
        .decode()
        .expect("decoding app icon");

    // A multi-size .ico looks crisp at every taskbar/Explorer scale.
    let sizes = [16usize, 32, 48, 64, 128, 256];
    let frames: Vec<image::codecs::ico::IcoFrame> = sizes
        .iter()
        .map(|&size| {
            let rgba = image
                .resize_exact(size, size, image::imageops::FilterType::Lanczos3)
                .to_rgba8();
            image::codecs::ico::IcoFrame::as_png(
                rgba.as_raw(),
                size as u32,
                size as u32,
                image::ExtendedColorType::Rgba8,
            )
            .expect("encoding ico frame")
        })
        .collect();

    let ico_path = PathBuf::from(&out_dir).join("app_icon.ico");
    let mut buffer = Vec::new();
    IcoEncoder::new(Cursor::new(&mut buffer))
        .encode_images(&frames)
        .expect("encoding .ico");
    std::fs::write(&ico_path, &buffer).expect("writing .ico");

    let mut resource = winresource::WindowsResource::new();
    resource.set_icon(ico_path.to_str().expect("icon path is valid UTF-8"));
    resource.set("FileDescription", "WZed");
    resource.set("ProductName", "WZed");
    if let Err(err) = resource.compile() {
        panic!("failed to compile Windows resource: {err}");
    }
    println!("cargo:rerun-if-changed={ICON_SRC}");
}

fn main() {
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    prepare_linux_icon();
    #[cfg(target_os = "windows")]
    prepare_windows_icon();
}
