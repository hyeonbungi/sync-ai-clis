# sync-ai-clis — 설계 명세 (SPEC)


| 항목 | 값 |
|------|-----|
| 프로젝트명 | **sync-ai-clis** |
| 한 줄 설명 | 여러 AI 코딩 CLI(Claude Code·Codex·Gemini·Kiro·Antigravity)를 한 명령으로 **감지·설치·최신 유지**하는 크로스플랫폼 도구 |
| 구현 언어 | **Rust** (단일 바이너리) |
| 지원 OS | **macOS · Windows · Linux** |
| 라이선스 | **MIT** |
| GitHub 오너 | 개인 **hyeonbungi** (`github.com/hyeonbungi/sync-ai-clis`) |
| 저작자 | 김현우 <tech@indiedrive.co.kr> |
| 배포 채널 | GitHub Releases · Homebrew tap · npm · crates.io |
| 작성일 | 2026-06-10 |

---

## 1. 배경 & 동기

이 프로젝트는 기존 bash 스크립트 `~/.local/bin/update-ai-clis`(114줄, macOS 전용)에서 출발한다. 원본은 5개 AI CLI를 각자의 설치 경로로 업데이트하고, 업데이트 후 실제 동작(`--version`)까지 재검증하는 잘 만들어진 스크립트였다.

원본의 한계와 확장 요구:
1. **macOS 전용** → Windows·Linux도 지원해야 한다.
2. **업데이트만** 함 → 미설치 도구를 **동의 후 설치**까지 해야 한다.
3. **개인 스크립트** → **MIT 오픈소스**로 GitHub 공개 + Homebrew·npm으로 배포해야 한다.

세 요구가 합쳐지면 "bash 스크립트 수정"이 아니라 **새 크로스플랫폼 CLI 프로젝트**가 된다. 특히 "Windows + npm 배포"는 bash로 불가능하므로 언어부터 새로 정했다.

### 원본에서 반드시 계승할 좋은 동작
- **continue-on-error**: 한 도구가 실패해도 나머지는 계속 진행
- **전·후 버전 표시**: 각 도구마다 현재→결과 버전 출력
- **업데이트 후 실제 동작 재검증**: 명령 존재 여부가 아니라 `--version`이 실제로 도는지까지 확인해 깨진 심링크/미동작을 잡아냄 (원본의 `works()` 함수)
- **요약 표 + exit code**: 마지막에 표로 정리, 하나라도 실패 시 `exit 1` (자동화 친화)
- **Codex "깨졌으면 reinstall"** 같은 복구 로직

---

## 2. 확정 결정 로그 (Decision Log)

설계 단계에서 확정한 사항.

| # | 결정 | 선택 | 이유 |
|---|------|------|------|
| D1 | 구현 언어 | **Rust** 단일 바이너리 | 사용자 선택. 크로스플랫폼·단일 실행파일·Homebrew 친화. (npm 배포는 플랫폼별 prebuilt wrapper 패턴으로 해결) |
| D2 | 동작 모델 | **자동감지 + 설치 동의** | 실행 시 설치된 건 업데이트, 미설치 알려진 도구는 `(y/N)` 프롬프트. `--yes`로 전체 자동, `--no-install`로 업데이트만, config로 비대화 고정. 원본의 "그냥 실행" 감성 유지 + 설치 기능 추가 |
| D3 | 지원 OS | **macOS · Windows · Linux** | Rust라 Linux 추가 비용이 작고, 오픈소스 사용자 기대치 충족 |
| D4 | 이름 | **sync-ai-clis** | 동사형(원본 `update-ai-clis`와 결 같음). 동작 모델("감지→없으면 설치·있으면 업데이트→원하는 상태로 맞춤")이 곧 **sync(reconcile)**. npm·crates·GitHub 3곳 모두 충돌 없음 확인 |
| D5 | GitHub 오너 | 개인 **hyeonbungi** | 개인 OSS 도구 |
| D6 | 배포 도구 | **`dist` (구 cargo-dist)** 권장 | GitHub Releases + Homebrew tap + npm wrapper + shell/PowerShell 설치기를 한 번에 생성·유지. 요구사항에 정확히 부합 |
| D7 | 라이선스 | **MIT** | 사용자 지정 |
| D8 | Windows 배포 채널 | **winget + Scoop** 추가 | winget=MS 공식·기본 탑재(homebrew-core 격), Scoop=버킷 구조(Homebrew tap 격). Windows 사용자가 npm 없이도 네이티브하게 설치. winget 우선, Scoop fast-follow |

### 아키텍처 방향 (권장, 미확정 — 구현 시 확정)
- 도구 정의는 **선언형 레지스트리(ToolSpec 데이터)** 권장. 도구 추가 = 데이터 한 항목 → 오픈소스 기여 장벽↓. (대안: 트레잇-per-tool. 유연하지만 보일러플레이트↑)

---

## 3. 목표 / 비목표

### 목표 (v1)
- 5개 AI CLI를 3개 OS에서 **감지·설치·업데이트·동작검증**
- 미설치 도구 **동의 후 설치** (대화형 + `--yes`/`--no-install`/config)
- **dry-run**: 실행할 명령만 출력(신뢰·디버깅·테스트)
- 사람이 읽는 요약 + `--json`(자동화)
- MIT 오픈소스 공개, Homebrew·npm·crates.io 배포

