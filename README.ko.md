# sync-ai-clis

<p align="center">
  <a href="https://github.com/hyeonbungi/sync-ai-clis/actions/workflows/ci.yml">
    <img alt="CI" src="https://img.shields.io/github/actions/workflow/status/hyeonbungi/sync-ai-clis/ci.yml?branch=main&style=flat-square&label=CI">
  </a>
  <a href="https://github.com/hyeonbungi/sync-ai-clis/releases/latest">
    <img alt="GitHub 릴리스" src="https://img.shields.io/github/v/release/hyeonbungi/sync-ai-clis?style=flat-square&label=release&color=2f80ed">
  </a>
  <img alt="플랫폼: macOS, Windows, Linux" src="https://img.shields.io/badge/platforms-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux-44cc11?style=flat-square">
  <a href="./LICENSE">
    <img alt="라이선스: MIT" src="https://img.shields.io/badge/license-MIT-111827?style=flat-square">
  </a>
</p>

<p align="center">
  <a href="https://crates.io/crates/sync-ai-clis">
    <img alt="crates.io" src="https://img.shields.io/crates/v/sync-ai-clis?style=flat-square&logo=rust&logoColor=white&label=crates.io&color=2f80ed">
  </a>
  <a href="https://www.npmjs.com/package/sync-ai-clis">
    <img alt="npm" src="https://img.shields.io/npm/v/sync-ai-clis?style=flat-square&label=npm&color=2f80ed">
  </a>
  <a href="https://github.com/hyeonbungi/scoop-bucket">
    <img alt="Scoop" src="https://img.shields.io/scoop/v/sync-ai-clis?bucket=https%3A%2F%2Fgithub.com%2Fhyeonbungi%2Fscoop-bucket&style=flat-square&label=scoop&color=2f80ed">
  </a>
  <a href="https://github.com/hyeonbungi/sync-ai-clis/pkgs/container/sync-ai-clis">
    <img alt="ghcr.io 컨테이너" src="https://img.shields.io/badge/ghcr.io-container-2f80ed?style=flat-square&logo=docker&logoColor=white">
  </a>
</p>

<p align="center">
  <a href="./README.md">English</a> | 한국어
</p>

> 여러 AI 코딩 CLI(Claude Code · Codex · Gemini · Kiro · Antigravity)를 한 명령으로 감지·설치·최신 유지.

<p align="center">
  <img alt="sync-ai-clis --dry-run 출력: 도구별 감지된 설치 채널과 실제 실행될 명령" src="https://raw.githubusercontent.com/hyeonbungi/sync-ai-clis/main/.github/assets/terminal-demo.svg" width="600">
</p>

`sync-ai-clis`는 머신을 "알려진 AI CLI가 전부 설치되어 있고, 동작하고, 최신인 상태"로 맞추는(reconcile) 크로스플랫폼(macOS · Windows · Linux) Rust CLI입니다. 설치된 도구는 업데이트하고, 미설치 도구는 동의를 받아 설치하며, 작업이 끝나면 각 도구를 재검증합니다(`command -v`가 아니라 `--version`이 실제로 도는지 확인해 깨진 설치를 잡아냅니다).

**현재 상태: 릴리스됨.** `list`·`doctor`·`--dry-run`·동의 기반 설치/업데이트가 모두 동작하며, 테스트 116개와 Linux 컨테이너·macOS·Windows CI의 실채널 검증을 통과했습니다. 확정 결정, 아키텍처, 도구별 매트릭스, 테스트·릴리스 전략 등 전체 설계는 단일 진실 원천인 [SPEC.md](./SPEC.md)에 있습니다.

## At A Glance

