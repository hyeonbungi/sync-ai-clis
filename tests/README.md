# Tests Directory

cargo 통합 테스트(`cargo test`)를 두는 위치입니다. 테스트 전략의 원천은 [SPEC.md](../SPEC.md) §8입니다.

계획된 구성 (Phase 1, P1-008):

1. `command_selection.rs` — MockRunner로 OS×설치상태×설치출처 매트릭스에서 엔진이 올바른 명령 문자열을 고르는지 검증 (네트워크·시스템 변경 없음)
2. 가짜 도구 픽스처(`footool` + 스텁 설치기/업데이터) — detect→install→update→verify→on_broken 파이프라인을 오프라인·결정적으로 검증

Linux 실통합은 `docker/` 매트릭스(P1-009), macOS/Windows 실통합은 GitHub Actions 러너(P2-001)가 담당합니다.

Windows Integration의 JSON 검증 회귀 테스트는 `test_integration_results.py`입니다.
Python 3과 jq로 실제 워크플로의 `.github/scripts/windows-results.jq`를 실행합니다.
설치기 실행과 네트워크 접근은 없습니다.

```bash
python3 -m unittest discover -s tests -p 'test_integration_*.py' -v
```

빌드 22000 이상에서는 Kiro `ok`, 미만·빌드 감지 실패에서는 명시한 지원 제한 사유의
`skip`만 허용합니다. 누락·중복 도구, 잘못된 건너뛰기, 실패 결과도 검증합니다.
실제 Windows 설치 성공 시에는 별도 단계에서 registry PATH를 갱신한 뒤
`kiro-cli --version`을 실행합니다. 수동 실행의 `target=windows`는 Windows 작업만 실행합니다.

`cargo test`에는 다운로드 실패 시 부분 설치기를 실행하지 않는지, npm shim의
설치 출처와 소유 npm 판별, Codex npm 복구 채널 유지, 설치 거절 시 JSON 출력,
PATH 밖 Windows `.exe` 탐색을 검증하는 회귀 테스트가 포함됩니다.
Windows 전용 runner 테스트는 공백이 있는 경로의 `.cmd`를 실행하고 인수 전달을 확인합니다.