### 비목표 (v1에서 안 함)
- AI CLI들의 **실행/오케스트레이션** (이 도구는 설치·업데이트만; 실행은 각 CLI가)
- 계정/인증/쿼터 관리 (cockpit-tools 류가 하는 일)
- GUI / TUI (CLI만)
- 텔레메트리 (수집 안 함 — 프라이버시)
- 자동 백그라운드 데몬 (수동 실행 또는 사용자가 cron 거는 정도)

---

## 4. 대상 도구 × OS 매트릭스 (리서치 결과)

5개 도구 모두 3개 OS에서 설치·업데이트 가능함을 웹 리서치로 확인(2026-06 기준, 출처는 **9. 참고자료**).

| 도구 | bin | mac | win | linux | 공식 설치기 (미설치 시) | self-update | 패키지매니저 |
|------|-----|:---:|:---:|:---:|------|------|------|
| Claude Code | `claude` | ✅ | ✅ | ✅ | `install.sh` / `install.ps1` | `claude update` (백그라운드 자동) | brew `claude-code` · winget `Anthropic.ClaudeCode` · npm `@anthropic-ai/claude-code` |
| Codex (OpenAI) | `codex` | ✅ | ✅ | ✅ | `chatgpt.com/codex/install.sh`/`.ps1` | `codex update` | npm `@openai/codex` · brew cask `codex`(mac만) |
| Gemini (Google) | `gemini` | ✅ | ✅ | ✅ | npm `@google/gemini-cli` | (없음 — npm/brew 재설치) | brew `gemini-cli`(mac/linux) · npm |
| Kiro (AWS) | `kiro-cli` | ✅ | ✅ | ✅ | `cli.kiro.dev/install` / `install.ps1` (Win은 **Win11**) | `kiro-cli update --non-interactive` (+백그라운드 자동) | **❌ 없음** |
| Antigravity (Google) | `agy` | ✅ | ✅ | ✅ | `antigravity.google/cli/install.sh`/`.ps1` | `agy update` | **❌ 없음** |

### ⭐ 핵심 설계 인사이트
**Homebrew/npm은 보편적이지 않다.** Kiro·Antigravity는 둘 다 없고, Codex cask는 mac 전용이다. 반면 **각 도구의 공식 네이티브 설치기 + self-update는 3 OS를 전부 커버**한다.

→ 따라서 크로스플랫폼 **기본 경로는 "공식 설치기 / self-update"** 로 잡고, Homebrew/npm은 **"이미 그걸로 깔린 경우 그 경로로 업데이트(중복 설치 방지)"** 하는 보조로 둔다. 이것이 원본 스크립트(Codex/Gemini를 brew에 의존)와의 가장 큰 차이다.

---

## 5. 아키텍처

### 5.1 구성 요소
```
┌─────────────┐   ┌──────────────────┐   ┌─────────────┐
│  CLI (clap) │──▶│  Engine          │──▶│ CommandRunner│
│  flags/args │   │  detect→plan→    │   │  (trait)     │
└─────────────┘   │  consent→run→    │   ├─────────────┤
                  │  verify→report   │   │ RealRunner   │ 실제 실행
┌─────────────┐   └──────────────────┘   │ MockRunner   │ 테스트 기록
│ Config(TOML)│──────────▲               │ DryRunRunner │ 출력만
└─────────────┘          │               └─────────────┘
┌─────────────────────────┴──┐  ┌────────────────────────┐
│ ToolSpec Registry (5종)     │  │ OsInfo / InstallSource │
│ 선언형 데이터 + 예외 훅       │  │ 감지                    │
└────────────────────────────┘  └────────────────────────┘
```

- **CommandRunner 추상화 (trait)**: 엔진은 명령을 *문자열로 구성*만 하고 실행은 Runner에 위임. 실제(Real)/테스트(Mock)/dry-run(DryRun) 3종 구현. 덕분에 시스템을 안 건드리고 "OS·설치상태별로 올바른 명령을 고르는지" 단위 테스트 가능 + **`--dry-run`이 공짜로** 따라온다.
- **OS 감지**: `std::env::consts::OS`(macos/windows/linux) + arch. Kiro용 **Windows 버전 판별(Win11 여부)**, Linux **glibc 버전**(Kiro musl 변형 선택) 추가.
- **설치 출처 감지(InstallSource)**: 바이너리 경로/매니저 조회(`brew list`·`npm ls -g`·경로 prefix)로 brew·npm·native·**winget·scoop** 중 무엇으로 깔렸는지 판별 → 같은 경로로 업데이트. (Windows: winget→`winget upgrade <id>` / scoop→`scoop update <pkg>` 감지)
- **graceful unsupported**: 지원 안 되는 조합(예: Win10의 Kiro)은 명확한 사유와 함께 SKIP.

### 5.2 ToolSpec 데이터 모델 (스케치 — 구현 시 확정)
```rust
struct ToolSpec {
    id: &'static str,            // "claude"
    display: &'static str,       // "Claude Code"
    bin: &'static str,           // "claude"  (PATH 감지 + --version)
    version_args: &'static [&'static str], // ["--version"]
    install_dir: fn(&OsInfo) -> Option<PathBuf>, // PATH 미반영 대비 절대경로 재확인 (예: ~/.local/bin)
    self_updates: bool,          // 백그라운드 자동 업데이트 여부 (Claude/Kiro)
    // OS별 설치 계획 (선호 순서대로). 미지원 OS는 None/Unsupported
    install:  fn(&OsInfo) -> Support<InstallPlan>,
    // OS + 설치출처별 업데이트 계획
    update:   fn(&OsInfo, InstallSource) -> Support<Vec<Command>>,
    // 예외 훅: Codex "깨졌으면 reinstall" 등 (대부분 None)
    on_broken: Option<fn(&OsInfo) -> Vec<Command>>,
}

enum Support<T> { Supported(T), Unsupported(&'static str /* 사유 */) }
```
> 선언형이 핵심이지만, `install`/`update`를 순수 데이터 테이블이 아니라 **함수 포인터**로 둬서 OS 분기·설치출처 분기 같은 조건 로직을 자연스럽게 표현한다. 새 도구 추가는 여전히 "한 모듈에 ToolSpec 하나 추가 + 레지스트리에 등록"으로 끝난다.

