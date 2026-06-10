# Docker Linux Integration Matrix

SPEC §8.2의 레이어 2 통합 테스트입니다. **실제 설치/업데이트 실행은 이 하니스(일회용 컨테이너) 안에서만 합니다** — 개발 머신은 절대 테스트베드로 쓰지 않습니다(SPEC §8.5). 호스트 포트는 바인딩하지 않습니다.

## 사용법

```bash
docker/run-matrix.sh                          # 스모크: ubuntu:24.04, claude+antigravity
MATRIX=full docker/run-matrix.sh              # glibc 배포판 6종 전체
TOOLS=claude,codex,kiro,antigravity docker/run-matrix.sh   # 도구 선택
RUN_MUSL=1 docker/run-matrix.sh               # musl 빌드 + alpine 레그 (실험적)
PLATFORM=linux/amd64 docker/run-matrix.sh     # QEMU로 교차 아키텍처
```

## 동작

1. **빌드도 컨테이너에서**: `rust:1-bookworm`에 저장소를 마운트해 release 빌드 (`target-linux/`, gitignored). cargo registry는 네임드 볼륨으로 캐시.
2. 배포판별 일회용 컨테이너에 바이너리를 **읽기 전용 마운트**하고 [container-test.sh](container-test.sh) 실행:
   전제조건 설치(curl·ca-certificates·bash) → `sync-ai-clis --yes --only $TOOLS` **실제 설치** → 도구별 `--version` 재검증 → `sync-ai-clis list`.
3. 레그별 OK/FAIL 요약, 하나라도 실패 시 exit 1.

## 주의

- 네트워크 필요(공식 설치기 다운로드). 설치기 쪽 장애로 flaky할 수 있음 — 실패 시 로그로 원인 구분.
- 기본 도구는 native 설치기 2종(claude·antigravity). gemini는 Node 전제가 커서 기본 제외(필요 시 TOOLS에 추가 + 이미지에 node 직접 설치).
- musl(alpine) 레그는 실험적: 우리 바이너리는 musl로 빌드되어 돌지만, 업스트림 설치기가 musl 빌드를 제공하지 않을 수 있음 (SPEC §7.4 Kiro만 musl 변형 명시).
