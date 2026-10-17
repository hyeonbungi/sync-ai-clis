# Exactly one successful result for each tool selected by the Windows job.
type == "array" and
(map(.id) | sort == ["antigravity", "claude"]) and
all(.[]; .result == "ok")