### 5.3 엔진 파이프라인 (도구 1개당)
1. **detect**: `bin`이 PATH에 있나? → 있으면 `current_version` 캡처
2. **plan**:
   - 설치됨 → `update(os, source)` 계획 (단, `--no-install`이어도 업데이트는 함)
   - 미설치 → `install(os)` 계획. 단 대화형이면 `(y/N)` 동의, `--yes`면 자동, `--no-install`이면 SKIP
   - Unsupported → SKIP(사유 출력)
3. **run**: CommandRunner로 실행 (continue-on-error)
4. **verify**: 다시 `--version`이 실제로 도는지 확인. 깨졌고 `on_broken` 있으면 복구 시도
5. **record**: 전·후 버전 + 결과(OK/FAIL/SKIP)

전체 끝나면 요약 표 + exit code(전부 OK면 0, 하나라도 FAIL이면 1).

### 5.4 모듈 레이아웃 (제안)
```
src/
  main.rs        # 진입점
  cli.rs         # clap 정의, 플래그 파싱
  config.rs      # ~/.config/sync-ai-clis/config.toml 로드
  os.rs          # OsInfo 감지 (os/arch/win버전/glibc)
  runner.rs      # CommandRunner trait + Real/Mock/DryRun
  source.rs      # InstallSource 감지
  engine.rs      # detect→plan→consent→run→verify→report
  report.rs      # 사람용 요약 + --json
  tools/
    mod.rs       # registry() -> Vec<ToolSpec>
    claude.rs
    codex.rs
    gemini.rs
    kiro.rs
    antigravity.rs
tests/
  command_selection.rs  # OS×설치상태별 올바른 명령 검증 (MockRunner)
```

### 5.5 견고성 · 보안 (Robustness & Security)

원본 bash가 (암묵적으로) 처리하던 실패 모드 + OSS로서의 신뢰 모델. 엔진이 반드시 다룰 것:

- **설치 직후 PATH 미반영**: 첫 설치 시 새 바이너리가 현재 프로세스 PATH에 아직 없는 위치(`~/.local/bin`·`%LOCALAPPDATA%\agy\bin` 등)에 깔려, 직후 verify가 **설치 성공에도 실패**할 수 있음. → 각 ToolSpec에 **알려진 설치 경로(`install_dir`)** 를 두고 PATH 조회 실패 시 그 절대경로로 재확인. 그래도 없으면 "셸 재시작 후 재실행" 안내(FAIL 처리 아님). (원본 `hash -r`의 Rust판)
- **전제조건(prerequisite) 감지**: npm 경로는 Node/npm, brew 경로는 brew 필요. 부재 시 암호 같은 실패 대신 `Unsupported("Node.js 필요")` 식 **명확한 SKIP**. (원본 `have brew` 가드 계승)
- **보안/신뢰 모델**: 이 도구는 원격 설치 스크립트를 실행(`curl|bash`·`irm|iex`)한다. ① 설치/업데이트 URL은 **레지스트리에 하드코딩된 공식 HTTPS 상수만**(사용자·config가 임의 URL 주입 불가), ② 실행 전 동의(또는 `--yes`), ③ `--dry-run`이 **실제 실행될 명령을 그대로** 노출, ④ 미검증 소스 실행 거부. README에 신뢰 모델 명시(rustup·Homebrew 관례).
- **Windows PowerShell 호출**: `irm…|iex` 류는 `powershell -NoProfile -ExecutionPolicy Bypass -Command "…"`로 감싸 실행(Windows PowerShell 5 vs `pwsh` 차이 고려).
- **권한 상승(sudo/UAC)**: 유저공간 설치기 우선이라 대체로 불필요. **자동 sudo/관리자 승격은 절대 하지 않음** — 필요하면 그 사실을 드러내고 사용자가 직접 실행하도록 안내.

---

## 6. CLI UX

### 6.1 명령 / 플래그
```
sync-ai-clis                 # 기본: 감지 → 설치된 건 업데이트, 미설치는 "설치할까요?(y/N)"
sync-ai-clis --yes, -y       # 비대화: 미설치도 설치 + 전부 업데이트 (CI용)
sync-ai-clis --no-install    # 업데이트만, 설치 권유 안 함
sync-ai-clis --only claude,gemini      # 일부만
sync-ai-clis --except kiro              # 일부 제외
sync-ai-clis --dry-run       # 실행할 명령만 출력, 아무것도 실행 안 함
sync-ai-clis list            # 알려진 도구 + 설치 여부 + 현재/최신 버전 표  (별칭: status)
sync-ai-clis doctor          # 진단: 깨진 설치 + 중복 설치(여러 채널) + PATH 미반영 탐지, 변경 없음
sync-ai-clis check           # 업데이트 가용성만 확인(읽기 전용), 종료코드로 신호 — CI·cron·프롬프트 배지용
sync-ai-clis audit           # 원격 설치 스크립트 변경 감지(읽기 전용), --accept로 신뢰 기준 갱신
sync-ai-clis --json          # 요약을 JSON으로 (자동화 연동)
sync-ai-clis --version / --help
```
`--only`, `--except`, `--json`은 전역 플래그라 `sync-ai-clis doctor --only gemini --json`처럼 서브커맨드 뒤에서도 사용할 수 있다.

