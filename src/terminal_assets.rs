use crate::paths::AppPaths;
use anyhow::{Context, Result};
use std::{fs::OpenOptions, io::Write, path::Path};

pub const GHOSTTY_SHADER: &str = r#"// SaveMyTerminal state-reactive black-hole shader
const float STATE_STARTING = 0.0;
const float STATE_THINKING = 1.0;
const float STATE_TOOL_RUNNING = 2.0;
const float STATE_WAITING = 3.0;

float decodeState(vec3 signal) {
    return floor(signal.r * 255.0 + 0.5) - 160.0;
}

float decodeIntensity(vec3 signal) {
    return clamp(signal.g, 0.0, 1.0);
}

float decodeContext(vec3 signal) {
    float encoded = floor(signal.b * 255.0 + 0.5);
    return encoded >= 255.0 ? -1.0 : encoded / 254.0;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;
    vec3 signal = iCurrentCursorColor.rgb;
    float state = decodeState(signal);
    float signalEnabled = step(-0.5, state) * step(state, 6.5);
    float intensity = decodeIntensity(signal);
    float context = decodeContext(signal);
    float pressure = context < 0.0 ? 0.35 : context;

    vec2 center = vec2(0.84, 0.18);
    vec2 delta = uv - center;
    delta.x *= iResolution.x / iResolution.y;
    float radius = length(delta);
    float speed = mix(0.35, 2.4, step(STATE_THINKING - 0.5, state));
    speed = mix(speed, 3.6, step(STATE_TOOL_RUNNING - 0.5, state));
    speed = mix(speed, 0.18, step(STATE_WAITING - 0.5, state));
    float angle = atan(delta.y, delta.x) + iTime * speed;

    float lens = signalEnabled * intensity * smoothstep(0.22, 0.025, radius) * 0.018;
    vec2 warped = uv + normalize(delta + vec2(0.0001)) * lens;
    vec4 terminal = texture(iChannel0, warped);

    float horizon = 1.0 - smoothstep(0.035, 0.052, radius);
    float diskRadius = mix(0.06, 0.17, pressure);
    float disk = exp(-95.0 * abs(radius - diskRadius));
    disk *= 0.62 + 0.38 * sin(angle * 3.0 - iTime * speed);
    float photonRing = exp(-180.0 * abs(radius - 0.055));
    float haze = exp(-15.0 * radius) * (0.7 + 0.3 * sin(iTime * speed));

    vec3 stateColor = vec3(0.39, 0.40, 0.95);
    if (state >= STATE_THINKING - 0.5) stateColor = vec3(0.55, 0.36, 0.96);
    if (state >= STATE_TOOL_RUNNING - 0.5) stateColor = vec3(0.96, 0.62, 0.08);
    if (state >= STATE_WAITING - 0.5) stateColor = vec3(0.02, 0.71, 0.83);

    // Context-pressure warning: shift toward red as the window nears full.
    float warn = smoothstep(0.8, 1.0, context) * signalEnabled;
    stateColor = mix(stateColor, vec3(0.95, 0.16, 0.18), warn);

    float glow = signalEnabled * intensity * (0.18 * disk + 0.24 * photonRing + 0.07 * haze);
    vec3 composed = mix(terminal.rgb, stateColor, clamp(glow, 0.0, 0.7));
    composed *= 1.0 - signalEnabled * horizon * 0.96;
    fragColor = vec4(composed, terminal.a);
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
