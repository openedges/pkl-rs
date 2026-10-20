# 구현 계획

이 문서는 Pkl **0.32.0** Rust 재구현의 단계별 로드맵, 실행 순서, 산출물, 완료 증거를 함께 관리합니다. 목표와 범위는 [INTENT.md](INTENT.md), 설계 원칙은 [아키텍처](docs/architecture.md), 판정 기준은 [TESTING.md](TESTING.md)를 따릅니다.

## 현재 상태

Git 저장소, Rust workspace, 참조 submodule, 개발 문서와 초기 구성 검증 도구가 준비되어 있습니다. 현재 workspace에는 최소 `pkl-runtime`과 `xtask`가 있으며 Pkl parser, evaluator, 제품 API는 아직 구현되지 않았습니다.

M0의 참조 소스와 Java oracle 고정은 완료했습니다. 호환성 목록과 코퍼스, 차등 실행 도구, 성능 기준선은 구축 중입니다. 초기 Rust 빌드와 테스트, Java 재실행 결과는 Rust 언어 호환성이나 성능 목표 달성의 증거가 아닙니다.

현재 **M0-02 기능·API 목록**, **M0-03 테스트 discovery**, **M0-04 차등 실행 도구**를 병행하고 있습니다. 이후 Pantry 실행과 Java 성능 기준선을 검증합니다.

## 단계별 로드맵

아래 표는 단계별 상태와 완료 기준의 요약입니다. 작업별 산출물과 구체적인 종료 조건은 뒤의 M0–M7 절에서 관리합니다.

| 단계 | 현재 상태 | 선행 조건 | 주요 산출물·완료 기준 |
| --- | --- | --- | --- |
| 초기 구성 | 구축됨 | 없음 | Git·Rust workspace·참조 submodule·개발 문서와 초기 구성 검증 |
| M0 기준선 | 진행 중: 참조 소스·oracle 고정 완료 | 초기 구성 | oracle 고정, API·stdlib·테스트 목록, 차등 실행 도구와 Java 성능 기준선 확보; 환경 재현과 누락·오류 검출 |
| M1 구조 검증 | 계획됨 | M0의 oracle·실행 계약·초기 기준선 | parser·SourceMap·resolver·진단과 regex·숫자·heap·VM 실험; 주요 의미론 위험과 공통 엔진·성능 전략 검증 |
| M2 언어 런타임 | 계획됨 | M1의 이름 해석·값·heap·진단 설계 결정 | 객체·lazy 평가·상속·closure·타입·Reference 구현; 언어 코퍼스의 결과·오류 바이트 일치 |
| M3 표준 라이브러리·출력 | 계획됨 | M2의 해당 언어 기능 | 전체 builtin·renderer·reflection·Pkl Binary 구현; 공개 멤버 누락 0, 출력·오류 일치 |
| M4 외부 기능 | 계획됨 | M1 Host 계약과 M2·M3의 해당 기능 | module/resource Host·HTTP·프로젝트·패키지·external reader 구현; 실제 I/O·패키지 코퍼스 통과 |
| M5 제품 API | 계획됨 | M2·M3·M4의 해당 기능 | CLI·REPL·server·Rust API·formatter·문서 생성·코드 생성·JVM adapter 구현; 기존 호출·client·수명주기 호환 |
| M6 Wasm·N-API 완성 | 계획됨 | M1의 실행환경 실험과 M2–M5의 해당 공통 기능 | 비동기 Host·adapter·배포 패키지 완성; 실제 브라우저·Node의 대상 코퍼스 통과 |
| M7 릴리스 검증 | 계획됨 | M0–M6의 필수 산출물 | 전체 Pantry 코퍼스·플랫폼 검증과 실행 시간·메모리 보고서; 필수 범위의 알려진 불일치 0, 성능·메모리 기준 충족 |

Wasm 빌드와 실제 브라우저·Node의 작은 실행 테스트는 M1부터 구축하고, M6에서 전체 기능으로 확대합니다. 성능과 메모리 측정도 M1부터 구현과 병행합니다.