### 6.2 config 파일 — `~/.config/sync-ai-clis/config.toml`
```toml
# 관리할 도구 (기본: 알려진 전체)
tools = ["claude", "codex", "gemini", "kiro", "antigravity"]

# 미설치 도구 처리: prompt | always | never
install_missing = "prompt"

# (선택) 도구별 선호 채널 오버라이드
[channels]
gemini = "brew"   # brew | npm
codex  = "npm"
```
- Windows config 경로는 `%APPDATA%\sync-ai-clis\config.toml` (또는 `dirs` 크레이트의 config_dir).
- 플래그가 config보다 우선.
- `[channels]`는 설치된 도구의 **업데이트 계획**에만 적용한다. 미설치 도구의 설치 경로와 `doctor`의 현실 진단에는 적용하지 않는다. 알 수 없는 도구 id나 채널명은 config 오류(exit 2)로 처리한다.

### 6.3 출력 / exit code
- 사람용: 도구별 `현재→결과` + 색상 OK/FAIL/SKIP + 마지막 요약 표 (원본 스타일 계승)
- dry-run에서 결과 버전 자리는 `(dry-run)`으로 표기 (아무것도 실행하지 않았으므로 "미정"이지 "없음"이 아님 — v0.1.2, 첫 실사용 피드백 반영)
- 업데이트 후 전·후 버전이 동일하면 `already current` 표기 (업데이트 명령은 대개 멱등)
- `--json`: `[{id, display, installed, before, after, action, result, reason}]` — 표기 분기 없이 원시 값 유지 (dry-run이면 `after: null`)
- exit: `0` 전부 정상 · `1` 하나라도 실패 · `2` 사용법 오류
- `doctor`(v0.2.0): 읽기 전용 진단 — 같은 도구가 여러 채널(brew·npm·…)에 깔려 PATH 순서로 가려지는 **중복 설치**, `--version`이 죽는 **깨진 설치**, 설치돼 있지만 **PATH에 없는** 경우를 보고. 문제 발견 시 exit `1`, 깨끗하면 `0`(미설치는 문제 아님 — sync의 일). `--json` 지원: `[{id, display, status, locations: [{path, source, version}], advice}]`
- `check`(v0.3.0): 읽기 전용 업데이트 가용성 점검 — 설치 버전과 채널별 latest(claude·codex·gemini는 npm 레지스트리 `npm view`, agy는 공식 매니페스트의 `version`)를 비교해 도구별 `current`/`update-available`/`unknown`/`not-installed`/`self-updating`을 보고. 변경 없음(설치/업데이트 실행 안 함). exit: `10` 하나라도 업데이트 있음 · `1` 결론 불가(프로브 실패) · `0` 전부 최신(미설치·self-updating은 중립). `--json`: `[{id, display, installed, current, latest, status, note}]`. kiro는 백그라운드 자동 업데이트라 `self-updating`으로 보고(설계 doc 0012). config `[channels]`는 check에 영향 없음(최신 릴리스 조회는 설치 채널과 무관).
- `audit`(v0.4.0): 읽기 전용 설치 스크립트 변경 감지 — claude·codex·kiro·agy의 원격 설치 스크립트(`curl|bash`/`irm|iex`)를 fetch해 마지막으로 신뢰한 베이스라인과 비교하고, 변경 시 unified diff(`similar`)를 보여준다. `data_dir`에 스크립트 전문을 저장하며, `audit`(플래그 없음)은 절대 쓰지 않는다 — 모든 쓰기는 `audit --accept`(현재 스크립트를 새 베이스라인으로 확정)로만. 베이스라인이 없는 첫 실행은 `unregistered`로 보고. exit: `10` 하나라도 변경 · `1` fetch 실패(결론 불가) · `0` 변경 없음(미등록·미해당은 중립). `--json`: `[{id, display, status, url, diff}]`. gemini는 npm 경유라 `not-applicable`. §5.5 신뢰 모델의 네 번째 기둥(설계 doc 0013).

---

## 7. 도구별 상세 명세 (구현 레퍼런스)

> 명령은 2026-06 리서치 기준.

### 7.1 Claude Code (`claude`)
- **install** mac/linux: `curl -fsSL https://claude.ai/install.sh | bash` · win(PowerShell): `irm https://claude.ai/install.ps1 | iex`
- **update** 기본: `claude update` (네이티브 self-update, 3 OS 동작). 설치출처별: brew→`brew upgrade claude-code` · npm→`npm i -g @anthropic-ai/claude-code@latest` · winget→`winget upgrade Anthropic.ClaudeCode` · scoop→`scoop update <pkg>`(존재 시)
- 비고: 네이티브는 백그라운드 자동 업데이트. `self_updates = true`

### 7.2 Codex (`codex`, OpenAI)
- **install** mac/linux: `curl -fsSL https://chatgpt.com/codex/install.sh | sh` · win: `powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"` · npm: `npm i -g @openai/codex`(Node 22+) · brew cask(mac): `brew install --cask codex`
- **update** 설치출처별: brew cask→`brew upgrade --cask codex` · npm→`npm i -g @openai/codex@latest` · native→`codex update` (공식 self-update 서브커맨드, TD-003)
- **설치 위치**: standalone 기본값은 mac/linux `~/.local/bin`(`CODEX_INSTALL_DIR`로 override 가능), Windows `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin`
- **on_broken** (원본 계승): 동작 안 하면 reinstall — mac brew면 `brew reinstall --cask codex`, 그 외 네이티브 설치기 재실행
- 주의: 비스코프 `codex` npm 패키지는 무관한 2012년 프로젝트 → 반드시 `@openai/codex`

