# 아키텍처

Pkl 0.32.0을 Rust로 재구현하고 하나의 공통 엔진을 Native·Wasm·N-API에서 사용하는 것이 목표입니다. 이 문서는 설계 방향이며, 현재 workspace에는 최소 `pkl-runtime` 라이브러리와 `xtask`만 존재합니다. 아래 컴포넌트와 기능은 후속 구현 대상입니다.

## 처리 흐름

```text
Pkl source
  → parser + SourceMap
  → name resolver + IR + compiler
  → shared runtime + value model + heap
  → standard library + renderer + structured diagnostics
  → evaluator API
  → CLI / REPL / server / Native API / Wasm / N-API

shared runtime ↔ Host interface ↔ filesystem / HTTP / packages / external reader
                              ↔ browser VFS / asynchronous loader
```

개념상 `syntax`, `parser`, `compiler`, `runtime`, `stdlib`, `host`, `api`, `cli`, `server`, `wasm`, `napi` 영역으로 나눕니다. 실제 crate 분리는 의존성과 구현 범위가 확인될 때 결정합니다. 초기부터 빈 crate를 모두 만들지는 않습니다.

## 공통 런타임

제품용 평가기와 값 모델은 하나로 유지합니다. bytecode VM을 우선 검토하되, 초기 프로토타입에서 Java의 워밍업된 실행 성능과 메모리 기준을 만족할 수 있는지 확인합니다. 실행 전략을 바꾸더라도 의미론·진단·출력 구현은 공유합니다.

Pkl 0.32의 이름 해석을 컴파일 과정에 반영합니다. lexical binding, receiver, owner, `this`, `super`를 구분하고 파싱·이름 해석·실행 단계의 오류 순서를 보존합니다.

lazy 객체는 미평가·평가 중·완료·실패 상태를 명시합니다. 상속·amendment·closure·타입 제약과 memoization을 함께 설계합니다. evaluator 소유 heap은 순환 객체를 포함해 장기 REPL·server에서 해제 가능한 수명주기를 제공해야 합니다.

## 호환성이 필요한 경계

| 경계 | 설계 요구 |
| --- | --- |
| 문자열·SourceMap | UTF-8 위치, Java의 UTF-16 위치, Pkl code point 연산 구분 |
| 숫자 | overflow, 부동소수점 문자열화, NaN·음수 0·반올림과 수학 연산의 결과 일치 |
| 정규식 | Java Pattern의 문법·Unicode·치환·진단 호환 |
| 객체·타입 | 멤버 순서, 강제 평가 순서, override, 제약 검사와 첫 오류 선택 보존 |
| 진단 | 오류 종류·인자·소스 위치·논리적 호출 프레임을 구조화한 뒤 0.32 형식으로 출력 |
| 렌더링 | 줄바꿈·공백·escape·순서·binary encoding까지 바이트 일치 |

표준 라이브러리의 Pkl 원문은 고정한 upstream을 기준으로 사용하고 external builtin을 Rust로 구현합니다. 공개 멤버와 builtin 등록 목록을 대조하여 누락을 검출합니다. `Reference<Domain, Type>` 등 0.32의 기능도 호환 범위에 포함합니다.

## Host와 공개 API

파일·HTTP·패키지·프로세스·외부 reader는 Host 인터페이스로 공급합니다. 코어는 CLI나 Wasm adapter에 의존하지 않습니다. Host 작업을 기다리는 평가는 같은 실행 상태에서 재개할 수 있어야 하며, 취소·timeout·close의 동작을 공통으로 정의합니다.

Wasm에서는 브라우저 VFS와 비동기 loader 등을 사용합니다. Host가 제공하지 않는 기능은 지원 여부를 명시하고 전체 호환 테스트의 성공으로 집계하지 않습니다. N-API에서는 큰 정수와 바이트를 손실 없이 전달하고 Node와 Rust 사이의 변환 비용도 성능에 포함합니다.

공개 범위에는 evaluator, CLI, REPL, server protocol, 프로젝트·패키지 기능, formatter, pkldoc, Java/Kotlin codegen이 포함됩니다. 기존 JVM 공개 API를 유지하는 adapter는 별도 후속 산출물로 계획하며, 그 평가 엔진도 Rust를 사용합니다.

## 캐시와 검증

컴파일 결과의 재사용과 평가 상태의 재사용은 수명주기를 구분합니다. 캐시 키에는 결과에 영향을 주는 소스·설정·의존성을 반영하고, 캐시 유무에 따라 오류나 출력이 바뀌지 않도록 검증합니다.

Java oracle은 개발·검증 도구입니다. 제품의 평가 경로에는 Java fallback을 두지 않습니다. 아키텍처 결정은 최소 재현, 바이트 비교, 워밍업을 포함한 시간·메모리 측정으로 검토합니다. 상세 통과 기준은 [TESTING.md](../TESTING.md)에 정의합니다.
