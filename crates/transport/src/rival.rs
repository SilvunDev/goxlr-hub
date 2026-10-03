/// Programs that drive the GoXLR, by process name, with the name to show.
const RIVALS: [(&str, &str); 2] = [
    ("goxlr-daemon", "GoXLR Utility"),
    ("goxlr app", "GoXLR App"),
];

/// Recognises a rival from the name of its process.
pub(crate) fn rival_named(process: &str) -> Option<&'static str> {
    let name = process.trim().to_lowercase();
    let name = name.strip_suffix(".exe").unwrap_or(&name);
    RIVALS
        .iter()
        .find(|(process, _)| *process == name)
        .map(|(_, shown)| *shown)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_programs_that_drive_the_device() {
        assert_eq!(rival_named("goxlr-daemon.exe"), Some("GoXLR Utility"));
        assert_eq!(rival_named("goxlr-daemon"), Some("GoXLR Utility"));
        assert_eq!(rival_named("GOXLR-DAEMON.EXE"), Some("GoXLR Utility"));
        assert_eq!(rival_named("goxlr-daemon\n"), Some("GoXLR Utility"));
        assert_eq!(rival_named("GoXLR App.exe"), Some("GoXLR App"));
    }

    #[test]
    fn leaves_the_other_programs_alone() {
        // Ourselves, the window of GoXLR Utility (it does not hold the
        // device), the control panel of the driver.
        for name in [
            "goxlr-hub.exe",
            "goxlr-utility-ui.exe",
            "GoXLRAudioCplApp.exe",
            "goxlr-daemon-helper",
            "",
        ] {
            assert_eq!(rival_named(name), None, "{name}");
        }
    }
}