독립적인 테스트 목록 조사와 adapter 개발은 병행할 수 있습니다. 단계의 시작과 완료는 구분하며, 후속 단계 착수가 선행 단계 완료를 뜻하지 않습니다. 일정과 작업 배분은 M0의 규모 조사와 M1의 실험 결과를 확보한 뒤 정합니다.

## 실행·판정 원칙

- 소스 버전의 기준은 [references.lock](references.lock)과 `external/`의 gitlink입니다. 실행 artifact의 버전·해시·빌드 조건도 별도로 고정합니다. 참조 저장소 변경은 버전 갱신 작업으로만 수행합니다.
- `../pkl-rs.rev1`은 설계와 회귀 입력의 참고 자료입니다. 가져올 구현은 0.32.0 의미론과 비교하고, 가져온 입력의 기대 결과는 고정된 Java oracle로 검증합니다.
- Native·Wasm·N-API는 평가기, 값 모델, 진단, renderer를 공유합니다. 외부 I/O는 Host로 분리하며 제품 평가 경로에 Java fallback을 추가하지 않습니다.
- 작업 상태는 `계획됨`, `진행 중`, `차단됨`, `완료`로 기록합니다. 차단 사유와 해제 조건을 남기며, 완료에는 산출물과 검증 증거가 필요합니다.
- 검증 결과는 `통과`, `실패`, `미실행`, `미구현`, `환경 오류`, `판정 보류`로 구분합니다. 임시 예외에는 범위·이유·해제 조건을 남기고 필수 범위의 완료 분모에서 제외하지 않습니다.
- 새 불일치는 입력을 최소화하고 oracle 출처·재현 환경·실제 바이트를 보관한 뒤 필수 회귀 사례에 추가합니다. Rust 결과로 기대 결과를 덮어쓰지 않습니다.
- 기본 문서와 설계 기록은 영어, 이 문서를 포함한 계획서는 한국어로 유지합니다. upstream 자료와 fixture 원문은 보존합니다.

## M0 — 호환성·성능 기준선

**목적:** 구현 성과를 판단할 수 있는 재현 가능한 Java 기준과 누락을 검출하는 실행 도구를 마련합니다.

### 진행 기록

| 작업 | 상태 | 증거·남은 작업 |
| --- | --- | --- |
| M0-01 | 완료 | [Oracle 계약](docs/oracle.md), `oracle.lock.json`, [실행 증거](reports/m0/oracle-evidence.tar.gz). 공식 JAR 재다운로드·checksum 변조 거부, 성공·오류 원시 바이트 재현, 회귀 검사 13개 통과. 성능용 GraalVM은 macOS arm64 고정이며 다른 플랫폼 확대는 별도 작업입니다. |
| M0-02 | 진행 중 | 기능·공개 API·builtin 목록과 누락 검출 구현·검토 |
| M0-03 | 진행 중 | 전체 소스·실행 사례·Host 의존성 manifest 조사 |
| M0-04 | 진행 중 | 원시 바이트·생성 파일·실행 누락 검출과 실패 주입 검사 |
| M0-05 | 진행 중 | 고정 Pantry 개발 트리·패키징·배포 모듈의 실제 Java 실행 |
| M0-06 | 계획됨 | 최적화 GraalVM을 이용한 cold CLI·상주 평가기 시간·RSS 기준선 |
| M0-07 | 계획됨 | CI·재현 절차와 전체 결과 대조 |

### 작업 계약

