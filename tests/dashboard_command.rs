use savemyterminal::app::{BrowserOpener, open_dashboard_url};
use std::sync::Mutex;

#[derive(Default)]
struct RecordingBrowser {
    opened: Mutex<Vec<String>>,
    fail: bool,
}

impl BrowserOpener for RecordingBrowser {
    fn open(&self, url: &str) -> anyhow::Result<()> {
        self.opened.lock().unwrap().push(url.to_owned());
        if self.fail {
            anyhow::bail!("browser unavailable");
        }
        Ok(())
    }
}

#[test]
fn dashboard_opens_the_short_lived_launch_url() {
    let browser = RecordingBrowser::default();
    let url = "http://127.0.0.1:1234/dashboard/launch?token=short-lived";

    open_dashboard_url(&browser, url).unwrap();

    assert_eq!(browser.opened.lock().unwrap().as_slice(), [url]);
}

#[test]
fn browser_failure_returns_the_usable_launch_url() {
    let browser = RecordingBrowser {
        fail: true,
        ..RecordingBrowser::default()
    };
    let url = "http://127.0.0.1:1234/dashboard/launch?token=short-lived";

    let error = open_dashboard_url(&browser, url).unwrap_err().to_string();

    assert!(error.contains("could not open browser"));
    assert!(error.contains(url));
}
