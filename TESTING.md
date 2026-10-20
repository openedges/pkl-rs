# 검증 계약

목표는 Pkl 0.32.0 Java 구현과의 동작·바이트 호환성, 그리고 필수 워크로드별 시간·메모리 동등 이상입니다. **현재 실행 가능한 검증은 초기 구성 검증뿐**이며, Java 차등 테스트·Pantry 실행·제품 성능 측정은 아직 구현되지 않았습니다.

## 현재 검증

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

첫 명령은 참조 저장소의 고정 상태를 확인합니다. 나머지는 Rust 형식·정적 검사·현재 workspace 테스트·Wasm 컴파일 검증입니다. Wasm 컴파일 성공은 브라우저에서의 평가나 API 호환성을 검증한 결과가 아닙니다. `cargo test`의 통과 개수를 Pkl 기능 완료율로 사용하지 않습니다.

## 호환성 비교

동일한 입력과 환경에서 고정된 Java 0.32.0 oracle과 Rust를 실행합니다. 비교 대상은 다음과 같습니다.

- stdout·stderr의 원시 바이트와 종료 코드.
- 생성 파일의 경로·집합·원시 바이트.
- 오류 메시지, 공백, 소스 위치, 밑줄, 호출 프레임, hint, 오류 발생 순서.
- CLI 옵션·기본값, REPL 상태, API의 결과·오류·취소·close 동작.
- server protocol의 encoding, callback, 세션 수명주기.

작업 경로, locale, 시간대, 터미널 설정, 환경변수, 네트워크 응답 등 결과에 영향을 주는 입력을 고정합니다. 경로 차이도 동일 mount 등의 실행 환경 구성으로 해결합니다. 정식 비교에서 `trim`, 정렬, JSON 재직렬화, 임의 문자열 치환으로 차이를 제거하지 않습니다.

Java의 무작위 evaluator ID처럼 실행마다 달라지는 protocol 필드는 별도 계약이 필요합니다. 고정 ID를 입력으로 주는 codec 테스트는 전체 바이트를 비교합니다. 실제 세션 테스트는 발급 ID의 참조 일관성과 결과 payload를 검증하고, 이를 세션 전체의 바이트 일치로 표현하지 않습니다.

기대 결과는 Java oracle에서 생성하고 출처와 변경 내용을 검토합니다. Rust의 현재 출력으로 기대 결과를 덮어써서 불일치를 해소하지 않습니다.

## 구축할 테스트 계층

| 계층 | 검증 범위 |
| --- | --- |
| 실행 도구 | 빈 코퍼스, 기대 결과 누락, 실행기 부재, 바이트 차이의 실패 검출 |
| parser·resolver | 문법, 잘못된 입력, source span, 이름 해석, formatter 왕복 |
| 런타임·타입 | lazy forcing, 상속, closure, 순환, 제약·generic, 첫 오류 |
| stdlib·renderer | 모든 공개 멤버, 경계값, 출력 순서, converter, binary |
| CLI·REPL·server | help·옵션, PTY 세션, 상태 유지, protocol, timeout·close |
| I/O·패키지 | HTTP fixture, redirect·proxy·CA, root 제한, checksum, cache, lockfile |
| Pantry | 고정 revision의 개발 트리·패키징된 모듈·실제 예제·생성 파일 |
| 플랫폼 | Native·Wasm·N-API의 결과·오류와 Host 기능 |
| 생성·변형 | 문법 생성, differential fuzzing, property·metamorphic 테스트 |
| 장기 실행 | 성공·실패 반복, 고유 소스 증가, 취소, evaluator 생성·종료와 메모리 회수 |

테스트 manifest에는 case ID, 원본 저장소·revision·경로, 입력·리소스 hash, 실행 인자·환경, 필요한 Host 기능, 예상 성공·오류, 검증 채널·파일, 실행 대상을 기록합니다. Pantry는 고정 commit만으로 0.32.0 호환성이 검증되지 않으므로 먼저 Java oracle에서 대상과 실행 조건을 확인합니다.

성공·실패·미실행·미구현·실행 환경 오류를 구분합니다. skip과 알려진 실패는 통과로 집계하지 않습니다. 실패의 최소 재현을 필수 회귀 사례에 추가하고, 테스트 목록과 실제 실행 결과를 대조해 누락을 검출합니다.

## 성능과 메모리

출력이 정확한 실행만 측정에 사용합니다. 필수 워크로드마다 다음 두 조건을 모두 만족해야 합니다.

```text
Rust 실행 시간 / Java 실행 시간 ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

Java 실행 artifact, JDK·Graal/Truffle 설정, 하드웨어·OS와 측정 방법을 고정합니다. 충분히 워밍업한 JVM과 비교하며, 공식 native-image CLI의 결과도 별도로 기록합니다. cold JVM의 시작 시간만으로 전체 성능 목표를 판정하지 않습니다.

| 사용 형태 | 측정 경계 |
| --- | --- |
| CLI | 프로세스 시작부터 출력 완료·종료까지 |
| 상주 evaluator | 초기화, 첫 요청, 반복 요청을 각각 측정 |
| REPL·server | 세션 생성, 요청 지연, 장기 메모리, 종료 |
| N-API | JS↔Rust 값 변환과 결과 전달을 포함 |
| Wasm | 다운로드·컴파일·초기화와 상주 요청을 구분 |
| 패키지·대형 입력 | cold/warm cache, import graph, 큰 출력, 실패 경로 |

반복 실행의 지연 분포와 peak RSS를 워크로드별로 기록합니다. 측정 오차로 판정이 어려우면 미확정으로 보고합니다. 평균 개선으로 개별 필수 워크로드의 퇴행을 숨기지 않습니다.

Wasm·N-API는 Host 프로세스를 포함하는 측정 경계를 먼저 정의합니다. JVM heap, Rust allocation, Wasm linear memory는 원인 분석 지표이며 서로 다른 지표를 직접 비교해 메모리 동등성을 주장하지 않습니다.

## 릴리스 통과 조건

필수 기능·API 목록에 누락이 없고, 대상 플랫폼에서 알려진 필수 불일치가 0건이어야 합니다. 필수 테스트의 미실행·미구현이 남아 있거나 성능·메모리 측정이 미확정이면 완료로 판정하지 않습니다. 이 조건을 충족하기 전에는 호환 완료 또는 Java와 동등한 성능을 보장한다고 문서화하지 않습니다.