| 작업 | 산출물 | 완료 증거 |
| --- | --- | --- |
| M0-01 Oracle 고정 | Java 0.32.0 실행 artifact, checksum, 출처·빌드 절차, JDK·Graal/Truffle 설정과 라이선스 기록 | 새 환경에서 같은 artifact와 버전을 확인하고 성공·오류 사례를 재실행 |
| M0-02 기능·API 목록 | 언어·stdlib 공개 멤버·builtin·renderer·CLI 명령과 옵션·protocol·공개 API별 구현 및 테스트 연결표 | 고정된 upstream 선언과 목록을 대조하고 누락 검출 방식 확인 |
| M0-03 테스트 discovery | upstream, Pantry, rev1 회귀 입력의 출처와 실행 계약을 담은 manifest | 파일 수·실행 사례 수·assertion 수를 구분하고 모든 발견 항목의 처리 상태 기록 |
| M0-04 차등 실행 도구 | 실행기 adapter, 원시 출력 수집, 바이트 비교, 실패 재현 자료와 결과 보고서 | stdout·stderr·종료 코드·생성 파일의 누락과 차이를 의도적으로 주입하여 실패 확인 |
| M0-05 Pantry 정합성 | 고정 revision의 패키지·예제·테스트 실행 목록, 의존성·외부 입력 고정 | Java 0.32.0 실행 결과 확보, 실행 불가 사례의 원인·처리 상태 명시 |
| M0-06 성능 계약 | 필수 workload 목록, 측정 범위·장비·통계·메모리 수집 방법, Java 초기 기준선 | cold CLI와 충분히 warmup한 상주 evaluator를 반복 측정하고 원시 자료 보관 |
| M0-07 실행 통합 | CI의 목록·실행 결과 대조, 검증 도구 자체 회귀, 재현 절차 | 빈 코퍼스·기대 결과 누락·oracle 부재·skip·부분 실행이 성공 보고로 이어지지 않음 |

테스트 manifest에는 `case_id`, 저장소·revision·원본 경로, 입력·리소스 hash, 인자·환경, 필요한 Host 기능, 예상 성공 또는 오류, 비교 채널·파일, 실행 target을 기록합니다. 입력 파일이 여러 사례를 만들거나 여러 파일이 한 사례를 구성하는 관계도 보존합니다.

차등 실행은 작업 경로, locale, 시간대, 터미널, 환경변수와 네트워크 응답을 고정합니다. stdout·stderr와 생성 파일을 원시 바이트로 비교하며 오류의 공백·밑줄·소스 위치·호출 프레임·hint·발생 순서도 포함합니다. `trim`, 정렬, 재직렬화, 경로 문자열 치환으로 차이를 없애지 않습니다.

서버의 무작위 evaluator ID는 고정 ID를 주입한 codec 테스트에서 전체 바이트를 비교합니다. 실제 세션에서는 발급 ID의 참조 일관성과 결과 payload를 검사하며 전체 세션의 바이트 일치로 보고하지 않습니다.

Pantry는 고정 commit만으로 호환성이 확인되지 않습니다. 개발 트리와 패키징된 모듈의 실행 조건, 의존성, 예제 출력과 생성 파일을 조사하고 Java 실행 결과를 확보합니다. 실행 불가 사례를 조용히 제외하지 않으며 참조 submodule을 수정해 통과시키지 않습니다.

아직 Rust evaluator가 없으므로 M0에서는 Java 결과와 실행 도구의 오류 검출을 검증합니다. 실행기 계약 테스트에 사용하는 합성 출력은 Pkl 호환성 결과로 집계하지 않습니다. 실제 Rust 차등 비교는 해당 기능 구현과 함께 연결합니다.

**종료 조건:** oracle 재현 절차, 기능·테스트 목록, 원시 비교 도구, Java 기준 자료와 초기 성능 측정이 검토 가능해야 합니다. 미실행과 환경 오류를 포함한 전체 상태가 보고되어야 하며, M0 완료는 Pkl 구현 완료를 뜻하지 않습니다.

## M1 — 핵심 구조와 호환 위험 검증

**목적:** 런타임 전체에 영향을 주는 선택을 작은 구현과 Java 비교로 검증합니다. 아래 실험은 Native와 초기 Wasm 실행을 함께 고려합니다.

