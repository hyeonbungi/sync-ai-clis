# Match the CLI's build gate, not the runner's Windows product name.
# Unknown build (0) is unsupported, just as OsInfo::is_windows_11(None).
type == "array" and
(map(.id) | sort == ["antigravity", "claude", "kiro"]) and
all(.[];
  if .id == "kiro" then
    if $windows_build >= 22000 then
      .result == "ok"
    else
      .result == "skip" and
      .reason == "Kiro requires Windows 11 (SPEC §7.4)"
    end
  else
    .result == "ok"
  end
)
