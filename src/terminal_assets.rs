use crate::paths::AppPaths;
use anyhow::{Context, Result};
use std::{fs::OpenOptions, io::Write, path::Path};

pub const GHOSTTY_SHADER: &str = r#"// SaveMyTerminal original ambient shader
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;
    vec4 terminal = texture(iChannel0, uv);
    vec3 cursor = iCurrentCursorColor.rgb;
    vec3 signatureColor = vec3(139.0, 92.0, 246.0) / 255.0;
    float signature = 1.0 - step(0.035, distance(cursor, signatureColor));
    vec2 delta = uv - vec2(0.86, 0.16);
    delta.x *= iResolution.x / iResolution.y;
    float radius = length(delta);
    float ring = exp(-80.0 * abs(radius - 0.09));
    float haze = exp(-18.0 * radius) * (0.72 + 0.28 * sin(iTime * 1.8));
    vec3 ambient = mix(vec3(0.12, 0.38, 0.76), cursor, 0.55);
    float strength = signature * (0.07 * haze + 0.12 * ring);
    fragColor = vec4(mix(terminal.rgb, ambient, strength), terminal.a);
}
"#;

pub fn asset_dir(paths: &AppPaths) -> std::path::PathBuf {
    paths.config_dir.join("assets")
}

pub fn ambient_path(paths: &AppPaths) -> std::path::PathBuf {
    asset_dir(paths).join("savemyterminal-ambient.png")
}

pub fn shader_path(paths: &AppPaths) -> std::path::PathBuf {
    asset_dir(paths).join("savemyterminal.glsl")
}

pub fn install(paths: &AppPaths) -> Result<()> {
    write_atomic(&ambient_path(paths), &ambient_png()?)?;
    write_atomic(&shader_path(paths), GHOSTTY_SHADER.as_bytes())?;
    Ok(())
}

pub fn uninstall(paths: &AppPaths) -> Result<()> {
    for path in [ambient_path(paths), shader_path(paths)] {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("could not remove {}", path.display()));
            }
        }
    }
    let _ = std::fs::remove_dir(asset_dir(paths));
    Ok(())
}

pub fn ambient_png() -> Result<Vec<u8>> {
    const WIDTH: usize = 320;
    const HEIGHT: usize = 200;
    let mut pixels = vec![0_u8; WIDTH * HEIGHT * 4];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let nx = (x as f32 / WIDTH as f32) - 0.78;
            let ny = (y as f32 / HEIGHT as f32) - 0.22;
            let distance = (nx * nx + ny * ny).sqrt();
            let ring = (-((distance - 0.12).abs()) * 48.0).exp();
            let haze = (-distance * 9.0).exp();
            let alpha = ((ring * 72.0) + (haze * 42.0)).clamp(0.0, 96.0) as u8;
            let offset = (y * WIDTH + x) * 4;
            pixels[offset] = 104;
            pixels[offset + 1] = 82;
            pixels[offset + 2] = 220;
            pixels[offset + 3] = alpha;
        }
    }
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, WIDTH as u32, HEIGHT as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&pixels)?;
    }
    Ok(bytes)
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(content)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temp);
    }
    result
}