| 실험 | 확인할 내용 | 설계 결정에 필요한 증거 |
| --- | --- | --- |
| Parser·SourceMap·resolver | UTF-8 위치, Java의 UTF-16 위치, Pkl code point 연산; 0.32의 정적 이름 해석; lexical binding·receiver·owner·`this`·`super` | Unicode·잘못된 입력·이름 충돌 사례의 값, 위치, 첫 오류 일치 |
| 정규식 | Java Pattern 문법, Unicode, capture·치환, 오류 문구 | 대표 경계 코퍼스의 Native·Wasm 결과와 오류 바이트 비교 |
| 숫자·수학 | overflow, `Double.toString`, `StrictMath`, NaN, 음수 0, subnormal, 반올림 | 경계값과 생성 입력의 값·출력 비교 및 비용 측정 |
| Lazy 값·heap | 미평가·평가 중·완료·실패 상태, 상속·amendment, memoization, 순환 객체 회수 | forcing·오류 순서 일치와 반복 세션의 메모리 회수 자료 |
| VM·캐시 | bytecode VM 후보, 객체 배치, dispatch, specialization, 컴파일 결과와 평가 상태의 수명 분리 | 올바른 결과를 낸 prototype과 warm JVM의 실행 시간·메모리 비교 |
| 비동기 Host·수명주기 | 중단·재개, 재진입, 취소·timeout·close, 캐시와 외부 입력 | 같은 평가 상태에서의 재개와 종료 후 자원 회수 사례 |

설계 기록에는 검증한 입력, upstream 근거, 원시 결과, 후보별 장단점, 미해결 항목과 다음 검증을 남깁니다. regex나 숫자 구현이 특정 플랫폼에서만 일치하면 공통 해결책의 근거가 확보될 때까지 해당 결정을 미완료로 둡니다.

이 단계부터 실제 브라우저와 Node의 작은 호출 경로를 구성합니다. 공통 engine 연결, 문자열·정수·바이트 전달, 비동기 Host 재개와 오류 전달을 확인합니다. Wasm 컴파일 성공만으로 이 실험을 대체하지 않습니다.

**종료 조건:** 이름 해석·값·heap·진단·Host의 계약을 확정하고 주요 위험에 대한 비교 증거를 확보합니다. bytecode VM을 채택할지 또는 실행 전략을 재검토할지 측정으로 결정합니다. 미달한 실험을 숨기거나 성능 기준을 완화하여 통과시키지 않습니다. 제한된 prototype의 성공은 전체 성능 목표 달성을 뜻하지 않습니다.

## M2 — 언어 런타임

- 전체 문법, 이름 해석, IR·compiler, lazy evaluation, 객체·amendment·상속, closure, generator를 구현합니다.
- 타입·제약·generic·union, `Reference<Domain, Type>`, reflection에 필요한 런타임 메타데이터를 구현합니다. 다중 행 문자열의 continuation 등 0.32 기능을 목록에 연결합니다.
- 구조화된 진단과 논리적 호출 스택으로 구문·이름 해석·실행 단계의 오류 및 선택 순서를 재현합니다. hidden override와 eager/lazy 타입 검사도 검증합니다.
- 해당 upstream 코퍼스와 rev1에서 선별한 회귀 입력을 Java/Rust 차등 실행에 연결하고 property·metamorphic 테스트와 differential fuzzing을 시작합니다.

**산출물·종료 조건:** 언어 목록의 해당 기능이 구현과 테스트에 연결되고 값·오류·위치가 일치해야 합니다. 발견한 불일치는 회귀 사례에 포함하며 언어 범위의 필수 미구현·미실행·알려진 불일치가 없어야 합니다.

## M3 — 표준 라이브러리와 출력

- 고정된 Pkl 표준 라이브러리 소스를 활용하고 모든 external builtin과 공개 멤버를 구현합니다. 등록 목록과 공개 API 목록을 기계적으로 대조합니다.
- PCF, JSON, YAML, XML, Properties, Plist, JSONnet, protobuf, Pkl Binary를 구현합니다. `output.bytes`, `output.files`, converter와 `Reference`의 렌더링·binary 표현을 포함합니다.
- escaping, 숫자 표현, 순서, 공백·개행, 잘못된 값과 형식의 오류, reflection을 비교합니다. 생성 파일의 경로 집합과 각 파일 바이트도 검증합니다.