### 7.3 Gemini (`gemini`, Google)
- **install** npm: `npm i -g @google/gemini-cli`(Node 20+, 보편) · brew(mac/linux): `brew install gemini-cli`
- **update** 설치출처별: brew→`brew upgrade gemini-cli` · npm→`npm i -g @google/gemini-cli@latest`
- 비고: 네이티브 self-update 없음. npm/brew 재설치로 갱신. 기본 채널은 npm(보편), brew로 깔렸으면 brew. 고정 `install_dir` 없음(npm/brew prefix 의존).

### 7.4 Kiro (`kiro-cli`, AWS — 구 Amazon Q CLI)
- **install** mac/linux: `curl -fsSL https://cli.kiro.dev/install | bash` · win(**Win11**, PowerShell/Windows Terminal): `irm https://cli.kiro.dev/install.ps1 | iex` · linux: glibc<2.34면 musl 변형
- **update**: `kiro-cli update --non-interactive` (+ 백그라운드 자동, 종료 시 적용). `self_updates = true`
- **설치 위치**: macOS `/Applications/Kiro CLI.app/Contents/MacOS`, linux `~/.local/bin`, Windows `C:\Program Files\Kiro-Cli`
- **Homebrew/npm 없음**
- **제약**: Windows는 **Win11 필수** → Win10이면 `Unsupported("Kiro는 Windows 11 필요")`로 SKIP

### 7.5 Antigravity (`agy`, Google)
- **install** mac/linux: `curl -fsSL https://antigravity.google/cli/install.sh | bash` · win(PowerShell, WSL 불필요): `irm https://antigravity.google/cli/install.ps1 | iex`
- **update**: `agy update` (네이티브 self-update)
- **Homebrew/npm 없음**. 설치 위치: mac/linux `~/.local/bin/agy`, win `%LOCALAPPDATA%\agy\bin`

---

## 8. 테스트 전략 (3 레이어 + Docker)

테스트는 "로직 검증"과 "실제 설치 검증"을 분리한다. 핵심 질문(여러 OS에서 실제로 설치·업데이트되는가)은 **레이어 2·3**가 답한다.

### 8.0 플랫폼 진리표 — Docker로 무엇이 되나
| OS | Docker | 실제 통합 경로 |
|----|:---:|------|
| **Linux** (배포판·arch·glibc/musl) | ✅ 로컬+CI | Docker 매트릭스 (8.2) |
| **macOS** | ❌ macOS 컨테이너 런타임 없음 | `macos-latest` CI (§8.5: 개발 머신 제외) |
| **Windows** | ❌ Mac 호스트서 win 컨테이너 불가 + winget 컨테이너 부재 | `windows-latest` CI / 로컬 VM |

→ Docker는 **Linux 다양성(가장 지저분한 부분)을 거의 공짜로** 커버. mac/win은 CI 러너가 정답. 로직은 레이어 1이 OS 없이 다 잡음.

### 8.1 레이어 1 — 단위 테스트 (어느 호스트에서나, 네트워크 없음)
`MockRunner`로 시스템을 안 건드리고 "주어진 OsInfo·InstallSource·설치상태에서 엔진이 *올바른 명령 문자열*을 고르는지" 검증. OS는 `OsInfo`를 주입 → mac/win/linux 분기를 실제 그 OS가 아니어도 전부 테스트. 가장 싸고 빠르며 로직 버그 대부분을 잡음.
  - 예: `(os=windows10, tool=kiro)` → SKIP("Win11 필요")
  - 예: `(os=macos, tool=gemini, source=brew)` → `brew upgrade gemini-cli`
  - 예: `(os=linux, tool=codex, 미설치, --yes)` → 네이티브 설치기 명령

### 8.2 레이어 2 — Linux 통합: **Docker 매트릭스** ✅
깨끗한 배포판 이미지에서 **실제 설치·업데이트**를 돌려 검증. 개발기(mac) Docker Desktop에서 로컬로도, CI에서도 동일.
  - 이미지: `ubuntu:22.04`·`ubuntu:24.04`·`debian:12`·`fedora`·`archlinux`·`alpine`(musl)·`rockylinux`
  - 아키텍처: amd64 + **arm64**(`docker buildx`/QEMU)
  - 커버: 네이티브 설치기(`curl|bash`)·npm 경로·**Kiro AppImage/musl 분기**·**glibc 버전 감지**(≥2.34 vs musl) — Docker로 glibc 다른 이미지를 골라 그 분기를 정확히 테스트. 도구 자신의 **정적 musl 빌드**도 alpine에서 검증
  - 부작용 격리: 컨테이너 일회용이라 실제 설치를 마음껏 돌려도 호스트 오염 없음
  - 패턴(개념): `docker run --rm -v "$BIN:/usr/local/bin/sync-ai-clis:ro" ubuntu:24.04 bash -c "sync-ai-clis --yes --only gemini && gemini --version"`

