# Tests Directory

cargo 통합 테스트(`cargo test`)를 두는 위치입니다. 테스트 전략의 원천은 [SPEC.md](../SPEC.md) §8입니다.

계획된 구성 (Phase 1, P1-008):

1. `command_selection.rs` — MockRunner로 OS×설치상태×설치출처 매트릭스에서 엔진이 올바른 명령 문자열을 고르는지 검증 (네트워크·시스템 변경 없음)
2. 가짜 도구 픽스처(`footool` + 스텁 설치기/업데이터) — detect→install→update→verify→on_broken 파이프라인을 오프라인·결정적으로 검증

Linux 실통합은 `docker/` 매트릭스(P1-009), macOS/Windows 실통합은 GitHub Actions 러너(P2-001)가 담당합니다.