**산출물·종료 조건:** stdlib 공개 멤버와 builtin 누락이 없고, 각 renderer의 성공·오류·경계값 코퍼스와 binary 비교가 통과해야 합니다.

## M4 — 모듈·리소스·프로젝트·패키지

- import/read/glob, URI, 파일·HTTP·modulepath, external reader를 공통 Host 계약에 연결합니다.
- PklProject, 설정 우선순위, 의존성 해석, dependency notation, local dependency, lockfile·checksum·cache·package 생성을 구현합니다.
- HTTP custom headers, `Project.resolvedEvaluatorSettings`, external reader의 경로·작업 디렉터리 동작을 포함합니다. redirect·proxy·CA, 접근 허용 규칙과 root 제한, 손상된 cache와 실패 경로를 검증합니다.
- 네트워크·파일 fixture를 고정하고 캐시 유무, import graph, 취소·timeout·close에 따른 자원 회수를 비교합니다.

**산출물·종료 조건:** 실제 I/O·프로젝트·패키지 코퍼스가 Java와 일치하고 접근 제한·오류·외부 프로세스 수명주기가 검증되어야 합니다. 해당 Pantry 패키지의 개발 트리와 배포 형태 모두 실행해야 합니다.

## M5 — 제품 API와 도구

- Rust evaluator API를 제공하고 CLI의 `eval`, `repl`, `server`, `test`, `project`, `download-package`, `analyze`, `format`, `run`, `shell-completion`을 구현합니다. 전체 옵션·기본값·help·종료 코드·파일 출력을 대조합니다.
- REPL의 PTY 세션·입력 편집·상태 유지와 server protocol의 codec·callback·요청 순서·취소·종료를 검증합니다. `spec`·`minimal` test reporter도 포함합니다.
- formatter, pkldoc, Java/Kotlin codegen을 구현하고 생성 결과를 바이트 비교합니다.
- 기존 Java/Kotlin 공개 API를 유지하는 JVM adapter를 별도 산출물로 제공합니다. 클래스·메서드·공개 타입·예외·수명주기를 대조하고 실제 기존 client로 확인합니다. 평가는 Rust engine이 수행합니다.

**산출물·종료 조건:** 공개 API 목록의 누락이 없고 기존 호출·client·CLI transcript·REPL·server 세션 및 생성 도구 결과가 계약을 충족해야 합니다.

## M6 — Wasm·N-API 제품 완성

- M1에서 만든 adapter를 전체 공통 engine과 연결하고 실제 브라우저·Node에서 대상 코퍼스를 실행합니다.
- Wasm에는 VFS·비동기 loader 등 Host 기능을 공급합니다. 필요한 기능과 제공 여부를 명시하고, 필수 기능을 제공하지 않는 환경의 사례를 전체 호환 통과로 집계하지 않습니다.
- N-API에는 Node I/O, 비동기 평가·취소·close, 손실 없는 `BigInt`와 `Uint8Array` 전달을 구현합니다. JS와 Rust 사이의 변환도 결과와 성능 검증에 포함합니다.
- Native 라이브러리·CLI, Wasm 배포물, npm 패키지와 JVM adapter의 빌드·설치·실행 절차를 검증하고 지원 플랫폼을 확정합니다.

**산출물·종료 조건:** 배포 artifact를 통해 실제 호출한 결과·오류·수명주기가 일치하고 공통 코퍼스 실행 결과가 target별로 보고되어야 합니다. 문서의 지원 기능은 검증된 범위와 일치해야 합니다.

## M7 — 전체 릴리스 검증

