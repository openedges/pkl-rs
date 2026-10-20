# 로드맵

기준은 Pkl **0.32.0**입니다. 현재 완료 범위는 Git 설정, Rust workspace, 참조 submodule, 개발 문서, 초기 구성 검증입니다. Java oracle, Pkl 구현, 전체 호환성 코퍼스와 성능 기준선은 아직 구축되지 않았습니다.

이 문서는 단계별 개요입니다. 목표와 범위는 [INTENT.md](../INTENT.md), 상세 실행 순서·작업 ID·선행 조건·완료 증거는 [PLAN.md](../PLAN.md)에서 관리합니다.

| 단계 | 주요 산출물 | 완료 조건 |
| --- | --- | --- |
| M0 기준선 | 참조·oracle artifact 고정, API·stdlib·테스트 목록, Java 실행 도구, 성능 기준선 | 비교 환경 재현, 누락·실행 오류 검출, 기준 결과 확보 |
| M1 구조 검증 | parser·SourceMap·resolver, 진단, regex·숫자·heap·VM 실험 | 주요 의미론 위험 확인, 공통 엔진과 성능 전략 검증 |
| M2 언어 | 객체·lazy 평가·상속·closure·타입·Reference | 해당 언어 코퍼스의 결과·오류 바이트 일치 |
| M3 표준 라이브러리·출력 | 전체 builtin, renderer, reflection, Pkl Binary | 공개 멤버 누락 0, 출력·오류 일치 |
| M4 외부 기능 | module/resource Host, HTTP, 프로젝트·패키지, external reader | 실제 I/O·패키지 코퍼스 통과 |
| M5 제품 API | CLI·REPL·server·Rust API·formatter·문서 생성·코드 생성·JVM adapter | 기존 호출·client·수명주기 호환 |
| M6 Wasm·N-API | 비동기 Host, adapter, 배포 패키지 | 실제 브라우저·Node의 대상 코퍼스 통과 |
| M7 릴리스 | 전체 Pantry 코퍼스, 플랫폼 검증, 실행 시간·메모리 보고서 | 필수 범위의 알려진 불일치 0, 성능·메모리 기준 충족 |

Wasm 빌드와 브라우저·Node의 작은 실행 테스트는 M1부터 구축합니다. M6는 이를 전체 기능으로 확대하는 단계입니다. 성능과 메모리 측정도 M1부터 구현과 병행합니다.

## 실행 계획 관리

[PLAN.md](../PLAN.md)의 현재 상태와 다음 작업을 기준으로 착수합니다. M0의 oracle·목록·비교 도구·기준선 작업과 M1의 구조 검증 실험은 해당 문서에 산출물 및 종료 조건과 함께 정리합니다.

작업별 진행 상태와 증거는 PLAN.md에 기록하고, 큰 단계의 변경은 이 개요에도 반영합니다. 검증 방법은 [TESTING.md](../TESTING.md), 설계 결정은 [아키텍처](architecture.md)를 따릅니다. 일정은 M0의 규모 조사와 M1 실험 결과를 확보한 뒤 산정합니다.
