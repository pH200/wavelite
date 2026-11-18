#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::process::Command;

pub(super) const RULE: &str = include_str!("../../packaging/99-wavelite.wavemic.rules");
pub(super) const RULE_PATH: &str = "/etc/udev/rules.d/99-wavelite.wavemic.rules";
#[cfg(target_os = "linux")]
const INSTALL_SCRIPT: &str = r#"set -eu
rule_dir=${2:-/etc/udev/rules.d}
rule_path="$rule_dir/99-wavelite.wavemic.rules"
temporary_path="${rule_path}.tmp.$$"
umask 022
mkdir -p "$rule_dir"
trap 'rm -f "$temporary_path"' EXIT HUP INT TERM
printf '%s' "$1" > "$temporary_path"
chmod 0644 "$temporary_path"
mv "$temporary_path" "$rule_path"
if command -v udevadm >/dev/null 2>&1; then
    udevadm control --reload-rules || :
    for product in 006e 0070 00ca; do
        udevadm trigger --subsystem-match=usb --attr-match=idVendor=0fd9 --attr-match="idProduct=$product" || :
    done
fi
"#;

#[cfg(target_os = "linux")]
pub fn is_installed() -> bool {
    rule_is_installed_at(Path::new(RULE_PATH))
}

#[cfg(not(target_os = "linux"))]
pub fn is_installed() -> bool {
    true
}

#[cfg(target_os = "linux")]
fn rule_is_installed_at(path: &Path) -> bool {
    std::fs::read_to_string(path).is_ok_and(|contents| has_current_rule(&contents))
}

fn has_current_rule(contents: &str) -> bool {
    contents == RULE
}

#[cfg(target_os = "linux")]
pub fn install() -> Result<(), String> {
    let output = Command::new("pkexec")
        .arg("/bin/sh")
        .arg("-c")
        .arg(INSTALL_SCRIPT)
        .arg("wavelite-udev-install")
        .arg(RULE)
        .output()
        .map_err(|error| {
            format!(
                "Could not start pkexec: {error}. Install polkit or use the manual udev setup instructions."
            )
        })?;

    if output.status.success() {
        if is_installed() {
            return Ok(());
        }
        return Err(
            "The authorization command finished, but the WaveLite udev rule is not installed."
                .to_owned(),
        );
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let details = stderr.trim();
    match output.status.code() {
        Some(126) => {
            Err("Authorization was dismissed. The udev rule was not installed.".to_owned())
        }
        _ if !details.is_empty() => Err(format!("Could not install the udev rule: {details}")),
        _ => Err(format!(
            "Could not install the udev rule (pkexec exit status {}).",
            output.status
        )),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn install() -> Result<(), String> {
    Err("udev rules are only used on Linux.".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{RULE, has_current_rule};

    #[test]
    fn only_the_embedded_rule_satisfies_the_setup_check() {
        assert!(has_current_rule(RULE));
        assert!(!has_current_rule(""));
        assert!(!has_current_rule("SUBSYSTEM==\"usb\", MODE=\"0666\"\n"));
        assert!(!has_current_rule(&format!("{RULE}\n# outdated copy\n")));
    }

    #[test]
    fn embedded_rule_covers_every_supported_product() {
        for product in ["006e", "0070", "00ca"] {
            assert!(RULE.contains(&format!("ATTR{{idProduct}}==\"{product}\"")));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn install_script_writes_the_rule_and_reloads_supported_devices() {
        use std::{os::unix::fs::PermissionsExt, path::PathBuf, process::Command};

        use super::INSTALL_SCRIPT;

        let test_dir =
            PathBuf::from("target").join(format!("udev-install-test-{}", std::process::id()));
        let bin_dir = test_dir.join("bin");
        let rules_dir = test_dir.join("rules.d");
        let log = test_dir.join("udevadm.log");
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&bin_dir).unwrap();
        let udevadm = bin_dir.join("udevadm");
        std::fs::write(
            &udevadm,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$WAVELITE_TEST_UDEVADM_LOG\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&udevadm, std::fs::Permissions::from_mode(0o755)).unwrap();

        let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(INSTALL_SCRIPT)
            .arg("wavelite-udev-test")
            .arg(RULE)
            .arg(&rules_dir)
            .env("PATH", path)
            .env("WAVELITE_TEST_UDEVADM_LOG", &log)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        let installed = rules_dir.join("99-wavelite.wavemic.rules");
        assert_eq!(std::fs::read_to_string(&installed).unwrap(), RULE);
        assert_eq!(
            std::fs::metadata(&installed).unwrap().permissions().mode() & 0o777,
            0o644
        );
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(calls.lines().any(|line| line == "control --reload-rules"));
        for product in ["006e", "0070", "00ca"] {
            assert!(calls.lines().any(|line| {
                line == format!(
                    "trigger --subsystem-match=usb --attr-match=idVendor=0fd9 --attr-match=idProduct={product}"
                )
            }));
        }
        std::fs::remove_dir_all(test_dir).unwrap();
    }
}