| 항목 | 현재 값 |
| --- | --- |
| 목적 | AI 코딩 CLI 감지 · 동의 후 설치 · 업데이트 · 동작 검증 |
| 관리 도구 (v1) | `claude`, `codex`, `gemini`, `kiro-cli`, `agy` |
| 플랫폼 | macOS · Windows · Linux |
| 스택 | Rust (단일 바이너리) |
| 상태 | 릴리스됨 — 3 OS 전부에서 엔진 검증 (오프라인 테스트 116개 + 실채널 CI) |
| 배포 | GitHub Releases · Homebrew tap · npm · crates.io · winget · Scoop · ghcr (Docker) |
| 테스트 | 오프라인 116개 + Docker 배포판 매트릭스 + 3 OS 실채널 CI |
| 라이선스 | [MIT](./LICENSE) |
| 저작자 | [hyeonbungi](https://github.com/hyeonbungi) |

## 설치

```sh
# Homebrew (macOS · Linux)
brew install hyeonbungi/tap/sync-ai-clis

# 쉘 설치기 (macOS · Linux)
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/hyeonbungi/sync-ai-clis/releases/latest/download/sync-ai-clis-installer.sh | sh

# npm
npm install -g sync-ai-clis

# cargo
cargo install sync-ai-clis
```

```powershell
# winget (Windows)
winget install hyeonbungi.sync-ai-clis

# Scoop (Windows)
scoop bucket add hyeonbungi https://github.com/hyeonbungi/scoop-bucket
scoop install sync-ai-clis

# PowerShell 설치기 (Windows)
powershell -ExecutionPolicy Bypass -c "irm https://github.com/hyeonbungi/sync-ai-clis/releases/latest/download/sync-ai-clis-installer.ps1 | iex"
```

```dockerfile
# Docker / devcontainer — 이미지에 AI CLI 베이킹 (정적 musl 바이너리, amd64+arm64)
COPY --from=ghcr.io/hyeonbungi/sync-ai-clis:latest /sync-ai-clis /usr/local/bin/sync-ai-clis
RUN sync-ai-clis --yes --only claude,gemini
```

## 사용법

[SPEC.md](./SPEC.md) §6에서 확정한 계약 그대로 구현했습니다:

```text
sync-ai-clis                 # 기본: 설치된 건 업데이트, 미설치는 "설치할까요?(y/N)"
sync-ai-clis --yes, -y       # 비대화: 미설치도 설치 + 전부 업데이트 (CI용)
sync-ai-clis --no-install    # 업데이트만, 설치 권유 안 함
sync-ai-clis --only claude,gemini
sync-ai-clis --except kiro
sync-ai-clis --dry-run       # 실행할 명령만 출력, 아무것도 실행 안 함
sync-ai-clis list            # 알려진 도구 + 설치/현재 버전 표 (별칭: status)
sync-ai-clis doctor          # 읽기 전용 진단: 깨진 설치, 중복 설치, PATH 미반영
sync-ai-clis --json          # 자동화용 JSON 요약
```

`--only`, `--except`, `--json`은 전역 플래그라 `sync-ai-clis doctor --only gemini --json`처럼 서브커맨드 뒤에서도 사용할 수 있습니다.

종료 코드: `0` 전부 정상 · `1` 하나라도 실패 · `2` 사용법 오류. 설정은 `~/.config/sync-ai-clis/config.toml`에 둡니다(플래그가 config보다 우선).

## 신뢰 모델

이 도구는 원격 공식 설치기(`curl | bash`, `irm | iex`)와 패키지 매니저 명령을 실행하므로 보안 규칙을 설계에 못박아 둡니다([SPEC.md](./SPEC.md) §5.5):

- 설치/업데이트 URL은 도구 레지스트리에 **하드코딩된 공식 HTTPS 상수만** 씁니다 — config나 플래그로 임의 URL을 주입할 수 없습니다.
- 미설치 도구를 설치하려면 **동의**가 필요합니다: 대화형 프롬프트 또는 명시적 `--yes`.
- `--dry-run`은 실제 실행될 명령을 **그대로** 출력합니다.
- **자동 권한 상승 없음** — 스스로 sudo/UAC 승격을 하지 않습니다.

## 저장소 지도

| 경로 | 역할 |
| --- | --- |
| `SPEC.md` | 설계 원천: 확정 결정, 아키텍처, 도구 매트릭스, 테스트 전략 |
| `Cargo.toml`, `src/` | Rust 크레이트 — 엔진·도구 레지스트리·CLI (`SPEC.md` §5 참고) |
| `tests/` | 통합 테스트 (OS × 상태 명령 선택 매트릭스, 바이너리 스모크) |
| `docker/` | Linux 통합 매트릭스 — 로컬에서 실제 설치가 허용되는 유일한 곳 |
| `.github/workflows/` | 3 OS CI, 실채널 통합, 릴리스 파이프라인, winget 발행 |
| `dist-workspace.toml` | 릴리스·패키징 설정 ([dist](https://github.com/axodotdev/cargo-dist)) |

## 개발

```bash
cargo test                 # 오프라인 테스트 116개 — 네트워크·시스템 변경 없음
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo run -- list          # 읽기 전용: 도구 감지·버전 표
cargo run -- --dry-run     # 실행될 명령만 그대로 출력, 실행 없음
docker/run-matrix.sh       # 실제 설치/업데이트 통합 (일회용 컨테이너에서만)
```

확정 결정·아키텍처·도구별 명령 매트릭스·테스트 전략 등 설계 근거는 [SPEC.md](./SPEC.md)에 있습니다. 실제 설치/업데이트는 개발 머신에서 절대 실행하지 않습니다 — Docker 매트릭스와 CI 러너가 그 역할을 맡습니다.

## 알려진 제약

- **Windows의 Kiro**: Windows 11이 필요하고, 공식 설치 명령이 업스트림에서 아직 확정되지 않아 URL을 추측하는 대신 명확한 SKIP으로 처리합니다 (이미 설치된 `kiro-cli`의 self-update는 정상 동작). [SPEC.md](./SPEC.md) §11에서 추적합니다.
- **Alpine/musl**: sync-ai-clis 바이너리 자체는 musl에서 동작하지만, 업스트림 설치기 대부분이 아직 musl 빌드를 제공하지 않습니다.
- **config `[channels]` 오버라이드**는 업데이트 계획에만 적용됩니다. 미설치 도구 설치와 `doctor` 진단은 실제 감지 상태를 그대로 봅니다.

## 운영 신호

- 기여 가이드: [CONTRIBUTING.md](./CONTRIBUTING.md)
- 보안 정책 + 신뢰 모델: [SECURITY.md](./SECURITY.md)
- 변경 기록: [CHANGELOG.md](./CHANGELOG.md)
- CI: 푸시마다 3 OS 테스트, 주간 실채널 통합

## 기원

이 도구는 5개 AI CLI를 업데이트하고 재검증하던 개인용 macOS 전용 bash 스크립트(`update-ai-clis`)에서 출발했습니다. v1은 이를 일반화합니다: 3개 OS, 동의 기반 미설치 도구 설치, 공개 배포 채널.

## 저작자

- [hyeonbungi](https://github.com/hyeonbungi) (김현우)

## 라이선스

MIT. [LICENSE](./LICENSE)를 참고하세요.