- 기능·API·테스트 목록과 실행 결과를 대조하고 upstream, Pantry, 회귀, fuzzing 발견 사례를 모두 검증합니다.
- 지원 OS·아키텍처·브라우저·Node 조합에서 배포물을 실행합니다. 플랫폼별 경로·개행·터미널 동작은 같은 환경의 Java와 비교합니다.
- 반복 성공·실패, 고유 소스 증가, evaluator 생성·종료, 취소, 캐시 변경과 순환 객체를 포함한 장기 실행으로 메모리 회수를 확인합니다.
- 정확한 결과를 낸 필수 workload 전체에 대해 실행 시간·메모리 측정을 완료하고 원시 자료, 환경, 반복 횟수, 분포와 판정을 공개합니다.

**종료 조건:** 필수 기능·API 누락과 알려진 불일치가 없어야 합니다. 필수 미실행·미구현·환경 오류·판정 보류가 남아 있으면 릴리스 완료로 선언하지 않습니다. 배포물의 재현 절차와 목표 달성 증거를 함께 제공해야 합니다.

## 전 단계의 성능·메모리 검증

필수 workload마다 아래 두 조건을 모두 적용합니다. M0에서 Java 기준을 만들고 M1부터 구현 변화에 따라 측정합니다. M5·M6에서 완성되는 API의 실제 호출 비용도 기준선에 연결합니다.

```text
Rust execution time / Java execution time ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

CLI는 프로세스 시작부터 출력 완료·종료까지, 상주 evaluator는 초기화·첫 요청·반복 요청을 구분합니다. REPL·server의 세션 생성과 장기 실행, cold/warm cache, 큰 import graph·출력·실패 경로도 필수 workload에 포함합니다. 공식 native-image CLI는 별도 비교 자료로 남깁니다.

Wasm은 다운로드·컴파일·초기화와 상주 요청을 분리하고, N-API는 JS↔Rust 변환·결과 전달을 포함합니다. Host 프로세스와 측정 범위를 사전에 고정하여 peak RSS를 비교합니다. JVM heap, Rust allocation, Wasm linear memory는 원인 분석용이며 서로 다른 지표를 직접 나누어 메모리 목표 달성을 주장하지 않습니다.

반복 측정의 지연 분포와 불확실성을 함께 보고합니다. 평균 개선으로 개별 필수 workload의 퇴행을 숨기지 않으며, 차이가 불분명하면 판정 보류로 남깁니다. 성능 개선 후에는 같은 입력의 결과 바이트를 다시 확인합니다.

## 작업 완료와 계획 갱신

작업 완료 기록에는 변경 범위, 기능·테스트 ID, oracle 출처, 실행 환경, 비교 결과와 재현 자료 위치를 포함합니다. 수행하지 않은 검사와 남은 문제를 구분하고, 성능에 영향을 주는 변경에는 해당 workload 결과를 첨부합니다.

현재 실행 가능한 초기 구성 검사는 다음과 같습니다.

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

`cargo xtask verify`는 참조 소스 고정을 확인합니다. 위 명령은 Java 차등 검증이나 제품 성능 측정을 수행하지 않습니다. 차등 실행·성능 측정 명령은 구현 후 [TESTING.md](TESTING.md)에 사용법과 실행 범위를 추가합니다. 단순 문서 수정에는 링크·내용·형식 확인 등 변경에 맞는 검증을 수행합니다.

단계별 로드맵과 상세 실행 계획은 이 문서에서 함께 관리합니다. 증거가 생길 때 단계 상태와 다음 작업을 갱신하고, 단계의 범위나 완료 기준이 바뀌면 요약 표와 해당 상세 절을 함께 수정합니다. 설계 결정은 아키텍처에, 검증 계약 변경은 TESTING.md에 반영합니다. 기준 버전·필수 기능·성능 목표를 바꾸는 경우에는 INTENT.md까지 함께 갱신하고 목표 변경 이유를 명시합니다. 실패를 없애기 위한 범위 축소는 일반 구현 진척으로 기록하지 않습니다.
