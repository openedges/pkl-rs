# 로드맵

기준은 Pkl **0.32.0**입니다. 현재 완료 범위는 Git·Rust workspace·참조 submodule·개발 문서·초기 구성 검증입니다. Java oracle, Pkl 구현, 전체 호환성 코퍼스와 성능 기준선은 아직 구축되지 않았습니다.

| 단계 | 주요 산출물 | 완료 조건 |
| --- | --- | --- |
| M0 기준선 | 참조·oracle artifact 고정, API·stdlib·테스트 목록, Java 실행 도구, 성능 기준선 | 비교 환경 재현, 누락·실행 오류 검출, 기준 결과 확보 |
| M1 구조 검증 | parser·SourceMap·resolver, 진단, regex·숫자·heap·VM 실험 | 주요 의미론 위험 확인, 공통 엔진과 성능 전략 검증 |
| M2 언어 | 객체·lazy·상속·closure·타입·Reference | 해당 언어 코퍼스의 결과·오류 바이트 일치 |
| M3 stdlib·출력 | 전체 builtin, renderer, reflection, Pkl Binary | 공개 멤버 누락 0, 출력·오류 일치 |
| M4 외부 기능 | module/resource Host, HTTP, 프로젝트·패키지, external reader | 실제 I/O·패키지 코퍼스 통과 |
| M5 제품 API | CLI·REPL·server·Rust API·formatter·doc·codegen·JVM adapter | 기존 호출·client·수명주기 호환 |
| M6 Wasm·N-API | 비동기 Host, adapter, 배포 패키지 | 실제 브라우저·Node의 대상 코퍼스 통과 |
| M7 릴리스 | 전체 Pantry, 플랫폼 검증, 시간·메모리 보고서 | 알려진 필수 불일치 0, 성능·메모리 기준 충족 |

Wasm 빌드와 브라우저·Node의 작은 실행 테스트는 M1부터 구축합니다. M6는 이를 전체 기능으로 확대하는 단계입니다. 성능과 메모리 측정도 M1부터 병행합니다.

## 다음 작업: M0

1. Java 0.32.0 실행 artifact와 JDK·Graal/Truffle 설정, 해시를 고정합니다. 소스 commit 고정만으로 실행 기준선을 확보했다고 보지 않습니다.
2. 언어·stdlib 멤버·CLI 명령과 옵션·protocol·공개 API 목록을 만들고 구현·검증 상태를 추적합니다.
3. upstream·Pantry·rev1 회귀 입력을 조사해 출처와 실행 계약을 가진 테스트 manifest를 구성합니다. 발견한 파일 수와 실행한 테스트 수를 구분합니다.
4. 원시 stdout·stderr·종료 코드·생성 파일을 보관하고 비교하는 Java/Rust 차등 실행 도구를 구현합니다.
5. 테스트 도구가 누락된 기대 결과, 빈 코퍼스, 실행기 부재와 바이트 차이를 실패로 검출하는지 확인합니다.
6. CLI의 전체 실행과 상주 evaluator의 첫 요청·반복 요청을 나누어 시간과 메모리 기준선을 확보합니다.

현재 `cargo xtask verify`는 참조 고정 검증 도구입니다. 차등 실행과 성능 측정 명령은 구현 후 문서에 추가합니다.

## M1에서 결정할 사항

- Java의 문자열·정규식·숫자 경계 동작을 Native와 Wasm에서 재현하는 방법.
- 이름 해석과 lazy 객체·타입 검사·오류 순서의 표현.
- 순환 객체를 포함한 heap 수명주기와 장기 세션의 메모리 회수.
- bytecode VM의 성능 가능성, specialization·캐시 필요성.
- 비동기 Host 호출의 중단·재개, 취소와 evaluator 종료 계약.

일정과 병렬 작업 범위는 M0의 대상 목록과 M1 실험 결과를 확보한 뒤 산정합니다. 필수 기능을 제외하거나 성능 기준을 완화하는 변경은 목표 변경으로 명시합니다.