### 8.3 레이어 3 — macOS·Windows 통합: **CI 러너** (Docker 불가 영역)
실제 OS가 필요 → GitHub Actions 러너.
  - `macos-latest`(Apple Silicon `macos-14` 포함): brew·네이티브 경로
  - `windows-latest`(실제 Windows VM): **winget·Scoop·네이티브 PowerShell 설치기** — winget은 컨테이너에 없고 데스크톱 Windows가 필요하므로 이 러너가 유일한 현실적 경로
  - 로컬 디버깅용 Windows VM(UTM/Parallels, Apple Silicon) 선택 가능

### 8.4 결정성 — 가짜 도구 픽스처 + 실제 스모크 분리
실제 외부 설치기는 네트워크·계정·속도·flaky 이슈가 있다. 그래서:
  - **가짜 CLI 픽스처**: 더미 도구(`footool` + 스텁 설치기/업데이터)로 엔진 파이프라인(detect→install→update→verify→on_broken)을 **오프라인·결정적**으로 검증
  - **실제 CLI 스모크**: 5종 실제 설치 후 `--version` 동작만 확인(전체 기능 아님). 네트워크 허용, 별도 잡(예: nightly), flaky 허용

### 8.5 실행 격리 정책
- **실제 설치/업데이트를 실행하는 테스트는 격리 환경에서만 한다**: 로컬은 Docker 컨테이너(8.2), CI는 GitHub Actions 러너(8.3).
- **개발 머신(사용자 컴퓨터)을 테스트베드로 쓰지 않는다** — 개발 머신에서 허용되는 것: `--dry-run`, MockRunner 단위 테스트, 가짜 도구 픽스처, 빌드. 실제 설치/업데이트 실행은 금지.
- macOS 전용 실경로(brew cask 등)는 Docker가 불가하므로 실검증을 Phase 2 CI(`macos-latest`)로 이연한다.

---

## 9. 배포 & 릴리스 (요구사항 3)

### 9.1 채널
- **GitHub Releases**: CI 매트릭스 크로스컴파일 바이너리 첨부
- **Homebrew**: 개인 tap `hyeonbungi/homebrew-tap` (formula `sync-ai-clis`). homebrew-core 입성은 성숙도 요건 있으니 **tap부터**. 설치: `brew install hyeonbungi/tap/sync-ai-clis`
- **npm**: wrapper 패키지 `sync-ai-clis` + 플랫폼별 prebuilt를 `optionalDependencies`로 (esbuild/biome 패턴). `npm i -g sync-ai-clis`
- **crates.io**: `cargo install sync-ai-clis` (Rust라 거의 공짜 보너스)
- **winget** (Windows 공식, 기본 탑재): `winget install hyeonbungi.sync-ai-clis`. dist가 만든 MSI/EXE 기반, **WinGet Releaser** GitHub Action(내부적으로 Komac)으로 릴리스 시 winget-pkgs에 자동 제출 (classic PAT `public_repo` 필요)
- **Scoop** (Windows 개발자용, 버킷=탭): 개인 버킷 `hyeonbungi/scoop-bucket`. 매니페스트 `checkver`/`autoupdate`로 GitHub Releases 자동 추적. `scoop bucket add hyeonbungi https://github.com/hyeonbungi/scoop-bucket; scoop install sync-ai-clis`
- **ghcr.io 컨테이너 이미지** (2026-06-11 추가): `ghcr.io/hyeonbungi/sync-ai-clis` — scratch 위 정적 musl 바이너리(amd64+arm64). devcontainer/CI 이미지에 `COPY --from`으로 베이킹하는 프로비저닝 용도(런타임 아님 — scratch엔 셸/CA 없음). 릴리스마다 GITHUB_TOKEN만으로 자동 발행

