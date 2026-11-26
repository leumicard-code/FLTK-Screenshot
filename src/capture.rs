use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use ashpd::desktop::screenshot::Screenshot;

/// מצב הלכידה: מסך מלא מיידי, או בחירת אזור אינטראקטיבית
#[derive(Debug, Clone, Copy)]
pub enum CaptureMode {
    FullScreen,
    Interactive,
}

fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create background Tokio runtime")
    })
}

fn temp_output_path() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("quickshot_{ts}.png"))
}

fn is_kde_session() -> bool {
    std::env::var("XDG_CURRENT_DESKTOP")
        .map(|v| v.to_ascii_uppercase().contains("KDE"))
        .unwrap_or(false)
}

/// מבצע צילום מסך מיידי דרך Spectacle (ללא דיאלוג אישור ב-KDE),
/// ועם Fallback אוטומטי ל-XDG Desktop Portal בדסקטופים אחרים או אם Spectacle לא מותקן.
pub async fn take_screenshot(mode: CaptureMode) -> Result<PathBuf> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    tokio_runtime().spawn(async move {
        let result = if is_kde_session() {
            match capture_with_spectacle(mode).await {
                Ok(Some(path)) => Ok(path),
                Ok(None) => capture_with_portal(mode).await, // spectacle לא מותקן -> fallback
                Err(err) => Err(err),
            }
        } else {
            capture_with_portal(mode).await
        };
        let _ = tx.send(result);
    });

    rx.await.context("Capture task dropped unexpectedly")?
}

/// מחזיר Ok(Some(path)) אם הצליח, Ok(None) אם spectacle לא מותקן, או Err אם בוטל/נכשל
async fn capture_with_spectacle(mode: CaptureMode) -> Result<Option<PathBuf>> {
    let output_path = temp_output_path();
    let mode_flag = match mode {
        CaptureMode::FullScreen => "-f",
        CaptureMode::Interactive => "-r",
    };

    let path_clone = output_path.clone();
    let status = tokio::task::spawn_blocking(move || {
        Command::new("spectacle")
            .args(["-b", "-n", mode_flag, "-o"])
            .arg(&path_clone)
            .status()
    })
    .await
    .context("Failed to join spectacle task")?;

    match status {
        Ok(exit) if exit.success() && output_path.exists() => Ok(Some(output_path)),
        Ok(_) => bail!("הצילום בוטל"),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("Failed to launch spectacle"),
    }
}

async fn capture_with_portal(mode: CaptureMode) -> Result<PathBuf> {
    let interactive = matches!(mode, CaptureMode::Interactive);

    let response = Screenshot::request()
        .interactive(interactive)
        .modal(true)
        .send()
        .await
        .context("Failed to send screenshot request to XDG Portal")?
        .response()
        .context("Screenshot was cancelled or failed")?;

    let uri = response.uri();
    uri.to_file_path()
        .map_err(|_| anyhow!("Portal returned a non-file URI: {uri}"))
}