### 9.2 권장: `dist` (구 cargo-dist)
GitHub Releases + Homebrew tap formula + npm wrapper + shell/PowerShell 설치기 + **MSI**를 **한 번에 생성·유지**해 준다. 단 **Scoop·winget 매니페스트는 dist가 자동 생성하지 않는다**(Scoop은 cargo-dist 이슈 #521로 미지원). 따라서:
- **winget**: 릴리스 워크플로에 **WinGet Releaser** 액션(Komac) 추가 — dist가 만든 MSI/EXE를 참조해 자동 제출
- **Scoop**: `hyeonbungi/scoop-bucket` 레포에 매니페스트(JSON) 1개 유지 + `autoupdate`/`checkver` → 이후 릴리스는 거의 무인 추적
- **crates.io** publish는 별도(`cargo publish`)

### 9.3 CI 빌드 타깃 (제안)
- macOS: `aarch64-apple-darwin`, `x86_64-apple-darwin`
- Windows: `x86_64-pc-windows-msvc` (필요 시 `aarch64`)
- Linux: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` (musl 변형 선택)

### 9.4 저장소 파일
`README.md`(영문, 사용법·설치) · `LICENSE`(MIT) · `CONTRIBUTING.md`(**레지스트리에 새 CLI 추가하는 법** 포함) · `CHANGELOG.md` · `.github/workflows/`(CI/release, **WinGet Releaser** 포함) · `docker/`(배포판별 Dockerfile + Linux 통합 테스트) · `Cargo.toml`(저작자·라이선스 메타).

**동반 레포(배포용)**: `hyeonbungi/homebrew-tap`(brew formula) · `hyeonbungi/scoop-bucket`(scoop 매니페스트). winget은 별도 레포 불필요(MS의 `winget-pkgs`에 PR 자동 제출).


### 9.5 버전 정책 (2026-06-11 명문화)

- **SemVer `MAJOR.MINOR.PATCH`**, 현재 0.x 단계 (Cargo/crates.io가 형식 강제).
- **0.x 단계의 의미**: **patch** = 버그·패키징 수정, 사용자 가시 동작 불변 (예: v0.1.1 — xz 없는 환경에서 npm 래퍼 실패 수정). **minor** = 기능 추가·동작 변경 — 0.x에서는 호환성 파괴도 minor에서 한다 (Cargo caret 규칙상 0.x는 minor가 사실상 major 역할).
- **1.0.0 선언 기준**: §6의 사용자 계약(플래그·exit code·`--json` 스키마·config 형식)을 더 이상 깨지 않겠다고 약속할 수 있을 때. 1.0 이후 계약 파괴는 major로만.
- **버전 번호 재사용 금지**: crates.io는 yank만 가능하고 같은 번호 재발행 불가, npm도 사실상 동일. 잘못 나간 릴리스는 번호를 올려서 고친다 (발행 *전* 태그 교체만 예외).
- **릴리스 단위**: `Cargo.toml` version과 `CHANGELOG.md`의 `## X.Y.Z - YYYY-MM-DD` 섹션을 함께 올린다 — dist가 이 섹션을 릴리스 본문 초안으로 쓴다. 여기에 **`release-notes/vX.Y.Z.md`** — 영어·한국어로 쓴 사용자 친화 릴리스 노트(기술 changelog와 역할 분리) + 그 릴리스를 가장 잘 보여주는 **터미널 SVG**(`release-notes/assets/`, README 데모와 같은 스타일, 실제 출력에 충실) — 를 같이 만들고, 발행 후 GitHub Release 본문을 이 노트로 갱신한다. 공개 레포에 `vX.Y.Z` 태그 푸시가 곧 릴리스 트리거다.
- **채널 자동화 현황 (v0.1.2부터 전자동)**: 태그 푸시 한 번으로 전 채널 발행 — GitHub Releases·brew formula(dist), crates.io(OIDC 신뢰 발행, **push:tags 트리거** — crates.io가 workflow_run을 보안상 거부), npm(OIDC 신뢰 발행, workflow_run 체인), Scoop 매니페스트(workflow_run 체인 자동 갱신), ghcr 컨테이너(workflow_run 체인). winget은 첫 버전 머지 후 자동(WinGet Releaser).

---


## 11. 열린 질문 / TODO (구현 중 해소)

- [x] **Codex 네이티브 업데이트**: 해소(2026-06-12, TD-003) — `codex update` 공식 self-update 서브커맨드 사용. standalone 설치기 재실행은 install/on_broken recovery 경로로 유지.
- [x] **Kiro Windows 정확 명령**: 해소(2026-06-12, TD-003) — Win11 PowerShell 설치 `irm https://cli.kiro.dev/install.ps1 | iex`, 업데이트 `kiro-cli update --non-interactive`.
- [ ] **짧은 별칭 커맨드**: `sync-ai-clis`가 길다면 `saic` 같은 짧은 별칭 바이너리도 제공할지
- [ ] **자기 자신 포함(self-include)**: 이 도구도 자기 레지스트리에 넣어 brew/npm/cargo로 self-update 할지 (nice-to-have)
- [ ] **공개 문서 언어**: README·CONTRIBUTING·코드 주석은 **영문 기본**(글로벌 OSS) 가정. 변경 원하면 조정
- [ ] **Linux 배포판 패키지매니저**: Claude Code의 서명된 apt/dnf/apk 저장소도 지원할지, 아니면 네이티브 설치기+npm만 (기본: 후자, 감지되면 그 경로)
- [x] **버전 비교**: "이미 최신" 표기 → 해소(v0.1.2): 업데이트 후 전·후 버전 동일이면 `already current` 표기 (§6.3)
- [ ] **Chocolatey**: winget/Scoop로 충분한지, choco도 추가할지(더 'apt식'·관리자 권한 → 후순위/선택)
- [ ] **winget 자동화 PAT**: WinGet Releaser는 classic PAT(`public_repo` scope) 필요 → 레포 시크릿으로 설정. fine-grained PAT 미지원
- [ ] **MSI 코드서명**: winget/MSI 배포 시 SmartScreen 경고 완화를 위한 서명 여부(초기엔 미서명으로 시작 가능)
- [ ] **실제 CLI 스모크 빈도/위치**: 5종 실제 설치 스모크를 어디서(nightly?·on-demand?) 돌릴지 + 일부 CLI 설치의 네트워크/리전 의존성
- [x] **도구별 `install_dir` 확정**: 해소(2026-06-12, TD-003) — `agy`·`claude`=`~/.local/bin` 계열, `codex` standalone=mac/linux `~/.local/bin`·Windows `%LOCALAPPDATA%\Programs\OpenAI\Codex\bin`, `kiro`=macOS app bundle `Contents/MacOS`·linux `~/.local/bin`·Windows `C:\Program Files\Kiro-Cli`. `gemini`는 npm/brew prefix 의존이라 고정 경로 없음.
- [x] **출처 분류: brew formula의 동봉 node_modules 오분류** → **해소(v0.2.1)**: `classify_path`가 Cellar/Caskroom을 node_modules보다 먼저 검사 (원 발견 기록: `gemini-cli`처럼 npm 패키지를 libexec에 동봉하는 brew formula는 심링크를 끝까지 풀면 `…/Cellar/<formula>/…/node_modules/…`가 되어 npm 마커가 brew 마커를 이긴다 → 업데이트가 `brew upgrade` 대신 `npm i -g`로 가고(중복 설치 유발 — 아래 prefix 항목과 결합 시 실제 발생), doctor의 출처 라벨도 틀린다. 수정 방향: `/cellar/`·`/caskroom/`을 `node_modules`보다 **먼저** 검사(formula/cask 경로는 결정적), 일반 homebrew prefix 검사는 node_modules 뒤에 둬서 brew-node 경유 npm 글로벌은 계속 npm으로)
- [x] **npm 채널의 prefix 고정** → **해소(v0.2.1)**: 엔진이 기존 바이너리의 소유 npm을 역추적(`owning_npm` — unix `<prefix>/lib/node_modules`↔`<prefix>/bin/npm`, win `<prefix>/node_modules`↔`npm.cmd`)해 절대경로로 실행. 미해석 시 종전대로 `npm` 폴백. dry-run은 재작성된 실제 명령을 표시(§5.5). (원 발견 2026-06-11: nvm 등 다중 node 환경에서 `npm i -g`가 다른 prefix에 새 사본을 만들어 sync가 중복을 유발 — doctor가 첫 실전에서 검출)

### v2+ 기능 후보 (2026-06-11 제안 — 채택 미정, 검토 중)

선정 기준은 둘: §5.5 신뢰 모델 강화(이 도구의 정체성), reconcile 실사용 페인포인트. v1 비목표(GUI·데몬·오케스트레이션·텔레메트리)는 유지 전제.

- [x] **`doctor` 서브커맨드** → **채택(2026-06-11)**: §6.1·§6.3에 명세, v0.2.0 구현
- [x] **`--check` 모드** → **채택·구현(v0.3.0)**: `check` 서브커맨드로 출하(설계 doc 0012). 읽기 전용 업데이트 가용성 점검, 종료코드 `10`(업데이트 있음)/`1`(결론 불가)/`0`(최신). 커버리지 A+agy — claude·codex·gemini는 npm 레지스트리, agy는 공식 매니페스트, kiro는 self-updating. §6.1·§6.3에 명세. 프롬프트 배지는 cron→파일 패턴으로 충족(캐시 비구축).
- [ ] **로컬 버전 이력 (+제한적 롤백)**: 실행마다 전·후 버전을 로컬 state(`history.jsonl`)에 기록 — "어제 뭐가 올라갔지?"에 즉답. 텔레메트리 아님(로컬 전용). 롤백은 버전 지정 설치가 가능한 채널(npm 등)만 (추천 3순위)
- [x] **설치 스크립트 변경 감지** → **채택·구현(2026-06-17, 설계 doc 0013, v0.4.0)**: `audit` 읽기전용 서브커맨드. 원격 설치 스크립트(claude·codex·kiro·agy)를 fetch해 마지막으로 신뢰한 내용과 비교, 변경 시 unified diff(`similar`). 베이스라인은 명시적 `audit --accept`로만 갱신(첫 실행=`unregistered`, 읽기전용 일관), `data_dir`에 전문 저장. 종료코드 `10`(변경)/`1`(fetch 실패)/`0`(변경 없음). §6.1·§6.3에 명세. §5.5 신뢰 모델의 네 번째 기둥.
- [ ] **멀티 머신 상태 비교**: 머신별 `--json` 스냅샷 파일을 모아 어느 머신이 뒤처졌는지 표로 비교 — 동기화 서버 없이 파일 기반만
- [ ] **레지스트리 확장 기준 명문화**: 새 AI CLI 추가 요건(공식 설치기 존재·HTTPS·라이선스)을 CONTRIBUTING에 명시해 커뮤니티 PR로 성장. 사용자 정의 ToolSpec(config 선언)은 §5.5 하드코딩 URL 원칙과 충돌 — 한다면 "업데이트 명령만 허용, 설치 URL 금지"로 제한 설계
- [ ] **폴리시 묶음**: 셸 자동완성(clap_complete)·man page·`--json` 스키마 버저닝(1.0 계약 준비)·devcontainer feature(ghcr 이미지의 연장)

---

## 12. 참고자료 (리서치 출처)

- Claude Code 설치/업데이트: https://code.claude.com/docs/en/setup
- Codex CLI: https://developers.openai.com/codex/cli · https://developers.openai.com/codex/cli/reference · https://www.npmjs.com/package/@openai/codex · https://github.com/openai/codex
- Gemini CLI: https://geminicli.com/docs/get-started/installation/ · https://www.npmjs.com/package/@google/gemini-cli · https://github.com/google-gemini/gemini-cli
- Kiro CLI: https://kiro.dev/docs/cli/installation/ · https://kiro.dev/docs/cli/reference/cli-commands/ · https://kiro.dev/blog/cli-2-0/ (Windows 지원)
- Antigravity CLI: https://antigravity.google/docs/cli-install · https://antigravity.google/docs/cli-getting-started
- 배포: `dist` (구 cargo-dist) — Rust CLI를 GitHub Releases/Homebrew/npm/shell installer/MSI로 배포
- Windows 배포: WinGet Releaser 액션 https://github.com/marketplace/actions/winget-releaser · Komac https://github.com/russellbanks/Komac · Scoop https://github.com/ScoopInstaller/scoop · cargo-dist Scoop 이슈 #521 https://github.com/axodotdev/cargo-dist/issues/521
- 테스트 환경: GitHub Actions 러너(`macos-latest`·`windows-latest`·`ubuntu-latest`) · Docker buildx/QEMU(멀티아치 Linux) · Alpine=musl. (macOS 컨테이너 런타임 없음 / Windows 컨테이너는 Windows 호스트 전용)

---

