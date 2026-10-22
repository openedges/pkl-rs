# 구현 계획

이 문서는 Pkl **0.32.0** Rust 재구현의 단계별 로드맵, 실행 순서, 산출물, 완료 증거를 함께 관리합니다. 목표와 범위는 [INTENT.md](INTENT.md), 설계 원칙은 [아키텍처](docs/architecture.md), 판정 기준은 [TESTING.md](TESTING.md)를 따릅니다.

## 현재 상태

Git 저장소, Rust workspace, 참조 submodule, 개발 문서와 초기 구성 검증 도구가 준비되어 있습니다. M1에서 제한된 parser·resolver·lazy VM·heap·Host와 실제 Node·브라우저 호출 경로를 구현했습니다. 전체 Pkl 언어·표준 라이브러리·CLI·REPL·server·제품 API는 아직 구현되지 않았습니다.

M0의 참조 소스·Java oracle 고정, 기능·코퍼스 목록, 원시 차등 실행 도구, Pantry 조사, 초기 Java 성능 측정과 CI 구성을 완료했습니다. 최종 코드 커밋 `2164ffa`의 원격 CI 작업 4개가 모두 성공했고, 내려받은 원시 증거와 Java 재비교 결과도 검증했습니다. [CI 확인 기록](reports/m0/ci-summary.json)을 참조하세요. 초기 Rust 빌드와 테스트, Java 재실행 결과는 Rust 언어 호환성이나 성능 목표 달성의 증거가 아닙니다.

전체 명령 코퍼스 1,112개 중 실행 가능한 1,013개를 모두 호출했습니다. 1,012개 결과 기록·99개 실행 조건 차단·1개 Java 힙 실패로 엄격한 전체 판정은 실패입니다. 2 GiB 힙의 별도 실험은 성공했으며 기존 512 MiB 실패를 대체하지 않습니다. 자세한 결과는 [M0 증거 보고서](reports/m0/README.md)에 모읍니다.

## 단계별 로드맵

아래 표는 단계별 상태와 완료 기준의 요약입니다. 작업별 산출물과 구체적인 종료 조건은 뒤의 M0–M7 절에서 관리합니다.

| 단계 | 현재 상태 | 선행 조건 | 주요 산출물·완료 기준 |
| --- | --- | --- | --- |
| 초기 구성 | 구축됨 | 없음 | Git·Rust workspace·참조 submodule·개발 문서와 초기 구성 검증 |
| M0 기준선 | 완료: 로컬 기준선·원격 CI 검증 확인 | 초기 구성 | oracle·목록·원시 비교 도구·Java 초기 측정과 전체 처리 상태 확보; 판정 보류와 미구현 범위 보존 |
| M1 구조 검증 | 완료: 구조·Graal 조사·원격 CI 증거 확인 | M0의 oracle·실행 계약·초기 기준선 | 제한된 공통 엔진·실제 Node/브라우저·원시 비교와 비용 측정 완료. 후보 기각과 알려진 호환성·성능 실패는 후속 필수 조건으로 보존 |
| M2 언어 런타임 | 진행 중: generic 인자 공유·stack-safe 해제·기본값 continuation 보완; 전체 gate 미완료 | M1의 이름 해석·값·heap·진단 설계 결정 | 객체·lazy 평가·상속·closure·타입·Reference 구현; 언어 코퍼스의 결과·오류 바이트 일치 |
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
- 개발 단계는 기록된 로컬 검증으로 마감할 수 있습니다. 호스팅 CI의 결제·서비스·artifact 전송 문제만으로 구현이나 단계 마감을 중단하지 않습니다. 실제 실행한 host·toolchain·소스·원시 결과와 미실행 OS를 구분하며, 전체 릴리스의 플랫폼·호환성·성능 조건은 유지합니다. 변경되지 않은 검증을 단순히 CI 형태를 흉내 내기 위해 반복하지 않습니다.

## M0 — 호환성·성능 기준선

**목적:** 구현 성과를 판단할 수 있는 재현 가능한 Java 기준과 누락을 검출하는 실행 도구를 마련합니다.

### 진행 기록

| 작업 | 상태 | 증거·남은 작업 |
| --- | --- | --- |
| M0-01 | 완료 | [Oracle 계약](docs/oracle.md), `oracle.lock.json`, [실행 증거](reports/m0/oracle-evidence.tar.gz). 공식 JAR 재다운로드·checksum 변조 거부, 성공·오류 원시 바이트 재현, 회귀 검사 13개 통과. 성능용 GraalVM은 macOS arm64 고정이며 다른 플랫폼 확대는 별도 작업입니다. |
| M0-02 | 완료 | [소스 목록](docs/inventory.md): 985개 파일·11,624개 항목, external 선언·builtin 등록 후보 각 542개 양방향 대응. 재생성·누락 검출과 회귀 검사 18개 통과. 소스 목록이며 컴파일된 ABI 증명은 아닙니다. 구현·행동 테스트 연결은 미구현·미연결 상태로 보존합니다. |
| M0-03 | 완료 | [코퍼스 계약](docs/corpus.md): Pkl 파일 1,546개·project 파일 73개·명령 사례 1,112개와 전체 리소스 hash. 실행 준비 1,013개·Host/adapter 차단 99개, JVM 테스트/지원 소스 199개와 shell suite 1개를 별도 기록. [rev1 회귀 3개](docs/regressions.md)를 원문·라이선스와 함께 별도 선별하고 Java 0.32.0 기록·정확한 재비교 완료. discovery 회귀 검사 8개·선별 입력 검사 4개 통과. |
| M0-04 | 완료 | [실행 도구 계약](docs/compatibility-tooling.md), 원시 stdout·stderr·종료 코드·생성 파일 비교, 입력 hash·실행 누락·timeout 검출. 회귀 검사 19개와 실제 Java smoke 7개 기록·Java 재비교 통과. [기록 증거](reports/m0/tooling-smoke-evidence.tar.gz), [재비교 증거](reports/m0/tooling-replay-evidence.tar.gz). 현재 프로세스 실행 adapter는 POSIX이며 Windows는 명시적 미지원입니다. |
| M0-05 | 완료 | [Pantry 결과](reports/m0/pantry-summary.json), [원시 증거·패키지·cache](reports/m0/pantry-evidence.tar.gz). 29개 project의 패키지/API 테스트 성공, 생성 ZIP 모듈 206개 실행(성공 142·설정 등이 필요한 오류 관찰 64). 다중 출력 4회·설정된 generator 2회·tar pipeline 추가 실행. 원격 의존성 7개 checksum 검증, 실제 cache 72개 파일 보관. 알려진 live GitHub 입력과 잠정 오류는 미검증 범위로 유지합니다. 회귀 검사 5개 통과. |
| M0-06 | 완료 | [측정 계약](docs/benchmarks.md), [측정 결과](reports/m0/benchmarks.json), [원시 증거](reports/m0/benchmark-evidence.tar.gz). 6개 workload·cold CLI 표본 30개·독립 JVM 평가기 실행 24회, 원시 결과 검증과 warmup 수렴 확인. 재귀 함수는 Graal 컴파일 중단이 관찰되어 완전 최적화 여부 판정 보류. 첫 timeout 설정 실험의 힙 실패도 별도 보존. 도구 회귀 검사 9개 통과. |
| M0-07 | 완료 | [CI 계약](docs/ci.md), [전체 결과](reports/m0/README.md), [로컬 검증 증거](reports/m0/validation-evidence.tar.gz), [원격 CI 증거](reports/m0/ci-evidence.tar.gz). 로컬 검증 11개 명령·Python 회귀 검사 79개 통과. `2164ffa`의 [CI 실행](https://github.com/luuvish/pkl-rs/actions/runs/39364517355)에서 Linux·macOS·Windows workspace와 Java tooling 작업 모두 성공. artifact checksum·원시 capture·smoke 재비교 7개를 내려받아 다시 검증했습니다. 전체 코퍼스의 알려진 실패 상태는 별도로 보존합니다. |

### 후속 단계로 넘기는 확인 항목

- 전체 코퍼스의 99개 차단 항목은 formatter·package/project·TLS/HTTP·classpath adapter와 fixture를 구현할 때 해제합니다. JVM 테스트/지원 소스 199개의 실제 API 실행도 별도로 연결합니다.
- 대형 List 사례는 512 MiB 실패와 2 GiB 성공 증거를 함께 검토한 뒤 공식 사례별 자원 계약을 결정합니다. 현재 전체 gate의 실패 상태는 유지합니다.
- M1 [Graal 마감 조사](docs/m1-graal.md)에서 재귀 함수의 컴파일 중단 원인과 실제 Tier 2로 컴파일된 workload를 확인했습니다. 이번 기준선으로 완전 최적화 성능이나 Rust 우위를 주장하지 않습니다. 장기 메모리·제품 API·Wasm·N-API 등 미구현 측정 항목도 유지합니다.

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

### 실행 항목과 마감 기준

M1은 아래 항목별 구현·비교 증거·설계 결정이 갖춰진 뒤 완료로 표시합니다. 초기 prototype의 지원 문법과 probe 범위를 명시하고, 미지원 기능을 성공 처리하지 않습니다. 필수 실험의 실행·증거 수집이 실패하거나 플랫폼별 결과가 다르면 해당 항목을 미완료로 유지합니다. 비교에서 발견한 불일치는 후보의 부적합 판단 근거로 보존하며 호환성 통과로 바꾸지 않습니다. 후보를 기각한 경우 공통 대안의 계약과 근거를 함께 기록해야 구조 검증을 마칠 수 있습니다.

| 항목 | 현재 상태 | 마감에 필요한 증거 |
| --- | --- | --- |
| M1-01 SourceMap·parser | 완료 | [프런트엔드 증거](reports/m1/frontend.json): 제한된 Java/Rust 비교 15/15, UTF-8·UTF-16·code point와 EOF underline 구분. 전체 문법은 M2입니다. |
| M1-02 이름 해석·진단 | 완료 | lexical binding·receiver·owner·this·super의 정적 계약과 Java 값·첫 parser 오류 비교. [계약](docs/m1-frontend.md)에 전체 진단 renderer 미구현을 명시했습니다. |
| M1-03 숫자·정규식 | 완료 | [6,078개 비교](reports/m1/semantics.json)와 Native·Node·Wasm 공통 결과 확보. 숫자 표본은 모두 일치, regex 후보는 Unicode·UTF-16 차이 2개로 기각하고 별도 cursor 증거를 갖춘 공통 대안을 선택했습니다. 숫자 비용 96개 결과 사전 일치·6개 프로세스 수렴 확인: 현재 formatter 약 6.1배 느림을 후속 성능 gate로 남깁니다. |
| M1-04 lazy 값·heap | 완료 | 실패를 영구 캐시하지 않도록 수정했습니다. [마감 차등 증거](reports/m1/lifecycle.json)의 retry protocol 전체 바이트가 Java와 일치하며 첫 오류 순서·복구·성공 의존성 캐싱을 확인했습니다. runtime 회귀 13개 통과, 진단 바이트 차이는 별도 실패로 보존합니다. |
| M1-05 VM·캐시 | 완료 | [일반 VM](reports/m1/runtime-performance.json)은 Java API 대비 약 1.27배 느림. [정수 특수화](reports/m1/runtime-specialized.json)는 약 0.46배, 일반 Rust 대비 약 2.75배 개선. M2의 제한된 기반으로 특수화 bytecode를 선택합니다. [Graal 마감 조사](docs/m1-graal.md)에서 원래 산술·재귀 함수의 프레임 병합 중단 원인과 원본 JAR의 실제 Tier 2 workload를 확인했습니다. 기존 비율은 컴파일 실패 당시 측정이며 전체 성능 gate는 별도입니다. |
| M1-06 Host·수명주기 | 완료 | 같은 VM·heap·성공 member cache를 공유하는 중첩 context를 구현했습니다. [재진입 protocol](reports/m1/lifecycle.json) 바이트가 Java와 일치하며 Host 회귀 15개가 root·token·deadline·종료를 검증합니다. 실제 JS에서 진행 중인 단일·중첩 Host 작업의 abort·Promise 종료·timer/listener/map 해제를 확인했습니다. |
| M1-07 브라우저·Node | 완료 | [마감 플랫폼 결과](reports/m1/platform-closing.json): Native·Node·Chromium의 6,078개 primitive 바이트 모두 동일, Node·브라우저 각각 필수 시나리오 14개 통과. Java 비교는 각 6,076개 일치·동일한 정규식 실패 2개입니다. |
| M1-08 통합·기록 | 완료 | Graal 조사 `bb80685`와 CI 오류 전달 보완 `4cd9c08`을 푸시했고 [최종 코드 CI 5개 작업](https://github.com/luuvish/pkl-rs/actions/runs/39378401720)이 모두 성공했습니다. 원격 artifact의 SHA-256·커밋 소스·원시 결과와 9개 Graal 증거 재검증을 확인했습니다. [최종 CI 기록](reports/m1/ci-graal-summary.json)·[증거](reports/m1/ci-graal-evidence.tar.gz), [로컬 검사 13개·Python 141개](reports/m1/validation-graal.json)를 보관했습니다. 앞선 `6a5e4b5`의 [CI 증거](reports/m1/ci-closing-summary.json)도 유지합니다. |

구문·이름 해석과 VM·heap은 각각 `gpt-6.1-sol / high`, 숫자·정규식 실험은 `gpt-6-astra / high` 에이전트가 담당합니다. 주 에이전트는 Host·플랫폼 연결·통합 검증을 담당하며 공유 인터페이스와 Cargo 설정을 관리합니다. 기능 검증은 병행하고 성능 측정은 호스트를 비운 상태에서 순차 실행합니다.

마감 보완은 공유 VM·Host 구현에 `gpt-6-astra / high`, Java 차등 검증과 Node·브라우저 검증에 각각 `gpt-6.1-sol / high`를 배정합니다. 주 에이전트는 코드 검토·증거 보관·문서·CI 통합을 담당합니다. 구현과 검증 통합, 최종 CI 증거 보관을 단계별로 커밋·푸시합니다. M1-04와 M1-06의 미달 항목이 닫히기 전에는 M1 전체를 완료로 보고하지 않습니다.

앞선 lazy·Host 마감 보완과 독립 코드 검토, 로컬·원격 증거 검증을 완료했습니다. 재검토에서 원래 M1에 배정한 Graal 중단 원인·실제 컴파일 workload 조사가 남아 있음을 확인하여 M1-05를 다시 열어 조사했습니다. 원인 분석은 `gpt-6-astra / high`, workload 실험과 증거 도구는 각각 `gpt-6.1-sol / high`에 배정했습니다. 원본 실패·재컴파일 대조군·진단 우회·원본 JAR 성공을 구분한 9개 사례와 컴파일러 그래프를 확보했고, [로컬 검사 13개·Python 141개](reports/m1/validation-graal.json)가 통과했습니다. 추가 원격 CI 5개 작업과 내려받은 증거 검증까지 완료하여 M1을 제한된 핵심 구조 실험 범위에서 종료합니다. 전체 언어/API 호환성과 성능 목표는 아래 후속 조건 및 M2 이후의 완료 기준으로 남습니다.

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

### M1에서 확인한 후속 구현·검증 조건

- regex의 현재 backend는 Java Unicode 버전과 supplementary 문자 내부의 zero-width 위치에서 실패합니다. Java 21의 Unicode table과 UTF-16 cursor를 갖춘 공통 matcher를 구현하고, 기존 실패 2개를 포함한 전체 비교를 통과해야 합니다. 별도의 빈 pattern·dot cursor 실험은 이 구현 전체의 완료 증거가 아닙니다.
- M1 마감 시 실행 오류 3개의 전체 stderr와 lifecycle 실험의 오류 payload 4개가 Java와 달랐습니다. M2의 공통 진단 경로를 연결한 재검사에서는 실행 오류 2개가 일치했고 순환 참조와 lifecycle payload 차이는 남았습니다. M2의 진단 renderer·논리적 frame·오류 선택과 함께 나머지 바이트 차이를 없애야 합니다. missing-super 동작도 언어 의미론 검증에 포함합니다. 실패 재시도는 마감 실험에서 수정·검증했습니다.
- 중첩 context·실패 복구·종료 시 취소 전달은 작은 구현으로 검증했습니다. 제품 module/resource cache와 공개 API는 후속 구현입니다. Java 0.32.0은 callback 안에서 새 resource를 읽으면 `ConcurrentModificationException`에 의한 `PklBugException`을 낼 수 있습니다. [Java-only 실패 관찰](reports/m1/lifecycle.json)을 보존하며 M4 cache 구현에서 이 동작을 다룹니다. JS의 unpaired surrogate와 장기 clock 범위도 제품 지원 전에 해결해야 합니다.
- 숫자 formatter는 선택한 primitive 비용 측정에서 Java보다 약 6.1배 느립니다. 정확한 출력 바이트를 유지하며 알고리즘·할당 비용을 개선하고 같은 입력·조건에서 다시 측정해야 합니다.
- 특수화 VM의 작은 정수 workload 개선은 전체 성능 통과가 아닙니다. 고정 Graal/Pkl 조합의 `FrameWithoutBoxing` 중단 원인과 실제 Tier 2 workload는 [마감 조사](docs/m1-graal.md)에서 확인했습니다. 원인 확인용 우회는 의미론을 생략하므로 기준 oracle에 적용하지 않습니다. 실제 컴파일된 Java와 같은 입력·호출 경계로 새 성능 측정을 수행하고, 큰 lazy 객체·장기 세션·제품 API를 비교해야 합니다.

## M2 — 언어 런타임

M1의 공통 VM을 확장합니다. VM·heap·호출 의미론은 `gpt-6-astra / high`, parser·resolver와 검증 도구는 각각 `gpt-6.1-sol / high`에 배정합니다. 주 에이전트가 공통 진단 렌더러, 통합 검증, 문서와 커밋·원격 CI를 담당합니다.

### 실행 항목과 진행 상태

| 작업 | 상태 | 구현·검증 범위와 남은 조건 |
| --- | --- | --- |
| M2-01 전체 목록과 실행 판정 | 진행 중 | `compatibility/m2-language.json`에 의미론 분류·upstream 선언·언어 snippet·rev1 후보를 보존합니다. 선언 수는 기능 커버리지가 아니며, 미연결 항목은 전체 gate 실패로 남깁니다. |
| M2-02 lexer·표현식·문자열 | 진행 중 | 숫자·연산자 우선순위·조건식·nullable 연산·raw/multiline/interpolation과 0.32 continuation을 구현했습니다. 고정 Java의 UTF-16 식별자 표, CR/CRLF, 지연 AST 검증 순서를 연결했습니다. 전체 유효·오류 문법 코퍼스 연결이 필요합니다. |
| M2-03 함수·closure·호출 | 진행 중 | 별도 method namespace, 재귀, lexical capture, 매개변수·반환 타입 검사, `\|>` 호출과 함수형 제약을 구현했습니다. Host 중단·재개 중 GC와 함수 타입 metadata·arity 검사를 연결했습니다. 전체 callable·amendment·오류 조건은 후속 항목입니다. |
| M2-04 객체·클래스·상속 | 진행 중 | 클래스·prototype·상속·super 메서드·const capture·runtime identity와 타입 초기화 순서를 연결했습니다. fixed·const 선언 일치, 객체 대입·amendment 제한과 선언별 오류 순서를 구현했습니다. 선언별 singleton의 선행 게시·실패 뒤 부분 상태·GC 보존을 같은 evaluator 순차 계약에 연결했습니다. annotation·module 초기화, 전체 modifier·상속 순환 및 eager/lazy 규칙은 남아 있습니다. |
| M2-05 타입·제약·generic | 진행 중 | primitive·nullable·union·클래스 타입과 generic alias·기본 값을 구현했습니다. 같은 VM의 제약 continuation, custom `this`, 상속된 제약의 선언 owner, alias 실행 환경, 복구 가능한 불일치와 실행 오류 구분을 연결했습니다. generic collection의 lazy wrapper·eager key/List 검사·캐시 재사용을 연결했습니다. 별칭 선언 identity와 확장 검사를 분리하고 함수 타입 metadata를 추가했습니다. 지원하는 타입 구문의 해석 순서와 generic 매개변수 범위를 연결했습니다. 완전한 지연 타입 해석·구조 타입·recursive 타입·전체 제약 코퍼스는 남아 있습니다. |
| M2-06 collection·generator | 진행 중 | 순서 있는 객체 본문, `Dynamic`·`Listing`·`Mapping`의 element·entry·subscript·amendment, `for`·`when`·spread와 `List` 생성 경로를 연결했습니다. 지연 멤버·반복 변수 capture·source 선행 강제 평가·상속 순서·Host 중단 중 GC를 검증합니다. generic wrapper와 일부 inferred new·클래스 기본 값을 연결했습니다. member predicate의 부모 순회·선택·amendment·지연 본문과 선언 owner 캐시를 추가했습니다. 구조 동등성과 객체 키를 확장하며 전체 해시 충돌·순환 키·typed generator·custom default 규칙은 남아 있습니다. |
| M2-07 Reference·메타데이터 | 진행 중 | `pkl:ref` import·생성·합성 접근·정규화·렌더링·동등성과 필요한 선언·타입 정보를 연결했습니다. 상속된 hidden fixed 선언 메타데이터를 referent 본문 실행 없이 조회합니다. 전체 builtin metadata·reflection·원래 upstream suite는 남아 있습니다. |
| M2-08 진단·첫 오류·논리적 스택 | 진행 중 | 공통 uncolored renderer에 제약·union 진단, 선택적인 power assertion 재실행·관찰값 그래프, 값을 강제하지 않는 객체 표시와 UTF-16 길이 제한을 연결했습니다. 재귀 frame 압축·전체 method 진단·전체 오류 선택은 남아 있습니다. |
| M2-09 생성 검사·플랫폼·완료 gate | 진행 중 | Java 차등 입력·metamorphic 관계·seeded 생성과 실제 native/Node/browser 실행을 연결합니다. 작은 선택 코퍼스 성공은 M2 전체 완료가 아닙니다. |

`|>`와 타입 제약에 이어 객체 본문을 순서 있는 멤버 모델로 확장하고 element·entry·generator·spread를 연결했습니다. generic collection wrapper와 member predicate를 추가했으며 다음 구현에는 나머지 collection 규칙, 함수/구조 타입, `Reference`·메타데이터, 전체 진단·upstream 코퍼스가 필요합니다. 현재 선택 사례의 성공은 원래 M2 범위나 종료 조건을 줄이지 않습니다.

타입 제약의 실행 상태·alias 실행 환경·GC와 선택적인 관찰 명령은 [아키텍처](docs/architecture.md)에 기록했습니다. 동일 alias를 타입 인자로 중첩한 오류 span은 새 회귀 사례로 확인했습니다. 클래스 초기화 실패 후 재시도 및 부분 초기화의 전체 규칙, 탐색 과정에서 드러난 Java alias JVM 버그와 미구현 Dynamic method 진단은 별도의 미해결 범위로 유지합니다.

### 원래 범위와 종료 조건

첫 구현과 클래스·타입 별칭 단계는 `fb5a348`, `281ef6c`에 커밋·푸시했고 각 원격 CI 6개 작업과 내려받은 증거를 검증했습니다. 원래 [두 번째 단계 기록](reports/m2/stage2-summary.json)도 유지합니다.

타입 제약을 확장한 [세 번째 단계 최종 로컬 검증](reports/m2/stage3-final-summary.json)에서는 선택 사례 294개 중 292개가 Java와 원시 바이트로 일치했습니다. 순환 참조 오류와 정수 `% 0`의 Java 버그 출력은 실패로 유지하며 새 예외는 추가하지 않았습니다. Metamorphic 관계 17개, Rust 검사 140개, Python 검사 170개, Node·브라우저 각 20개 시나리오가 통과했습니다. 제약 안의 Host 중단·재진입·GC·취소·재시도·종료·시간 초과와 power assertion 재실행 순서도 검증했습니다. 마지막 독립 검토에서 상속된 제약의 잘못된 반환값 오류가 선언·재정의 프레임을 잃는 문제를 수정하고 회귀 사례 2개를 추가했습니다. 수정 전 실패와 이전 292개 선택 기록도 별도로 보존합니다. 전체 목록의 클래스·타입 나머지 규칙, generator·Reference 및 미연결 코퍼스가 남아 있으므로 **M2는 진행 중**입니다. 개발 gate 성공과 엄격한 전체 gate 실패를 구분합니다.

세 번째 단계 구현은 `b00e508`, 최종 오류 프레임 수정은 `5d58e76`에 커밋·푸시했습니다. 최종 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39396439537) 작업 6개가 모두 성공했습니다. [내려받은 증거 검증](reports/m2/ci-stage3-summary.json)에서 두 artifact의 원본 ZIP 해시와 소스 스냅샷 541개(고유 파일 394개)를 정확한 커밋과 대조했습니다. CI의 선택 결과도 292개 일치·나머지 2개 실패이며, 전체 gate는 실패로 유지합니다.

네 번째 단계는 런타임·GC·평가 순서를 `gpt-6-astra / high`, 프런트엔드·독립 검토와 차등 검증을 각각 `gpt-6.1-sol / high`에 배정했습니다. `4512488`에 순서 있는 collection·generator 구현과 회귀 검사를 커밋·푸시했습니다. [최종 로컬 검증](reports/m2/stage4-final-summary.json)은 선택 사례 415개 중 413개가 Java와 원시 바이트로 일치했고, 기존 순환 참조의 oracle 실패와 `% 0` 차이를 유지했습니다. 새 사례 121개와 metamorphic 관계 20개, Rust 163개·Python 171개 검사, 실제 Node·브라우저 각 23개 시나리오가 통과했습니다. source 선행 강제 평가·상속 순서·spread 값 보존·local 중복·module/class 분류·생성 중 Host 중단과 GC의 회귀도 포함했습니다. 원래 294개 선택 입력은 변경하지 않았으며, 수정 전 실패는 [탐색 증거](reports/m2/exploration-stage4-evidence.tar.gz)에 보존했습니다.

객체 키와 구조 동등성·전체 custom default·typed constructor generator와 진단 표시 중 wrapper 실행은 후속 구현이 필요합니다. 함수/구조 타입·`Reference`·메타데이터·전체 진단·언어 코퍼스도 남아 있으므로 **M2 전체는 아직 완료되지 않았습니다**. 전체 gate의 1,950개 실패 이유와 미구현 960개·미연결 40개 accounting row를 유지합니다. 이 수치는 중복되는 요구사항·후보 기록이며 독립적인 기능 개수나 완료율이 아닙니다. 성능·메모리 목표에 대한 새 측정이나 달성 주장은 하지 않습니다.

네 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39403541822) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage4-summary.json)은 원본 artifact ZIP 두 개의 GitHub 해시와 소스 스냅샷 675개(고유 파일 520개)를 정확한 구현 커밋과 대조했습니다. 원격 결과도 413개 일치·기존 실패 2개이며, 전체 M2 gate는 실패로 유지합니다.

다섯 번째 단계는 런타임·GC·평가 순서를 `gpt-6-astra / high`, 프런트엔드·수명주기 검토와 Java 차등 증거를 각각 `gpt-6.1-sol / high`에 배정했습니다. `Listing<T>`·`Mapping<K,V>`·`List<T>` 검사와 wrapper, 일부 inferred new·클래스 기본 값, 별칭과 union 오류 위치를 연결했습니다. 함수 인자에 검사 결과 wrapper를 저장하고 GC의 임시·성공 반환값 추적, Host 재진입의 제약 eager 상태, 제약식 노드 동일성을 수정했습니다. `3a70e03`에 구현을 커밋·푸시했습니다. 원래 415개 입력을 유지하며 145개 사례를 추가한 [최종 로컬 검증](reports/m2/stage5-final-summary.json)은 560개 중 558개가 원시 바이트로 일치했고 기존 `% 0` 차이와 순환 참조 oracle 오류를 유지했습니다. Metamorphic 관계 20개, Rust 172개·Python 171개 검사, 실제 Node·브라우저 각 26개 시나리오가 통과했습니다. 소스 스냅샷 627개와 후보 실행 파일이 수집 중 바뀌지 않았음을 확인했습니다. 진단 표시 도중 부모 캐시를 가져와 wrapper 검사를 실행하는 Java 동작과 그 과정의 Java 오류는 미선택 탐색 증거로 유지합니다. 새 개발 예외는 추가하지 않습니다.

다섯 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39411357487) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage5-summary.json)은 원본 artifact ZIP의 GitHub 해시와 소스 스냅샷 824개(고유 파일 667개)를 정확한 구현 커밋과 대조했습니다. 원격에서도 560개 중 558개 일치·기존 실패 2개, 관계 20개와 Node·브라우저 각 26개 시나리오를 확인했습니다. 전체 gate는 1,950개 실패 이유를 유지하므로 **M2 전체는 계속 진행 중**입니다.

여섯 번째 단계의 member predicate (`[[조건]]`) 구현은 `ca751a1`에 커밋·푸시했습니다. 런타임·평가 순서는 `gpt-6-astra / high`, parser·resolver·독립 수명주기 검토와 Java 차등 검증은 각각 `gpt-6.1-sol / high`에 배정했습니다. 부모 선언의 child-first 순회, 조건과 값의 교대 평가, 선언 owner 캐시, custom `this` 복원, 지연 replacement·amendment, 중첩 generator와 const 문맥을 검증했습니다. 기존 560개 입력을 유지하며 113개를 추가했습니다. [최종 로컬 검증](reports/m2/stage6-final-summary.json)에서 673개 중 671개가 Java와 원시 바이트로 일치했고, 기존 `% 0` 차이와 순환 참조 oracle 오류는 유지했습니다. Metamorphic 관계 23개, Rust 191개·Python 171개 검사, 실제 Node·브라우저 각각 29개 시나리오가 통과했습니다. 소스 스냅샷 744개와 후보 실행 파일의 수집 중 동일성을 확인했습니다. 미선택 탐색 사례 8개와 수정 이전 입력 6개를 별도 증거로 보존하며 새로운 개발 예외는 추가하지 않았습니다. 늘어난 Java 차등 코퍼스와 직전 CI 실행 시간을 고려해 M2 작업 제한 시간을 20분에서 30분으로 늘렸습니다. 호환 판정이나 사례별 timeout·기존 실패 예외는 바꾸지 않았습니다.

여섯 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39417418349) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage6-summary.json)은 원본 artifact ZIP의 GitHub 해시와 소스 스냅샷 945개(고유 파일 784개)를 정확한 구현 커밋 `ca751a1`과 대조했습니다. 원격에서도 673개 중 671개 일치·기존 실패 2개, 관계 23개와 Node·브라우저 각 29개 시나리오를 확인했습니다. 봉인된 증거 패키지의 오프라인 재검증도 통과했고 패키지 내용이 바뀌지 않았음을 확인했습니다. 전체 gate는 1,950개 실패 이유를 유지하며, 구조적 객체 키·동등성, 남은 타입·진단·`Reference`·전체 언어 코퍼스가 미완료이므로 **M2 전체는 계속 진행 중**입니다.

일곱 번째 단계는 구조적 객체·컬렉션 동등성과 객체 키를 구현했습니다. 런타임 continuation·GC·평가 순서는 `gpt-6-astra / high`, Java 차등 사례와 독립 frontend·수명주기 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. 일반 `==`의 숫자 변환과 중첩 값·키의 boxed 비교를 구분하며, 객체 종류별 선행 검사·deep force 순서, 클래스 동일성, 상속·중복·amendment, Host 중단·재진입·실패 재시도를 검증했습니다. 작은 선언 map의 지연 비교와 다섯 번째 멤버의 해시 준비, 상속 키·Mapping 길이의 강제 평가 순서도 연결했습니다. 상수 List 키의 조기 중복 검증, 음수 리터럴의 숫자 토큰 위치, 함수 arity 표시와 Java 진단 highlighter의 보간 공백 동작을 맞췄습니다. 진단 재생은 명시적인 순회 스택을 사용하며 깊이 20,000인 잘못된 입력도 검사했습니다.

구현 커밋 `90f7df4`를 푸시했습니다. 기존 673개 선택 정의·입력 바이트와 알려진 실패를 유지하고 고정 Java에서 원시 결과를 확보한 201개를 추가했습니다. [최종 로컬 검증](reports/m2/stage7-final-summary.json)에서 전체 874개 중 872개가 원시 바이트로 일치했고, 기존 정수 `% 0` 차이와 순환 참조 oracle 오류를 유지했습니다. Metamorphic 관계 26개, Rust 210개·Python 171개 검사, 실제 Node·Chromium 각 32개 시나리오의 개발 계약이 통과했습니다. 기존 Rust probe 1개는 일반 단위 검사에서 계속 ignored이며 별도 차등 adapter로 실행합니다. 공통 primitive 6,078개 중 Java와 일치하는 6,076개 및 기존 regex 실패 2개를 구분합니다. 소스 스냅샷 950개와 후보 실행 파일의 수집 중 동일성을 확인했습니다. 탐색 증거는 내용 파일 14,196개에 원래 실패·미선택·수정 전·반복 수집을 보존하며 새 개발 예외는 추가하지 않았습니다. 전체 해시 충돌 순서·캐시 map 전환·순환 키, 함수/구조 타입·`Reference`·메타데이터·전체 진단·언어 코퍼스는 남아 있으므로 **M2 전체는 진행 중**입니다. 전체 gate의 1,950개 실패 이유를 유지하며 성능·메모리 목표에 대한 새 측정이나 달성 주장은 하지 않습니다.

일곱 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39427102753) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage7-summary.json)은 원본 artifact ZIP 두 개의 GitHub 해시와 소스 스냅샷 1,165개(고유 경로 990개)를 정확한 구현 커밋 `90f7df4`와 대조했습니다. 원격에서도 874개 중 872개 일치·기존 실패 2개, 관계 26개와 Node·브라우저 각 32개 시나리오를 확인했습니다. 두 차례 오프라인 재검증이 통과했고, 마지막 검증에서는 봉인된 패키지 내용과 판정·검증기 식별자가 유지됨을 확인했습니다. 전체 M2 완료 조건과 성능 목표는 계속 열려 있습니다.

여덟 번째 단계는 일반 클래스의 `default` 속성, 성공한 멤버 캐시 삽입 순서와 클래스 선언 순서에 따른 구조 비교를 보완했습니다. Dynamic·Listing·Mapping·typed 객체의 deep force를 마친 뒤 내부 키·값의 해시를 준비하도록 평가 단계를 분리했습니다. 엄격한 List의 순차적 원소 해시 규칙은 유지했습니다. 런타임은 `gpt-6-astra / high`, Java 차등 증거와 독립 수명주기 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다.

원래 874개 정의·입력과 기존 실패를 유지하며 33개 사례와 6개 변형 관계를 추가했습니다. Rust 212개·Python 171개 검사, Wasm 빌드, 기존 874개 재실행, 실제 Node·Chromium 각 35개 시나리오의 개발 계약이 통과했습니다. Host 중단·재진입·GC·실패 재시도·종료를 포함한 수명주기 검사 10개도 통과했습니다. 구현 커밋 `5024708`을 푸시했고 [최종 로컬 검증](reports/m2/stage8-final-summary.json)에서 907개 중 905개가 원시 바이트로 일치했습니다. 기존 정수 `% 0` 차이와 순환 참조 oracle 오류는 유지했습니다. Metamorphic 관계 32개가 통과했고 소스 스냅샷 983개를 정확한 구현 커밋과 대조했습니다. 실제 해시 버킷·캐시 다섯 번째 삽입 전환·일부 local 캐시 및 중첩 `let` 진단 프레임 차이 10개는 탐색 실패로 보존하며 새 개발 예외를 추가하지 않았습니다.

[원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39435577349) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage8-summary.json)은 원본 artifact ZIP 해시와 소스 스냅샷 1,198개(고유 경로 1,023개)를 구현 커밋에 대조했습니다. 두 차례 오프라인 검증이 통과했고 최종 봉인 패키지는 바뀌지 않았습니다. 전체 gate의 1,950개 실패 이유와 성능 목표는 계속 열려 있으므로 **M2 전체는 진행 중**입니다.

아홉 번째 단계는 구조 동등성 ID와 Java 정수 해시를 분리하고, 안정적인 해시의 선언 버킷과 캐시의 부분 게시·재해시·실패 재시도를 구현했습니다. 런타임 continuation은 `gpt-6-astra / high`, 고정 Java 관찰·차등 입력과 독립 map·수명주기 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. 다섯 번째 캐시 값의 선행 게시, 실패 뒤 숨겨진 슬롯과 중복 값, local 캐시의 비대칭 비교, zero-hash 재계산을 검증합니다. Mapping 길이는 별도의 HashSet 규칙으로 조상부터 키를 순차 처리하고 성공한 길이만 게시합니다. 중첩 `let`·타입 제약·wildcard·명시적 cast의 오류 프레임은 소스 메타데이터로 복원합니다.

기존 907개 정의·입력 바이트를 유지하며 152개 사례와 9개 변형 관계를 추가했습니다. 구현 커밋 `2a69a63`을 푸시했고 [최종 로컬 검증](reports/m2/stage9-final-summary.json)에서 1,059개 중 1,057개가 Java와 원시 바이트로 일치했습니다. 기존 정수 `% 0` 차이와 순환 참조 oracle 오류는 유지했습니다. 관계 41개, Rust 221개·Python 171개 검사, Wasm 빌드와 실제 Node·Chromium 각 39개 시나리오의 개발 계약이 통과했습니다. 최종 소스 스냅샷 1,137개를 구현 커밋과 대조했습니다. 마감 회귀 검사에서 발견한 상속 Mapping 길이 차이와 중단된 수집·수정 전 검사 결과를 별도 보존했고 수정된 후보로 전체 검증을 다시 수행했습니다. 기존 탐색 증거를 덮어쓰거나 개발 예외를 추가하지 않았습니다.

[원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39529055834) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage9-summary.json)은 원본 ZIP 해시와 소스 스냅샷 1,357개(고유 경로 1,177개)를 정확한 구현 커밋에 대조했습니다. 두 차례 오프라인 검증이 통과했고 최종 봉인 패키지는 바뀌지 않았습니다. JVM identity hash·같은 캐시의 순환 재진입·순환 키·재귀 진단 압축과 나머지 M2 언어·타입·Reference·메타데이터 범위가 남아 있으므로 **M2 전체는 진행 중**입니다. 전체 gate의 1,950개 실패 이유와 성능·메모리 목표도 계속 열려 있습니다.

열 번째 단계는 [`pkl:ref`](external/pkl/stdlib/ref.pkl)의 실행 경로와 공통 선언·타입 메타데이터를 연결했습니다. 런타임은 `gpt-6-astra / high`, frontend·수명주기 검토와 Java 차등 증거는 각각 `gpt-6.1-sol / high`에 배정했습니다. import alias·qualified 타입 이름, Reference의 domain·data·path·정규화된 referent 타입·합성 접근과 공통 VM의 렌더링·동등성·해시 준비·GC·Host 수명주기를 구현했습니다. 고정 `ref.pkl`의 원본 소스와 URI를 진단에 연결하고 값 종류 및 implicit 문자열 변환 경계도 보완했습니다.

기존 1,059개 정의·입력 바이트를 유지하며 143개 사례와 변형 관계 3개를 추가했습니다. 구현 커밋 `adcfc44`를 푸시했고, 새 고정 Java 수집을 포함한 [최종 로컬 검증](reports/m2/stage10-final-summary.json)에서 선택 1,202개 중 1,200개가 원시 바이트로 일치했습니다. 기존 정수 `% 0` 차이와 순환 참조 oracle 오류를 유지하며 새 개발 예외는 추가하지 않았습니다. 관계 44개, Rust 236개·Python 179개 검사, Wasm 빌드와 실제 Node·Chromium 각 42개 시나리오의 개발 계약이 통과했습니다. 일반 단위 검사에서 제외된 기존 probe 1개는 별도로 유지합니다. 공통 primitive 6,078개 중 Java와 일치하는 6,076개 및 기존 regex 실패 2개를 구분합니다. 소스 스냅샷 1,286개와 후보 실행 파일의 수집 중 동일성을 확인했습니다. 이전 1,059개 CI 실행이 약 24분 걸린 점과 코퍼스 증가를 반영해 M2 작업 시간 예산을 40분으로 늘렸습니다. 사례별 timeout·원시 비교·기존 개발 예외는 바꾸지 않았습니다.

`compatibility/m2-mappings.json`에는 요구사항·Rust 심볼·선택 사례를 연결한 부분 검토 6개를 기록했습니다. 원래 사례의 입력·adapter·실행 상태를 유지하며 부분 구현·부분 검증과 미실행 upstream 사례는 전체 gate 실패로 남습니다. 목록의 요구사항 474개와 사례 기록 2,202개에는 실행 선택 1,202개·미구현 960개·미연결 40개가 포함됩니다. 중복되는 accounting row이며 독립적인 기능 개수나 완료율은 아닙니다. 무결성·개발 gate 성공과 엄격한 선택·전체 gate 실패를 구분하고 전체 gate의 1,950개 실패 이유를 유지합니다.

탐색에서 원본 `basic/reference.pkl`, `errors/reference1..27.pkl`, `api/reference.pkl`을 선택 API 식으로 실행했지만 원래 renderer·snippet suite를 실행한 것으로 대체하지 않습니다. 최종 탐색 후보 191개 중 168개가 일치하고 미구현·진단 차이 23개를 그대로 보존합니다. 초기 debug 재실행 중 후보 실행 파일의 식별자나 전체 소스 연결을 보존하지 못한 기록은 잠정 관찰로 남깁니다. 이전 1,059개 입력 및 새 143개 입력의 탐색 후보 비교와 최종 recorded-build Java 전체 수집도 구분합니다. 일반 import·module inheritance, Map·Set·Duration·함수 타입, 일부 fixed 규칙·alias 진단·전체 reflection·원래 upstream 코퍼스는 남아 있으므로 **M2 전체는 진행 중**입니다. 성능·메모리 목표에 대한 새 측정이나 달성 주장은 하지 않습니다.

구현 커밋의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39538489442) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage10-summary.json)은 원본 ZIP의 GitHub 해시와 소스 스냅샷 1,520개(고유 경로 1,326개)를 정확한 구현 커밋 `adcfc44`와 대조했습니다. 원격에서도 1,200개 일치·기존 실패 2개, 관계 44개와 Node·브라우저 각 42개 시나리오를 확인했습니다. 두 차례 오프라인 재검증이 통과했고 최종 봉인 패키지는 바뀌지 않았습니다. 열 번째 단계의 제한된 구현·검증은 마쳤으며 **M2 전체 완료 조건과 성능 목표는 계속 열려 있습니다**.

열한 번째 단계는 `fixed` 선언·상속·객체 수정 규칙과 Reference의 선언 modifier 메타데이터를 공통 엔진에 연결했습니다. 클래스는 속성 선언 순서대로 타입을 해석한 뒤 `fixed`, `const` 일치를 검사하고 메서드는 속성 다음에 검사합니다. 부모의 fixed 속성은 fixed 선언으로 재정의할 수 있으며 실제 객체에 대한 대입·amendment는 거부합니다. 초기화 본문을 실행하지 않는 상속 메타데이터 조회와 일반 receiver의 지연 평가·virtual dispatch를 유지합니다. authored amendment와 명시적 대입을 구분해 abstract amendment를 허용하고, abstract 본문·modifier 오류의 header 범위와 선언 프레임을 보완했습니다. 중복 modifier의 덧셈 결과와 header 문자열 검색에 따른 진단 위치도 고정 Java와 비교했습니다.

**같은 evaluator에서 실패한 클래스 초기화를 재시도하는 동작은 알려진 불일치로 남습니다.** 고정 Java는 superclass·속성 초기화가 끝나기 전에 클래스 singleton을 게시하고, 오류 뒤에도 그 객체와 완료된 선언 prefix를 유지합니다. 같은 클래스의 첫 접근은 오류가 나더라도 후속 접근은 부분 초기화된 클래스를 반환합니다. Rust는 초기화를 다시 수행해 같은 오류를 반복합니다. 같은 입력·같은 evaluator 순차 프로토콜에서 첫 오류의 전체 원시 바이트는 일치하고 뒤의 두 호출은 Java 성공·Rust 오류로 달랐습니다. [원시 증거](reports/m2/exploration-stage11-evidence.tar.gz)에 보존한 이 실패는 독립 식 선택 1,291개와 별개이며 개발 예외나 통과로 바꾸지 않습니다. Host read의 실패 재시도 성공이 이 클래스 수명주기 차이를 해결한 것으로 간주하지 않습니다.

구현 커밋 `52cabbe01a8392ee44bf55889346c6f692d3c595`를 푸시했습니다. 기존 선택 1,202개 정의·입력 바이트를 유지하고 89개 사례와 변형 관계 2개를 추가했습니다. 새 고정 Java 수집과 recorded-build 후보를 포함한 [최종 로컬 검증](reports/m2/stage11-final-summary.json)에서 선택 1,291개 중 1,289개가 원시 바이트로 일치했고 관계 46개가 통과했습니다. 기존 정수 `% 0` 차이와 순환 참조 oracle 오류를 유지하며 새 개발 예외는 추가하지 않았습니다. [전체 언어 증거](reports/m2/language-stage11-final-evidence.tar.gz)의 소스 스냅샷 1,378개를 정확한 구현 커밋과 대조했고 수집 중 소스·후보 실행 파일의 동일성을 확인했습니다. [이전 1,202개 입력 재실행과 새 89개 사전 비교](reports/m2/regressions-stage11-final-evidence.tar.gz)는 별도의 탐색 후보 결과로 유지하며 recorded-build 전체 새 수집과 구분합니다.

[기본 검증 기록](reports/m2/validation-stage11-final.json)과 [원시 출력](reports/m2/validation-stage11-final-evidence.tar.gz)에서 참조 pin, formatting, workspace Clippy, Rust 251개·Python 179개 검사, Wasm 빌드와 inventory freshness가 통과했습니다. 기존 ignored probe 1개는 일반 단위 검사 통과 개수에 포함하지 않으며 별도 차등 실행 계약을 유지합니다. [실제 Node·Chromium](reports/m2/platforms-stage11-final-evidence.tar.gz)은 각각 45개 시나리오의 개발 계약을 통과했습니다. 공통 primitive 6,078개 중 Java와 일치하는 6,076개와 기존 regex 실패 2개를 구분합니다. 새 fixed 수명주기 검사 3개는 상속 initializer의 receiver, Host 중단·재진입·GC·실패 재시도·종료와 hidden fixed Reference의 referent 비실행을 확인합니다.

[탐색 증거](reports/m2/exploration-stage11-evidence.tar.gz)는 독립 식 관찰 111개 중 최종 후보 100개 일치와 미선택 불일치 11개를 보존합니다. 이 개수에는 별도 같은 evaluator 클래스 재시도 프로토콜이나 플랫폼 URI 관찰을 더하지 않습니다. 이전 Reference 탐색 191개를 다시 비교한 결과는 169개 일치·22개 불일치이며 `property-fixed`가 일치로 바뀌었습니다. 원본 fixed snippet 14개를 선택 API 식으로 관찰한 결과도 원래 snippet·renderer·assertion 계약의 실행을 대체하지 않습니다. 탐색 후보의 소스 연결은 recorded-build로 검증되지 않았고 구현 커밋과의 소스 차이는 `crates/pkl-runtime/examples/lifecycle_probe.rs`의 `fixed: false` 필드 보완 전 기록입니다. 초기 실제 URI 부분 수집·수정 전 후보·잘못 작성된 기대 metadata·upstream `PklBugException`과 [실패한 초기 lifecycle 빌드 및 대체 전 runtime 수집](reports/m2/exploration-stage11-build-failures-evidence.tar.gz)을 그대로 남겼습니다.

[부분 검토 연결표](compatibility/m2-mappings.json)는 11개이며 원래 upstream 실행 계약과 미실행 상태를 유지합니다. 목록의 요구사항 474개와 중복되는 accounting row 2,291개에는 실행 선택 1,291개·미구현 960개·미연결 40개가 포함됩니다. 이는 독립적인 기능 개수나 완료율이 아닙니다. [gate 증거](reports/m2/gates-stage11-final-evidence.tar.gz)는 무결성·개발 gate 통과와 엄격한 선택·전체 gate 실패를 구분하고 전체 gate의 1,950개 실패 이유를 보존합니다. **M2 전체는 진행 중**이며 성능·메모리 목표에 대한 새 측정이나 달성 주장은 하지 않습니다.

구현 커밋의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39543295843) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage11-summary.json)은 원본 ZIP 2개의 GitHub 해시와 소스 스냅샷 1,615개(고유 경로 1,418개)를 정확한 구현 커밋 `52cabbe`와 대조했습니다. 원격에서도 1,289개 일치·기존 실패 2개, 관계 46개와 Node·Chromium 각 45개 시나리오의 개발 계약을 확인했습니다. [원본 CI 증거](reports/m2/ci-stage11-evidence.tar.gz)를 보존하고 캡처된 실행 파일을 실행하지 않는 두 차례 [오프라인 재검증](reports/m2/ci-stage11-offline-evidence.tar.gz)을 통과했으며 최종 봉인 패키지는 바뀌지 않았습니다. 열한 번째 단계의 제한된 구현·검증은 마쳤지만, 같은 evaluator의 클래스 초기화 재시도 차이와 나머지 M2 완료 조건·성능 목표는 계속 열려 있습니다.

열두 번째 단계는 Stage 11에서 남긴 같은 evaluator의 클래스 초기화 실패 뒤 재접근 차이를 현재 지원하는 클래스 선언 범위에서 해결했습니다. 런타임·GC·평가 순서는 `gpt-6-astra / high`, 차등 증거·수명주기·독립 검토는 `gpt-6.1-sol / high`에 배정했습니다. 구현 커밋 `50fc60b2aeac6fa9bb43c6c6043fc8f9976cb287`을 푸시했습니다. superclass 해석 전 singleton을 게시하고, 성공한 속성 선언과 그 다음 메서드를 순차 게시합니다. 오류 뒤에는 같은 handle과 완료된 prefix를 유지하며 초기화를 다시 실행하지 않습니다. 일반 lazy 속성의 실패 캐시는 비우고 클래스 singleton은 별도 evaluator 소유 GC root로 유지합니다. typed missing-property·missing-method 진단은 실제 게시된 선언과 상속된 후보를 사용합니다.

[독립 순차 비교 요약](reports/m2/stage12-class-lifecycle.json)과 [원시 증거](reports/m2/stage12-class-lifecycle-evidence.tar.gz)는 고유 순서 목록 18개를 새 evaluator에서 각각 두 번 실행한 36회와 호출별 원시 비교 194개 전부의 일치를 기록합니다. 각 목록은 오류 뒤에도 같은 Java evaluator와 Rust VM·module을 유지합니다. 최초 superclass·속성 타입·modifier·method 초기화 실패, 재접근 identity, 게시된 prefix·미게시 tail, 부분 클래스 대입, 일반 wrapper 재시도, 재귀 타입과 partial Reference 메타데이터를 비교했습니다. 새 Rust 회귀 19개는 핵심 의미 9개·수명주기 8개·공통 순차 API 2개이며, 새 Python 도구 회귀는 15개입니다. GC의 정확한 보존 그래프·임시 handle 회수·heap 격리와 terminal 해제는 별도 구조 검증이며 Java 언어 호출 수에 합산하지 않습니다. 수정 전 원시 진단 불일치 22개와 build 중 소스 변경에 따른 기각, Java 탐색 오류·timeout도 같은 증거에 남깁니다.

최종 기본 검사에서 Rust 270개와 Python 194개가 통과했고 기존 ignored probe 1개는 일반 통과 개수에서 제외했습니다. 실제 Node·Chromium은 각각 46개 시나리오의 개발 계약을 통과했으며 공통 primitive 6,078개 중 Java와 일치하는 6,076개와 기존 regex 실패 2개를 구분합니다. 기존 독립 식 1,291개 정의·입력과 변형 관계 46개는 바꾸지 않았습니다. 이전 Java 캡처 재비교와 새 Java·recorded-build 후보의 최종 전체 수집 모두 1,289개 일치·기존 실패 2개를 기록했고 관계 46개가 통과했습니다. 새 언어 수집의 소스 스냅샷은 1,382개이며, 최종 언어·순차 클래스·M1 회귀 수집의 소스 사본 1,738개(고유 경로 1,462개)를 구현 커밋 `50fc60b`와 바이트 단위로 대조했습니다.

[최종 로컬 요약](reports/m2/stage12-final-summary.json)은 [새 Java 전체 비교](reports/m2/language-stage12-final-evidence.tar.gz), [기본 검사](reports/m2/validation-stage12-final-evidence.tar.gz), [실제 플랫폼 실행](reports/m2/platforms-stage12-final-evidence.tar.gz), [기존 회귀와 이동 재검증](reports/m2/regressions-stage12-final-evidence.tar.gz)을 연결합니다. [소스 대조 증거](reports/m2/source-stage12-final-evidence.tar.gz)는 정확한 커밋 대조와 독립적인 결과 목록·원시 집계 검토를 보존합니다. 기존 M1 회귀는 frontend 15개, runtime 값 6개·전체 진단 2개 일치와 cycle 진단 1개 불일치, lifecycle protocol 2개 일치와 전체 진단 4개 불일치입니다. [gate 증거](reports/m2/gates-stage12-final-evidence.tar.gz)는 무결성·개발 계약 통과와 엄격한 선택·전체 완료 gate 실패를 분리합니다.

구현 커밋의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39548215484) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage12-summary.json)은 원본 ZIP 2개의 GitHub 해시와 소스 사본 1,746개(고유 경로 1,465개)를 정확한 구현 커밋 `50fc60b`와 대조했습니다. 원격에서도 독립 식 1,289개 일치·기존 실패 2개, 관계 46개, 별도 클래스 순차 호출 194개 일치와 Node·Chromium 각 46개 시나리오의 개발 계약을 확인했습니다. [원본 CI 증거](reports/m2/ci-stage12-evidence.tar.gz)를 보존하고 캡처된 실행 파일을 실행하지 않는 두 차례 [오프라인 재검증](reports/m2/ci-stage12-offline-evidence.tar.gz)을 통과했습니다. 최종 봉인 패키지 574개 파일은 바뀌지 않았습니다. 이전 보고서의 [CI 실행](https://github.com/luuvish/pkl-rs/actions/runs/39546025680)도 6개 작업 성공 상태로 보존하며 두 커밋의 증거를 구분합니다. 열두 번째 단계의 제한된 구현·검증은 마쳤지만 M2 전체 종료 조건은 충족하지 않았습니다.

별도의 [잠정 보존 증거](reports/m2/exploration-stage12-provisional-evidence.tar.gz)는 내용 파일 2,536개와 완료된 process pair 79개, 중단된 최초 언어 수집·실패한 Node 후보·대체 전 기본 검사와 M1 기록을 보존합니다. 이들은 최종 성공 수에 포함하지 않습니다. [마감 검토 증거](reports/m2/stage12-class-review-evidence.tar.gz)는 실패한 override 뒤 미게시 local 선언을 읽는 별도 탐색에서 Java 내부 `PklBugException`과 Rust missing-property 진단의 차이를 보존합니다. 이 탐색은 선택 순차 목록 18개 밖의 미해결 Java 내부 오류 진단 범위이며 통과나 개발 예외로 추가하지 않습니다.

Java에서는 annotation 생성 중 `read`와 같은 evaluator의 부분 클래스 재진입이 실제로 발생함을 관찰했습니다. Rust frontend의 annotation 문법이 미지원이므로 실제 언어의 이 Host 경로는 미실행으로 남기며, 내부 instruction 주입 중단·재진입 검사를 Java 언어 호환성으로 계산하지 않습니다. 같은 URI의 변경 source를 무시하는 Java module cache identity와 다른 URI·evaluator의 분리는 Java 단독 관찰이며 Rust의 완전한 module cache 계약은 M4 연결 범위입니다. 간접 순환 superclass의 class identity는 비교했지만 Java의 property 순회는 두 번 모두 30초 timeout이었고 Rust는 visited-set 오류로 무한 순회를 막습니다. 이 순회 차이를 숨기거나 새로운 성공 사례로 바꾸지 않습니다. **M2 전체는 계속 진행 중**이며 전체 gate의 1,950개 실패 이유, 나머지 필수 미구현·미실행과 성능·메모리 목표를 유지합니다.

열세 번째 단계는 사용자 TypeAlias 선언 값과 함수 타입 metadata를 공통 엔진에 연결했습니다. 런타임·타입 해석은 `gpt-6-astra / high`, Java 차등 검증·수명주기·독립 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. 구현 커밋 `f90d7b09124bb5a7d73dd5b2bfcddf2a90d2761f`를 푸시했습니다. 별칭의 선언 identity와 확장 검사를 분리하고, 실패한 초기화 뒤에도 게시된 별칭 값을 유지합니다. 나중에 그 별칭을 타입으로 사용하면 authored 위치에서 cyclic-alias 오류를 내며 클래스의 부분 게시 상태와 함께 검증합니다. Arrow·FunctionN annotation은 arity를 검사하고 Function은 지원하는 함수 arity를 허용합니다. lambda 자신의 선언 검사는 호출 때 유지하며 annotation이 추가 argument/result wrapper를 만들지는 않습니다. Reference의 동등성과 subtype 검사를 분리하고 exported arrow metadata 안의 별칭 identity·정확한 제약 source text를 보존합니다.

새 독립 식 89개와 변형 관계 2개를 추가해 선택은 1,380개·관계 48개입니다. 기존 1,291개 정의·입력 바이트·46개 관계를 그대로 유지했습니다. 별도 같은 evaluator 순차 계약은 27개 목록을 두 번씩 실행한 54회·호출 302개가 모두 Java와 원시 바이트로 일치했습니다. 기존 클래스 호출 194개와 새 별칭·함수 호출 108개를 구분합니다. 실패한 별칭을 속성 타입·`is`·`as`·typed let·lambda·constructor·superclass에서 다시 쓰는 순서까지 추가했습니다. 기존 18개 클래스 목록·36개 입력 파일과 미실행 목록은 바꾸지 않았습니다.

필수 검사 7종에서 Rust 288개·Python 194개가 통과했고 기존 ignored probe 1개는 별도입니다. 새 Rust 회귀 18개는 핵심 의미 12개와 GC·Host 수명주기 6개이며, exact retained graph·임시 handle 회수·VM 격리·실패 재시도·terminal 해제를 확인합니다. 실제 Node·Chromium은 각각 49개 시나리오의 개발 계약을 통과했고 공통 primitive 6,078개 중 Java 일치 6,076개와 기존 regex 실패 2개를 유지합니다.

[탐색 증거](reports/m2/exploration-stage13-provisional-evidence.tar.gz)는 잘못된 초기 parser 입력, 최초 순차 목록 작성 오류, 수집 중 소스 변경으로 기각한 기록과 수정 전 진단 불일치를 보존합니다. 최종 release 후보로 새 89개가 모두 일치했고, 이전 Reference 탐색 191개에서는 173개 일치·18개 불일치를 기록하여 이전 실패 4개를 해결했습니다. 이는 원래 upstream snippet·renderer·API suite 실행을 대체하지 않습니다. 직접 제공한 후보의 원래 provenance 표시는 유지하며 별도 기록한 release build와 실행 파일 해시가 같음을 확인했습니다.

[최종 로컬 요약](reports/m2/stage13-final-summary.json)의 선택된 1,380개 식의 새 Java 수집은 1,380개 중 1,378개 일치·기존 실패 2개를 기록했고 변형 관계 48개가 통과했습니다. 순환 참조 oracle 오류와 정수 `% 0` 진단 차이는 그대로 유지합니다. 수집 중 소스·후보 실행 파일 식별자가 유지됐고 언어 소스 스냅샷은 1,473개입니다. 최종 7개 증거 suite의 소스 사본 1,852개(고유 경로 1,572개)를 정확한 구현 커밋에 대조했습니다. [소스 증거](reports/m2/source-stage13-final-evidence.tar.gz)는 결과 순서·원시 집계 검토와 패키징 첫 실패도 보존합니다. 보조 스크립트가 변형 관계의 등장 순서를 기대했지만 실제 harness는 case-ID tuple 순서로 정렬하므로 검증 가정을 수정했습니다. 원시 결과나 구현은 바꾸지 않았습니다.

[회귀 증거](reports/m2/regressions-stage13-final-evidence.tar.gz)는 이전 1,291개 식 재비교와 M1 frontend 15개, runtime 값 6개·전체 진단 2개 일치와 cycle 진단 1개 불일치, lifecycle protocol 2개 일치와 전체 진단 4개 불일치를 보존합니다. [gate 기록](reports/m2/gates-stage13-final-evidence.tar.gz)은 무결성·개발 계약 통과와 엄격한 선택·전체 gate 실패를 구분합니다. 부분 검토 연결은 15개이며, 목록의 요구사항 474개와 중복 accounting row 2,380개에는 ready 1,380개·미구현 960개·미연결 40개가 있습니다. 이는 독립 기능 수나 완료율이 아니며 전체 gate의 1,950개 실패 이유를 유지합니다.

구현 커밋의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39554130320) 작업 6개가 모두 성공했습니다. [독립 검증](reports/m2/ci-stage13-summary.json)은 원본 ZIP 2개의 GitHub 해시와 소스 사본 1,860개(고유 경로 1,575개)를 정확한 구현 커밋에 대조했습니다. 원격에서도 1,378개 식 일치·기존 실패 2개, 변형 관계 48개, 별도 순차 호출 302개와 Node·Chromium 각 49개 시나리오를 확인했습니다. [원본 CI 증거](reports/m2/ci-stage13-evidence.tar.gz)와 두 차례 [오프라인 재검증](reports/m2/ci-stage13-offline-evidence.tar.gz)이 통과했고 최종 패키지 182개 파일은 바뀌지 않았습니다. Artifact 다운로드의 timeout·TLS 실패는 별도 전송 재시도로 해결했으며 원본 ZIP 해시는 GitHub와 일치합니다. 실패한 전송 기록의 해시와 민감한 임시 URL을 제외한 metadata를 보존했고 CI나 언어 코퍼스를 다시 실행하지 않았습니다. 열세 번째 단계의 제한된 구현·검증을 마쳤습니다.

Stage 13 시점에는 완전한 지연 타입 해석 모델, 미해결 이름과 별칭 의존성이 섞인 오류 순서, generic 매개변수 shadowing 실패가 남아 있었습니다. strict Map·Set, annotation, module inheritance/cache, 전체 reflection·원래 upstream 언어 코퍼스도 남아 있습니다. 새로운 개발 예외나 성능·메모리 목표 달성 주장은 추가하지 않았으며 **M2 전체는 계속 진행 중**입니다.

열네 번째 단계는 지원하는 타입 구문의 해석 순서와 generic 매개변수 범위를 보완했습니다. 공통 VM·타입 해석은 `gpt-6-astra / high`, Java 차등 증거와 수명주기·플랫폼·독립 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. 구현 커밋 `4b27b7ecf2c6133e363ed228606b289e85c2e627`을 푸시했습니다. 별칭·클래스의 서로 다른 초기화·인자 검사 순서를 authored 위치의 명령으로 실행하며, 앞선 이름 오류 뒤에 나오는 선언은 게시하지 않습니다. bare 매개변수의 이름 가림과 `T<String>`처럼 인자가 붙은 모듈 선언의 해석을 구분하고 중첩 별칭의 호출 위치 범위와 union 진단 이름을 보존합니다. owner 초기화 시점의 signature 오류를 연결했으며 확장된 값 검사 metadata는 여전히 정적으로 컴파일합니다.

[최종 로컬 요약](reports/m2/stage14-final-summary.json)과 [새 Java 전체 비교](reports/m2/language-stage14-final-evidence.tar.gz)는 1,472개 독립 식 중 1,470개 일치·기존 oracle 오류와 `% 0` 차이 2개를 기록합니다. 새 사례 92개는 성공 값 31개·예상 오류 61개이며 모두 전체 원시 바이트가 일치했습니다. 원래 1,380개 정의·입력과 관계 48개를 유지하고 변형 관계 50개가 통과했습니다. [같은 evaluator 순차 비교](reports/m2/class-lifecycle-stage14-final-evidence.tar.gz)는 고유 목록 32개를 두 번씩 실행한 64회·406개 호출이 모두 일치했으며 원래 27개 목록·302개 호출의 정의·순서·원시 결과를 보존합니다.

[필수 검사](reports/m2/validation-stage14-final.json)에서는 Rust 303개·Python 194개, formatting·Clippy·Wasm·참조 pin·목록 freshness가 통과했습니다. 기존 ignored probe 1개는 일반 통과 개수에서 제외합니다. 새 Rust 검사는 핵심 8개·수명주기 7개이며 실제 Node·Chromium은 각 52개 시나리오가 통과했습니다. 공통 primitive 6,078개 중 Java와 일치하는 6,076개와 기존 regex 실패 2개를 구분합니다. 언어 수집은 소스 스냅샷 1,567개를 보존하며 [소스 대조 증거](reports/m2/source-stage14-final-evidence.tar.gz)는 최종 7개 suite의 사본 1,961개·고유 경로 1,677개를 정확한 구현 커밋과 대조합니다.

[탐색 증거](reports/m2/exploration-stage14-evidence.tar.gz)는 98개 관찰 중 96개 일치·미선택 compound 출력 adapter 한계 2개, 수정 전 오류와 최종 focused 92개 일치를 보존합니다. 이전 Reference 191개는 173개 일치·18개 불일치로 유지됩니다. [대체 전 증거](reports/m2/superseded-stage14-evidence.tar.gz)에는 owner 초기화로 옮겨진 오류에 대해 컴파일 실패를 기대하던 기존 검사 실패와 명시적으로 중단한 첫 수집을 남겼습니다. 검사 시점과 오류 내용을 수정한 후 변경된 소스가 포함된 suite를 새 디렉터리에서 재수집했습니다. 부분 연결표 15개, 요구사항 474개, accounting row 2,472개(ready 1,472·미구현 960·미연결 40), 엄격한 전체 gate의 실패 이유 1,950개는 완료율이 아닙니다.

[구현 CI](https://github.com/luuvish/pkl-rs/actions/runs/39561429923)는 GitHub 계정의 결제 실패 또는 사용 한도 문제로 작업 6개가 모두 시작되지 않았습니다. [차단 요약](reports/m2/ci-stage14-summary.json)과 [원시 증거](reports/m2/ci-stage14-blocked-evidence.tar.gz)에 원인·실행하지 못한 검사·artifact 0개를 기록했습니다. 원격 테스트, 원본 ZIP 해시·소스 대조, 두 차례 오프라인 재검증은 미실행입니다. 이 기록은 원격 실행의 실패·미실행 증거로 유지하며 직전 보고서 커밋의 별도 CI 6개 작업 성공과 구분합니다. 이후 사용자가 로컬 검증으로 진행하도록 승인했습니다. [독립 로컬 수용 검사](reports/m2/local-acceptance-stage14.json)와 [봉인된 증거](reports/m2/local-acceptance-stage14-evidence.tar.gz)에서 기존 archive·정확한 구현 소스·원시 결과를 다시 검증했고 필요한 Java 식별자 테이블 검사도 새로 실행해 일치했습니다. 따라서 **Stage 14의 제한된 개발 계약은 로컬 검증으로 마감했으며 M2 전체는 계속 진행 중**입니다. Linux·Windows 실행은 미검증으로 유지하며 존재하지 않는 CI ZIP을 요구하거나 로컬 증거를 원격 실행으로 표시하지 않습니다.

열다섯 번째 단계의 구현 커밋 `7eca4046a4a40519d8a53348deb94ed98827b5a7`을 푸시했습니다. non-generic 재귀 제약의 컴파일 무한 확장을 유한 bytecode graph로 바꾸고 `Class<T>`의 인자 소거 규칙을 연결했습니다. 런타임은 `gpt-6-astra / high`, Java 증거·플랫폼·독립 검증은 `gpt-6.1-sol / high`로 분담했습니다. 미사용·단락 평가·유한 직접/상호 재귀·별칭 선언 위치의 타입 범위와 Host/GC 수명주기를 검증했으며 확장된 값 검사 metadata는 여전히 정적으로 표현합니다.

새 사례 41개는 성공 값 33개·예상 오류 8개이며 Java와 전체 원시 바이트가 일치했습니다. 변형 관계 2개를 추가하여 선택은 1,513개·관계 52개입니다. 원래 1,472개 정의·입력 바이트·50개 관계와 기존 요구사항 연결을 보존했습니다. 같은 evaluator의 순차 계약은 고유 목록 34개를 두 번씩 실행한 68회·454개 호출이 모두 일치했으며, 기존 32개 목록·406개 호출의 status·stdout·stderr 사본 2,436개가 이전 결과와 같은 바이트임을 독립 확인했습니다.

[새 Java 전체 비교](reports/m2/language-stage15-final-evidence.tar.gz)는 1,513개 중 1,511개 일치와 기존 `error-cycle` oracle 오류·`error-remainder-zero` 진단 차이를 기록했고 변형 관계 52개가 모두 통과했습니다. 수집 중 소스·후보 실행 파일이 유지됐으며 언어 소스 스냅샷은 1,610개입니다. [이전 선택 재비교](reports/m2/regressions-stage15-final-evidence.tar.gz)는 Stage 14 Java 원시 결과를 사용해 기존 1,470개 일치·2개 실패를 별도로 보존합니다. [gate 증거](reports/m2/gates-stage15-final-evidence.tar.gz)는 무결성·개발 계약 통과와 엄격한 선택·전체 gate 실패를 구분합니다. 요구사항 474개·부분 연결 15개·accounting row 2,513개(ready 1,513·미구현 960·미연결 40)와 전체 gate 실패 이유 1,950개는 완료율이 아닙니다.

필수 로컬 검사 7종에서 Rust 315개·Python 194개와 formatting·Clippy·Wasm·참조 pin·목록 freshness가 통과했습니다. 기존 ignored probe 1개는 일반 통과 수에서 제외합니다. 실제 Node·Chromium은 각각 54개 시나리오가 통과했고 공통 primitive 6,078개 중 Java 일치 6,076개와 기존 regex 실패 2개를 구분합니다. 로컬 검증으로 개발 단계를 마감하며 새 원격 CI 성공이나 미실행 Linux·Windows 결과를 주장하지 않습니다.

[최종 요약](reports/m2/stage15-final-summary.json)과 [소스 증거](reports/m2/source-stage15-final-evidence.tar.gz)는 소스 사본 2,091개·고유 경로 1,725개를 정확한 구현 커밋의 blob에 대조합니다. 이번 사본에는 upstream submodule 소스가 없으며 참조 버전은 `cargo xtask verify`와 고정 oracle metadata로 별도 확인했습니다. 기존 M1 frontend 15개, runtime 값 6개·전체 진단 2개 일치와 cycle 진단 1개 차이, lifecycle protocol 2개 일치와 전체 진단 4개 차이도 유지했습니다.

[증거 범위 정정](reports/m2/source-stage15-final-amendment-evidence.tar.gz)은 최초 패키징 기록의 upstream blob 검증 플래그를 바로잡고 원본 archive를 보존합니다. 확장된 목록에 옛 파일 바이트를 요구하여 중단된 첫 보조 스크립트 실행도 남겼습니다. 검증 가정을 기존 행·요구사항·연결의 보존으로 수정했으며 구현이나 Java 수집은 다시 실행하거나 바꾸지 않았습니다.

[탐색 증거](reports/m2/exploration-stage15-evidence.tar.gz)는 파일 3,995개와 최종 관찰 46개 중 41개 일치·5개 불일치를 보존합니다. `recursive-finite-rejection`, `recursive-as-in-constraint`, `recursive-this-after-is`는 Java의 공유 custom-this slot으로 인한 값·진단 차이입니다. `recursive-generic-shadow`는 매개변수화된 재귀 타입 표현의 미구현이며 `recursive-lexical-capture`는 non-const 별칭 capture를 Rust가 잘못 허용하는 차이입니다. 초기 순차 목록과 native 수명주기 검사 초안의 실패, 원래 Java 기대·출력·고정 기준선을 유지합니다. 새 개발 예외를 추가하지 않았으며 **전체 M2와 성능·메모리 목표는 계속 미완료**입니다.

**Stage 15의 제한된 개발 계약은 로컬 검증으로 마감했습니다.** 전체 릴리스의 OS·언어/API·성능 조건은 유지하며 미실행 환경과 남은 불일치를 통과로 바꾸지 않습니다.

열여섯 번째 단계의 구현 커밋 `540d81818a0c5c18fab6d6354356621dfea9f962`를 푸시했습니다. frame별 mutable custom-this slot, lambda 생성 시 값 캡처, 별칭의 지연 const 검사와 Java 반복 frame 압축을 구현했습니다. 런타임·압축은 `gpt-6-astra / high`, Java 증거·수명주기·플랫폼·독립 검증은 `gpt-6.1-sol / high`로 분담했습니다. 같은 호출의 중첩 `is`/`as`는 슬롯을 공유하며 함수·let root·lazy member·Host 재진입은 분리합니다. 내부 슬롯도 evaluator의 tracing heap에서 순환·중단 상태와 함께 회수하고 기존 heap 검사 수치를 완화하지 않았습니다.

Stage 15의 원래 입력·URI 그대로 재비교하여 shared-slot 실패 3개와 non-const alias capture 1개를 해결했으며 recursive generic은 남았습니다. 새 선택 84개를 추가해 1,597개·변형 관계 54개, 별도 순차 목록 36개를 두 번 실행하는 72회·502개 호출 계약을 구성했습니다. 반복 frame과 hint 위치는 Java의 부분 패턴·count-one loop 동작까지 구현했고 const method 오류는 전체 호출 span을 사용합니다. 부분 연결은 16개이며 요구사항 474개·미구현 960행·미연결 40행과 기존 실패는 그대로 추적합니다. [새 Java 전체 비교](reports/m2/language-stage16-final-evidence.tar.gz)는 1,595개 일치와 기존 `error-cycle` oracle 오류·`error-remainder-zero` 진단 차이만 남겼으며 변형 관계 54개가 모두 통과했습니다. [이전 선택 재비교](reports/m2/regressions-stage16-final-evidence.tar.gz)도 기존 1,511개 일치·2개 실패를 보존했습니다.

필수 로컬 검사 7종에서 Rust 338개·Python 194개와 formatting·Clippy·Wasm·참조 pin·목록 freshness가 통과했습니다. 기존 ignored probe 1개는 제외합니다. 실제 Node·Chromium은 각각 56개 시나리오가 통과했고 공통 primitive 6,078개 중 Java 일치 6,076개와 기존 regex 실패 2개를 유지했습니다. [순차 계약](reports/m2/class-lifecycle-stage16-final-evidence.tar.gz) 502개 호출은 모두 일치하며 이전 454개 호출의 Java/Rust status·stdout·stderr 사본 2,724개도 같은 바이트입니다.

[최종 요약](reports/m2/stage16-final-summary.json)과 [소스 증거](reports/m2/source-stage16-final-evidence.tar.gz)는 소스 사본 2,199개·고유 경로 1,818개를 정확한 구현 커밋에 대조합니다. 언어 수집의 소스 스냅샷은 1,698개이며 이번 source proof에 upstream submodule 소스 사본은 없습니다. 참조 pin과 oracle 출처는 별도 검사했습니다. 최초 패키징 준비 검사가 변경된 플랫폼 파일에도 옛 바이트를 요구해 중단된 기록과 수정한 검사·기존 시나리오 보존 증거도 남겼습니다. [gate 증거](reports/m2/gates-stage16-final-evidence.tar.gz)는 무결성·개발 계약 통과와 엄격한 선택·전체 gate 실패를 구분하며 전체 실패 이유 1,950개를 유지합니다.

[탐색 증거](reports/m2/exploration-stage16-evidence.tar.gz)는 파일 7,629개와 독립 관찰 94개 중 84개 일치·10개 차이, 별도 순차 탐색 82개 호출 중 69개 일치·13개 차이를 보존합니다. 단계의 선택 통과에 미선택 실패를 섞거나 새 예외를 추가하지 않았습니다. object payload boxing은 내부 슬롯 크기를 줄이기 위한 구조 변경이며 실행 속도·peak RSS 동등성은 아직 측정하지 않았습니다. **Stage 16의 제한된 개발 계약은 로컬 검증으로 마감했으며 전체 M2·API·성능·릴리스 플랫폼 조건은 계속 미완료입니다.**

탐색 중 local const read는 Java가 오류 전에 resolution node를 cache하여 재시도 결과가 달라짐을 확인했습니다. inline 별칭·let root·lambda·재귀 깊이에 따라 node/call-target 공유 범위가 달라 단일 alias/block flag로 수정하지 않았습니다. const 함수의 local read와 grouped callee 구문 진단 차이도 원시 결과를 보존합니다. 새 개발 예외를 추가하지 않습니다.

열일곱 번째 단계의 구현 커밋 `43db2bc4664ae91521204021deac8b7ce1641f3b`를 푸시했습니다. 런타임과 소유권 검토는 `gpt-6-astra / high`, Java 차등 검증·수명주기·실제 플랫폼·증거 검토는 `gpt-6.1-sol / high`로 분담했습니다. evaluator별 executable/type-use node를 도입하여 오류 전에 게시한 local const-property 해석 상태를 재시도에서 보존합니다. 독립 inline·lambda 사용, 공유 non-generic let root, 재귀 깊이, 상속 annotation 선언, lazy Listing·Mapping 원소의 node 공유를 구분하고 receiver·owner는 현재 호출에서 가져옵니다. const 메서드와 lambda의 lookup/barrier 차이, `outer`의 직전 lexical scope, grouped callee의 문맥별 parser 진단도 보완했습니다.

최종 독립 검토에서 앞선 타입 검사 실행 여부가 이후 생성되는 lambda·객체의 소유권을 바꾸는 오류를 발견했습니다. canonical owner를 환경에 암묵적으로 승격하지 않도록 수정하고 양쪽 분기 순서의 Java 순차 사례와 Host 재진입·취소·GC 회귀를 추가했습니다. 수정 전 완료한 검사들과 323개 사례에서 중단한 첫 전체 수집은 별도 보존하고 최종 증거에서 제외했습니다.

[최종 요약](reports/m2/stage17-final-summary.json)의 선택은 1,659개 독립 식·56개 변형 관계입니다. 추가 62개는 성공 31개·예상 오류 31개이며 Java와 전체 바이트가 일치합니다. 기존 1,597개 정의·입력과 54개 관계를 유지했습니다. 새 전체 Java 수집은 1,657개 일치·기존 순환 oracle 오류와 정수 `% 0` 진단 불일치 두 개를 기록하고 새 개발 예외를 추가하지 않았습니다. 별도 persistent 계약은 67개 목록을 두 번씩 실행한 134회·1,052개 호출이 모두 일치합니다. 이전 502개 호출의 원시 결과도 보존했습니다.

필수 검사 7종에서 Rust 363개·Python 194개가 통과했고 기존 ignored probe 1개는 별도입니다. 실제 Node·Chromium은 각각 61개 시나리오를 통과했습니다. 공통 primitive 6,078개의 엔진 일치와 target별 Java 일치 6,076개·기존 regex 실패 2개를 유지합니다. 내부 수명주기 검사는 기존 값 수치를 유지하면서 executable node를 별도 정확한 수치로 검사하고 terminal 상태에서 전체 heap 0을 요구합니다. [소스 증거](reports/m2/source-stage17-final-evidence.tar.gz)는 소스 사본 2,409개·고유 경로 1,946개를 구현 커밋에 대조합니다. upstream 소스 사본은 0개이며 참조 pin·oracle 출처는 별도 검증했습니다.

[탐색 증거](reports/m2/exploration-stage17-evidence.tar.gz)에서 이전 독립 관찰 94개는 84개에서 93개 일치로 개선되고 recursive generic 한 개가 남았습니다. 이전 const 재시도 82개 호출은 모두 일치합니다. 새 generic let-root 공유, const let body 범위, local typed-property frame과 일부 parser·missing-member 진단 차이는 미선택 실패로 유지합니다. 엄격한 전체 gate의 실패 이유 1,950개와 미구현 960개·미연결 40개 accounting row를 성공으로 바꾸지 않습니다. **Stage 17의 제한된 로컬 개발 계약만 마감했으며 전체 M2·제품 API·성능·메모리·릴리스 플랫폼은 계속 미완료입니다.**

열여덟 번째 단계는 유한 recursive generic을 실행 시 타입 인자 환경으로 확장했습니다. 런타임·메타데이터 검토는 `gpt-6-astra / high`, Java 차등 검증·수명주기·실제 플랫폼·증거 검토는 `gpt-6.1-sol / high`로 분담했습니다. 유한한 constraint bytecode template에 실제 인자를 전달하고 재귀 back edge는 활성 template을 재사용하여 `A<List<T>>`처럼 인자가 바뀌는 경우를 처리합니다. generic let root는 Java처럼 선언 call target을 공유하고 내부 타입 변수를 `unknown`으로 유지합니다. let·for 본문의 const barrier와 initializer·annotation·lambda·eager generator 경계를 구분합니다.

독립 검토에서 실제 인자가 정해지기 전 컬렉션 기본값을 생략하는 회귀와 `Reference<D,T>`의 constrained referent 검사를 빠뜨리는 회귀를 발견하여 수정했습니다. 원래 Java·Stage 17 성공 입력을 그대로 보존하고, 기본값은 대입 후 기존 class 선언을 사용하며 Reference 검사는 authored resolution 위치에서 수행합니다. constraint body의 short-circuit는 유지합니다. 수정 전 검사·중단된 수집과 실패 관찰은 별도 증거로 남깁니다.

구현 커밋 `e918efdf718f0bebe1b5184e10421b1b1ffaa730`를 푸시했습니다. [최종 로컬 요약](reports/m2/stage18-final-summary.json)은 1,798개 독립 식 중 1,796개 원시 바이트 일치와 기존 순환 oracle 오류·정수 `% 0` 진단 불일치를 기록합니다. 추가 139개는 성공 85개·예상 오류 54개이며 모두 일치하고, 변형 관계 63개도 통과했습니다. 기존 1,659개 정의·입력·56개 관계는 유지하며 별도 이전 선택 재비교도 1,657개 일치와 원래 두 실패를 유지했습니다. 새 개발 예외는 추가하지 않았습니다.

별도 persistent 계약은 108개 목록을 두 번 실행한 216회·1,782개 호출이 모두 일치합니다. 기존 67개 목록과 Java/Rust 원시 결과 파일 6,312개를 바이트 그대로 보존했습니다. [필수 로컬 검사 7종](reports/m2/validation-stage18-final.json)이 통과했고 Rust 382개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. 실제 Node·Chromium 각각 68개 개발 시나리오가 통과했으며 공통 primitive 6,078개의 엔진 일치와 target별 Java 일치 6,076개·기존 regex 실패 2개를 유지합니다. Host 중단·재진입·GC·취소·서로 다른 evaluator의 분리·terminal 해제는 기존 값 수치와 별도 executable 수치로 검증했습니다.

[탐색 증거](reports/m2/exploration-stage18-evidence.tar.gz)와 연결된 지원 archive 3개는 수정 전 실패·중단된 수집을 포함하여 원본 경로 74,806개를 고유 내용 10,671개로 보존하며 모든 바이트·크기·mode 복원을 검증했습니다. 더 넓은 탐색의 독립 관찰 166개 중 25개 불일치와 중복되는 호출 관찰 598개 중 69개 불일치는 선택 계약과 분리합니다. [소스 대조와 gate 증거](reports/m2/source-stage18-final-evidence.tar.gz)는 소스 사본 4,996개·고유 경로 2,244개를 정확한 구현 커밋과 연결합니다. upstream 소스 사본은 0개이며 참조 pin·oracle 출처는 별도 검증했습니다. 무결성·개발 gate는 통과하고 엄격한 전체 gate는 기존 실패 이유 1,950개를 유지합니다. 요구사항 474개·부분 연결표 16개와 미구현 960개·미연결 40개 accounting row를 완료율로 계산하지 않습니다.

`this is A<List<T>>`의 무한 타입 인자 증가는 native stack을 넘칠 수 있으며 실제 SIGABRT를 보존합니다. 제한된 yield/cancel 검사로 이 실패를 성공 처리하지 않습니다. **Stage 18의 제한된 로컬 개발 계약만 마감했으며 전체 M2와 성능·메모리·제품 API·릴리스 플랫폼 조건은 미완료입니다.**

열아홉 번째 단계는 recursive generic의 타입 인자를 불변 공유 그래프로 바꾸었습니다. 런타임 구현과 교차 검토는 `gpt-6-astra / high`, Java 비교·수명주기·실제 플랫폼·증거 검토는 `gpt-6.1-sol / high`로 분담했습니다. 이미 대입한 인자를 다시 깊게 복사하지 않고, 마지막 소유자의 해제는 명시적 작업 목록으로 수행합니다. `Arc::into_inner`로 동시에 마지막 참조를 해제해도 한 소유자가 회수하도록 했습니다. 깊은 타입 비교·진단·Reference 정규화와 metadata identity를 반복 순회로 바꾸고, nested collection 기본값의 하강·생성·복귀는 VM fuel을 소비하며 임시 객체를 GC root로 보존합니다.

초기 검증에서 256 KiB 스택의 20,000단계 공유 그래프·동시 해제와 2,048단계 Reference metadata, 768단계 evaluator 상태·캡처·실패 재시도·GC, Host 재진입·terminal 해제를 확인했습니다. 원래 무한 확장 입력은 150,000개 VM step씩 세 차례 실행 후 취소·재사용하고, 실행 중 VM을 해제하는 검사도 통과했습니다. 기존 1,798개 재비교는 원래 두 실패를 유지합니다. 독립 검토가 찾은 `Domain` 동일 class id의 shortcut 회귀를 수정했습니다. Java 증거가 없는 wrapper actual-argument 동등성 변경은 포함하지 않았습니다.

구현 커밋 `a0801b9b4d614f6d936d9c410f67a05be59e7733`를 푸시했습니다. [최종 로컬 요약](reports/m2/stage19-final-summary.json)은 1,821개 독립 식 중 1,819개 원시 바이트 일치와 기존 순환 oracle 오류·정수 `% 0` 진단 불일치를 기록합니다. 추가 23개는 성공 19개·예상 오류 4개이며 모두 일치하고 변형 관계 66개도 통과했습니다. 기존 1,798개 정의·입력·63개 관계는 그대로 보존하며 새 개발 예외는 추가하지 않았습니다.

별도 persistent 계약은 115개 목록을 두 번 실행한 230회·1,870개 호출이 모두 일치합니다. 기존 108개 목록과 1,782개 호출, 과거 수명주기 검사 파일 16개를 유지했습니다. [필수 로컬 검사 7종](reports/m2/validation-stage19-final.json)이 통과했고 Rust 392개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. 자식 프로세스의 재실행은 검사 수에 중복 계산하지 않습니다. 실제 Node·Chromium 각각 71개 개발 시나리오가 통과했고, 공통 primitive 6,078개의 엔진 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 유지했습니다.

[탐색 증거](reports/m2/exploration-stage19-evidence.tar.gz)와 지원 archive 1개는 원본 경로 13,900개를 고유 내용 5,906개로 보존하며 바이트·크기·mode 복원을 검증했습니다. 기존 Stage 18 archive는 해시로 연결하고 내용을 중복 저장하지 않았습니다. 이전 관찰 166개의 Rust 결과와 비일치 25개는 그대로 유지하며, 그중 Java 내부 오류 한 개의 oracle-error 분류를 명확히 했습니다. 별도 집중 탐색 77개의 일치 66개·불일치 10개·oracle 오류 1개도 선택 계약과 구분합니다. [소스·gate 증거](reports/m2/source-stage19-final-evidence.tar.gz)는 소스 사본 5,059개·고유 경로 2,284개를 정확한 구현 커밋과 연결합니다. upstream 소스 사본은 0개이며 참조 pin·oracle 출처는 별도 검증했습니다. 무결성·개발 gate는 통과하고 엄격한 전체 gate의 실패 이유 1,950개를 유지합니다.

원래 125바이트 입력의 SIGABRT와 수정 전 실행은 역사적 증거로 보존합니다. 기존 probe의 instruction 예산 종료와 작은 스택의 안전한 취소는 Java의 stack-overflow 오류 바이트 일치를 의미하지 않습니다. 앞의 VM step 수에는 타입·기본값 scheduler 작업도 포함됩니다. Reference 정규화·검증 등 한 instruction 안의 작업량과 진단 문자열 확장, authored compiler nesting, 전체 호환성·실행 시간·peak RSS 목표는 여전히 별도 검증 대상입니다. **Stage 19의 제한된 로컬 개발 계약만 마감했으며 M2 전체·제품 API·성능·메모리·릴리스 플랫폼 조건은 미완료입니다.**

스무 번째 단계는 nullable 타입의 내부 기본값 보존을 구현합니다. Java의 `VmNull`과 nullable 타입·객체 literal 소스를 기준으로, 일반 `null`과 같은 출력·타입·동등성·해시를 유지하면서 amendment에 필요한 기본값을 evaluator 소유 heap entry로 보존합니다. 빈 amendment는 내부 기본값 한 단계를 반환하고, 멤버가 있는 amendment는 중첩 기본값을 풀어 부모를 선택합니다. null-propagation은 기본값을 제거한 일반 `null`을 반환합니다. generic collection의 기본값 생성은 기존 continuation·VM fuel·GC root를 사용합니다.

핵심 구현은 `gpt-6-astra / high`, Java 비교와 독립 수명주기·플랫폼 검증은 `gpt-6.1-sol / high`로 분담했습니다. 원본 nullable 실패 입력과 새 성공·오류 사례, persistent 호출, 캡처된 closure, Host 실패·재시도·취소·evaluator 분리·terminal 해제를 검증합니다. 독립 검토에서 찾은 Host 입력·응답의 내부 null slot 검증 문제도 회귀 검사에 포함합니다. 첫 전체 검사에서는 기존 256 KiB 스택 테스트가 실패했습니다. 빈 amendment가 큰 instruction dispatcher를 재귀 호출하던 경로를 제거했고 기존 조건 그대로 재검사를 통과했습니다. 최초 검토가 놓친 이 회귀와 원시 실패·중단 기록을 보존하며 수정 후 새 디렉터리에서 최종 검증합니다. 직접 `new T`, shared-let·Reference·일부 기본값 진단과 동기 metadata의 취소 지연은 계속 별도 과제입니다.

구현 커밋 `c4e2a41e8391fe5ef031fc6fd5880cf46112c66e`를 푸시했습니다. [최종 로컬 요약](reports/m2/stage20-final-summary.json)은 1,860개 독립 식 중 1,858개 원시 바이트 일치와 기존 순환 oracle 오류·정수 `% 0` 진단 불일치를 기록합니다. 새 39개는 성공 34개·예상 오류 5개이며 모두 일치하고 변형 관계 69개도 통과했습니다. 기존 1,821개 정의·66개 관계·115개 순차 목록과 수명주기 테스트 파일 17개를 보존했으며 새 개발 예외는 추가하지 않았습니다.

별도 persistent 계약은 118개 목록을 두 번 실행한 236회·1,936개 호출이 모두 일치합니다. [필수 로컬 검사 7종](reports/m2/validation-stage20-final.json)이 통과했고 Rust 400개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. 새 heap 테스트 2개는 256 KiB 스택에서 20,000단계 nullable 연결의 tracing·해제를 검증합니다. 새 수명주기 테스트 6개는 기본 thread stack에서 캡처된 closure·Host 재진입·실패 재시도·24회 취소·외부/해제된 handle·evaluator 분리·terminal 해제를 검증합니다. 실제 Node·Chromium 각각 74개 개발 시나리오와 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 구분합니다.

[탐색 증거](reports/m2/exploration-stage20-evidence.tar.gz)와 [수정 전 증거](reports/m2/exploration-stage20-superseded-evidence.tar.gz)는 원본 경로 16,744개와 명시적으로 제외한 Cargo cache 파일 178개를 기록합니다. 독립 검토가 archive payload 16,752개의 무결성과 원본 경로·바이트·크기 및 과거 증거 연결을 확인했습니다. 수정된 최종 artifact로 새 39개·과거 원본 URI 6개·새 순차 호출 33개를 별도로 재비교했습니다. 전체 Python 검사와 별개인 보조 수명주기 도구 검사 한 건의 timeout, invalid syntax 가설, 직접 `new T`, Map constructor, abstract 기본값의 missing-property 진단 차이를 보존합니다.

[소스·gate 증거](reports/m2/source-stage20-final-evidence.tar.gz)는 소스 사본 4,951개·고유 경로 2,331개를 정확한 구현 커밋과 연결합니다. upstream 소스 사본은 0개이며 참조 pin·oracle 출처는 별도 검증했습니다. 요구사항 474개·부분 연결 16개와 2,860개 accounting row 중 ready 1,860개·미구현 960개·미연결 40개를 유지합니다. 엄격한 전체 gate는 기존 실패 이유 1,950개를 유지하며 목록 수치를 완료율로 계산하지 않습니다. **Stage 20의 제한된 nullable-default 개발 계약만 마감했으며 M2 전체·제품 API·성능·메모리·릴리스 플랫폼 조건은 미완료입니다.**

스물한 번째 단계는 generic constraint 안의 직접 `new T`가 실제 타입 인자의 기본값으로 부모를 선택하도록 구현합니다. 기존 VM 기본값 continuation·fuel·GC root와 amendment 경로를 재사용하며, bare `new T`와 명시적 `new Alias<T>`의 결과 타입 검사 차이를 Java 소스·원시 비교로 구분합니다. 별칭의 작성 위치와 원래 표기를 타입 identity와 분리해 보존하고, 인자 전달·재귀·실패 재시도에도 오류 위치를 유지합니다. inline·lambda와 지연 멤버 call target의 타입 인자 전달 경계도 구분합니다.

핵심 구현은 `gpt-6-astra / high`, Java 비교 및 독립 수명주기·플랫폼 검증은 `gpt-6.1-sol / high`로 분담했습니다. 새 독립 식 80개는 성공 39개·예상 오류 41개이며, 기존 목록을 보존한 전체 선택은 1,940개 식·72개 변형 관계입니다. 별도 persistent 계약은 122개 목록·2,026개 호출입니다. 새 native 검사 6개는 Host 실패·재진입·재시도, 24회 취소, 실제 타입이 다른 evaluator, 반환 closure, terminal 해제와 256 KiB evaluator stack의 768단계 기본값 생성을 다룹니다. 구현 커밋 `6530fc7066779781485e1e74dae47fff9a5ac819`를 푸시했습니다.

[최종 로컬 요약](reports/m2/stage21-final-summary.json)은 1,940개 식 중 1,938개 원시 바이트 일치와 기존 순환 oracle 오류·정수 `% 0` 진단 차이를 기록합니다. 새 80개와 변형 관계 72개, 122개 순차 목록을 두 번 실행한 2,026개 호출이 모두 일치합니다. 기존 1,860개 정의·69개 관계·118개 목록·수명주기 파일 18개를 보존했으며 새 개발 예외는 추가하지 않았습니다. [필수 로컬 검사 7종](reports/m2/validation-stage21-final.json)이 통과했고 Rust 406개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. 실제 Node·Chromium 각각 76개 개발 시나리오와 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 유지합니다.

[탐색 증거](reports/m2/exploration-stage21-evidence.tar.gz)는 원본 경로 5,806개·inventory payload 5,810개를 제외 없이 보존합니다. 최종 artifact로 새 80개·원래 Stage 20 URI의 실패 입력 4개·재귀 제어 사례 3개·순차 호출 45개를 재비교했습니다. 더 넓은 탐색 85개 중 일치 71개·불일치 14개는 선택 계약과 별도이며, builtin·missing-property·generator·Listing key·지연 멤버 frame 이름 차이를 유지합니다. 동시 수정 중 실패한 준비 build와 원시 출력이 별도 기록되지 않은 초기 테스트 실패도 최종 통과 증거와 구분합니다.

[소스·gate 증거](reports/m2/source-stage21-final-evidence.tar.gz)는 소스 사본 5,133개·고유 경로 2,421개를 정확한 구현 커밋과 연결합니다. 참조 pin·oracle 출처는 별도 검증하며 upstream 소스 사본은 0개입니다. 요구사항 474개·부분 연결 16개와 accounting row 2,940개 중 ready 1,940개·미구현 960개·미연결 40개를 완료율로 계산하지 않습니다. 엄격한 전체 gate는 기존 실패 이유 1,950개를 유지합니다. **Stage 21의 제한된 generic-constructor 개발 계약만 마감했으며 M2 전체·제품 API·성능·메모리·릴리스 플랫폼 조건은 미완료입니다.**

스물두 번째 단계는 strict `Map`의 생성·조회·기본값·타입 검사·순회·동등성·해시와 강제 평가 없는 문자열 출력을 구현했습니다. 핵심 런타임은 `gpt-6-astra / xhigh`, Java 차등 비교와 독립 수명주기·플랫폼 검증은 각각 `gpt-6.1-sol / high`로 분담했습니다. root는 Map 진단·expression-string adapter와 나머지 런타임의 독립 소스 검토·최종 로컬 검증을 담당했습니다. 구현 커밋 `bb30657aecc0be64cdbf1510e949053794b1ca8d`를 푸시했습니다.

각 key→value 평가 직후 해시·삽입을 수행하고, 중복 키는 최초 위치를 유지하며 값을 교체합니다. 기존 Mapping의 EconomicMap 선언 index와 분리해 Paguro 해시 트리의 탐색 경로를 모델링합니다. 삽입 순서와 내부 해시 순서, boxed 숫자 키, nested Map·lazy wrapper·오류 후 재시도를 원시 Java 비교로 검증합니다. unpublished builder·key/value와 lookup·comparison continuation은 Host 재진입·GC·실패·취소·terminal 해제까지 함께 검증합니다. 전체 Map API·Set·일반 compound 출력·JVM identity hash의 순서·성능 및 메모리 동등성은 별도 과제로 유지합니다.

독립 검토에서 추가한 256 KiB·768단계 중첩 Map 검사에 한 차례 스택 회귀가 나타났습니다. optional Map index를 boxed 저장으로 바꾸고 큰 dispatcher의 Map 작업을 별도 helper로 분리해 원래 조건을 유지한 재검사를 통과했습니다. 또한 구조적으로 같은 키가 cache 순서에 따라 서로 다른 해시를 가지면 Paguro의 collision 조회와 삽입이 다르게 동작해 내부 table 항목이 삽입 순서 목록보다 많아질 수 있음을 Java로 확인했습니다. 두 목록을 분리하고 목록에 없는 heap 값의 GC·Host 수명주기 검사와 새 원시 회귀 사례 5개를 추가했습니다. 수정 전 실패·초기 수집 한계는 최종 통과 증거와 구분해 보존합니다.

첫 전체 최종 검사에서는 기존 generic metadata의 256 KiB 스택 검사와 Node의 23개 List 키 Mapping 시나리오가 추가로 실패했습니다. 집중 검사에서 기존 generic metadata 파일을 빠뜨린 점과 새 코드의 회귀를 기록합니다. 작은 scheduler에서 collection·semantic continuation을 직접 실행하도록 분리하고, 한 bytecode의 여러 hash 준비 단계가 서로 재시도를 반복하지 않도록 완료된 키를 모아 보존한 뒤 명령 완료 시 지웁니다. 기존 stack·depth·timeout·instruction 예산을 유지한 집중 검사 25개와 원래 23개 키 입력이 통과했습니다. 최초 최종 수집은 superseded 증거로 보존하고, 수정 후 새 `final-02`에서 전체 필수 검사·플랫폼 검증을 먼저 통과한 뒤 Java 전체 비교를 다시 수집했습니다.

[최종 로컬 요약](reports/m2/stage22-final-summary.json)은 2,131개 독립 식 중 2,129개 원시 바이트 일치와 기존 순환 oracle 오류·정수 `% 0` 진단 차이를 기록합니다. 새 191개는 성공 108개·예상 오류 83개이며 모두 일치했습니다. 변형 관계 76개와 129개 순차 목록을 두 번 실행한 2,166개 호출도 모두 통과했습니다. 이전 1,940개 정의·72개 관계·122개 목록·수명주기 파일 19개·플랫폼 시나리오 76개를 보존했고 새 개발 예외는 추가하지 않았습니다.

[필수 로컬 검사 7종](reports/m2/validation-stage22-final.json)은 Rust 415개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. [native 검사 9개](reports/m2/native-map-stage22-final-evidence.tar.gz)는 builder·query·comparison 중단, Host 실패·재시도, 24회 취소, evaluator 분리·stale handle·terminal 해제와 내부 table에만 있는 값의 GC 보존을 다룹니다. 8개는 일반 테스트 스택을 사용하며, 별도 child process 검사는 256 KiB evaluator thread에서 768단계 Map 해시를 실행하고 부모의 60초 제한을 유지합니다. 컴파일은 작은 스택 제한 밖에서 수행합니다. 실제 Node·Chromium 각각 78개 개발 시나리오가 통과했고, 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 유지합니다. Java와 동일한 자원 제한이나 실행 시간·peak RSS를 달성했다는 주장은 하지 않습니다.

[탐색 증거](reports/m2/exploration-stage22-evidence.tar.gz)와 두 superseded 묶음은 원본 경로 39,437개·inventory record 39,449개를 제외 없이 보존합니다. 최종 artifact로 새 191개 원래 탐색 URI와 순차 호출 70개를 재비교했습니다. 별도 201개 탐색은 일치 186개·불일치 15개를, 추가 12개 관찰은 일치 9개·Java 내부 오류 3개를 유지합니다. 이 관찰은 선택 계약과 중복되며 추가 성공 분모로 계산하지 않습니다. 초기 컴파일 오류의 원시 기록 부재와 중단된 첫 수집의 outer process 기록 부재도 명시합니다. 80 MB를 넘은 최초 압축 시도는 원시 자료를 세 묶음으로 나누고 실패 이력을 보존했습니다. 최종 포장 검사기의 기존 변형 관계 prefix 가정도 수정했습니다. 기존 72개 전체 기록과 상대 순서는 그대로이며, 중복·누락·변경을 거부하는 독립 검토와 실패한 검사기의 원본 실행 기록을 남겼습니다.

[소스·gate 증거](reports/m2/source-stage22-final-evidence.tar.gz)는 소스 사본 5,557개·고유 경로 2,629개를 정확한 구현 커밋과 연결합니다. upstream 소스 사본은 0개이며 참조 pin과 oracle 출처는 별도로 확인했습니다. 요구사항 474개·부분 연결 16개와 accounting row 3,131개 중 ready 2,131개·미구현 960개·미연결 40개를 유지합니다. 엄격한 전체 gate는 기존 실패 이유 1,950개를 유지하며 이 수치를 완료율로 계산하지 않습니다. [독립 최종 감사](reports/m2/stage22-post-seal-audit.json)는 최종 archive 12개·소스·리뷰·gate·실제 포장 검사기와 역사적 증거를 확인합니다. **Stage 22의 제한된 strict-Map 개발 계약만 마감했으며 M2 전체·제품 API·성능·메모리·릴리스 플랫폼 조건은 미완료입니다.**

스물세 번째 단계는 strict `Set`의 생성·중복 제거·조회·기본값·타입 검사·동등성·해시·순회·spread·문자열 출력을 구현했습니다. 핵심 런타임은 `gpt-6-astra / xhigh`, Java 차등 검증과 독립 수명주기·플랫폼 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. root는 공유 multiline renderer·adapter, 독립 소스 검토와 최종 통합을 담당했습니다. 구현 `84d7044e7b9b83350b895a3b91a92663aedb73df`를 커밋·푸시했습니다. 원소별 평가→contains→필요한 경우 put 순서를 분리하고, 내부 table과 삽입 순서를 각각 보존·추적합니다. 진단과 직접 Map 출력의 한 줄 형식은 유지하며, Set의 중첩 객체 표현은 multiline 형식을 따릅니다.

[최종 로컬 요약](reports/m2/stage23-final-summary.json)은 2,348개 독립 식 중 2,346개 원시 바이트 일치와 기존 순환 oracle 오류·정수 `% 0` 진단 차이를 기록합니다. 새 217개(성공 158개·예상 오류 59개)와 변형 관계 82개가 모두 통과했습니다. 순차 목록 137개를 두 번 실행한 2,326개 호출도 모두 일치했습니다. 이전 2,131개 입력·76개 관계·129개 목록·수명주기 파일 20개·플랫폼 시나리오 78개는 그대로 보존했고 새 개발 예외는 추가하지 않았습니다.

[필수 로컬 검사 7종](reports/m2/validation-stage23-final.json)은 Rust 424개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. [native 검사 8개](reports/m2/native-set-stage23-final-evidence.tar.gz)는 builder·membership query·비교 피연산자의 보존, Host 실패·재시도·재진입, 24회 취소, evaluator 분리·stale handle·terminal 해제와 선언·캐시의 반복 호출 plateau를 검증합니다. 7개 일반 사례는 매 yield마다 GC를 실행합니다. 별도의 깊이 768 Set 검사는 256 KiB evaluator stack·60초 parent deadline·poll당 instruction 1개·128 poll마다 GC·이벤트당 최대 4,000,000 poll 조건에서 901,299 poll과 주기적/대기 GC 7,042회·마지막 rooted GC 1회를 기록했습니다. 원래 매 yield GC의 시간 초과와 이후 이벤트당 200,000 poll 제한의 실패를 보존하며, 다른 조건의 통과를 같은 자원 계약의 통과로 처리하지 않습니다. 이전 Map·generic 작은 스택 검사는 입력·조건을 바꾸지 않았습니다.

실제 Node·Chromium 각각 80개 시나리오가 통과했고, 새 Set 두 그룹은 target마다 합계 160개 순차 호출을 Java 원시 payload와 비교했습니다. 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 유지합니다. 기록된 브라우저 경로는 full Chrome이며 headless-shell 선택은 고정 Playwright 소스에서 추론한 것으로, 그 경로 해시를 실제 실행 binary의 직접 증명으로 표시하지 않습니다. 실행 중 관찰한 브라우저 버전·시나리오 결과와 구분합니다.

[탐색](reports/m2/exploration-stage23-evidence.tar.gz)·[중간 후보](reports/m2/exploration-stage23-superseded-evidence.tar.gz)·[native 준비 기록](reports/m2/exploration-stage23-native-preliminary-evidence.tar.gz)은 56,234개 원본 경로를 제외 없이 보존합니다. 최종 탐색 246개 중 224개 일치·22개 불일치, 별도 alias/Host 관찰 3개 중 1개 일치·2개 불일치를 유지합니다. 이 246개는 새 선택 217개와 겹칩니다. 일반 compound 반환의 원래 실패 입력 6개와 이전 순회 목록은 미선택 관찰로 보존하고, 새 source·URI의 스칼라 관찰로 순서·중복·강제된 원소의 진단을 검증했습니다. 최초 native 준비 중 source drift와 parse·alias scope·retained-cache 단언 실패도 원래 상태로 보존합니다.

[소스·gate 증거](reports/m2/source-stage23-final-evidence.tar.gz)는 소스 사본 6,053개·고유 경로 2,873개를 정확한 구현 커밋에 연결합니다. 요구사항 474개·부분 연결 16개, accounting row 3,348개 중 ready 2,348개·미구현 960개·미연결 40개를 유지하며 완료율로 계산하지 않습니다. strict subset/full gate는 실패하고 전체 gate의 실패 이유 1,950개를 유지합니다. [독립 최종 감사](reports/m2/stage23-post-seal-audit.json)는 archive 12개·총 108,225,352 bytes와 소스·Git·리뷰·gate·실제 포장 helper를 확인했습니다. **Stage 23의 제한된 strict-Set 개발 계약만 마감했으며 M2 전체·제품 API·성능·메모리·릴리스 플랫폼 조건은 미완료입니다.**

스물네 번째 단계는 `Collection<T>`의 제한된 타입 의미론을 구현했습니다. 핵심 런타임은 `gpt-6-astra / xhigh`, Java 차등 코퍼스와 독립 수명주기·플랫폼 검증은 각각 `gpt-6.1-sol / high`에 배정했습니다. root는 구현·테스트 독립 검토와 통합·로컬 검증을 담당했습니다. 구현 `123f41660286787bc46418d6a81d984f48ba22ef`를 커밋·푸시했습니다. List·Set을 받는 추상 타입으로 모델링하고 List의 중첩 지연 wrapper와 Set의 즉시 검사를 유지합니다. 빈 List 기본값, 별칭·nullable attached default·Collection을 선택한 union 기본값·Reference metadata를 연결했습니다. 빈 명시적 타입 인자 `<>`의 공통 parser 진단도 고쳤으며 기존 깊이·노드 제한은 바꾸지 않았습니다.

최종 로컬 수집에서 독립 식 2,652개 중 2,650개가 Java 원시 바이트와 일치했습니다. 기존 순환 oracle 오류와 정수 `% 0` 진단 차이를 유지하며, 새 304개(성공 196개·예상 오류 108개)와 변형 관계 88개가 모두 통과했습니다. 순차 목록 147개를 두 번 실행한 2,532개 호출도 모두 일치했습니다. 이전 2,348개 입력·82개 관계·137개 순차 목록·21개 수명주기 파일·80개 플랫폼 시나리오를 보존했습니다. root는 기존 native 테스트 파일 51개와 내부 metadata 테스트 본문 바이트도 그대로임을 확인했습니다. 새 개발 예외는 추가하지 않았습니다.

[필수 로컬 검사 7종](reports/m2/validation-stage24-final.json)은 Rust 432개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. 새 native 검사 7개는 지연·즉시 검사 시점, 중단된 constraint 문맥, 임시 receiver 보존, Host 재진입·실패 재시도, 24회 취소, evaluator 분리·stale handle·terminal 해제를 검증합니다. poll당 instruction 1개·이벤트당 최대 200,000 poll·매 yield와 pending read에서 GC 조건입니다. 별도 20,000단계 metadata 검사는 256 KiB stack에서 순회·정규화·subtype·출력 이름·해제를 다룹니다. 깊게 작성한 parser 입력이나 Java 속도·peak RSS 동등성을 대신하지 않으며 기존 Map·Set·generic 작은 스택 검사의 조건은 유지했습니다.

실제 Node·Chromium 각각 82개 개발 시나리오가 통과했습니다. 새 두 그룹은 target마다 206개 순차 호출을 Java 전체 payload와 비교합니다. 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 유지합니다. 기록된 full Chrome 경로와 고정 Playwright 코드로 추론한 headless-shell 선택을 구분하며, 실제 실행 binary의 직접 증명으로 확대하지 않습니다. M1의 기존 성공·순환 오류·전체 진단 차이도 유지합니다. 새 hosted CI·Linux·Windows 실행은 주장하지 않습니다.

최종 artifact로 새 304개 원래 탐색 URI와 순차 호출 103개를 재비교해 모두 일치했습니다. 별도 넓은 탐색 363개는 일치 337개·불일치 26개를 유지하며, 새 선택 304개와 미선택 대조군 59개를 포함하므로 추가 성공 분모로 합산하지 않습니다. String을 기본값으로 선택한 별칭 union 생성자의 진단 차이 2개, 미지원 API·compound 출력과 잘못 작성한 입력·수정된 별도 입력을 각 원본으로 보존합니다. 같은 함수·다른 함수 wrapper 관찰은 값·오류를 검증하며 wrapper identity나 성공 predicate 호출 횟수까지 입증하지 않습니다. 초기 탐색 빌드는 artifact·입력·결과 식별자를 보존하지만 전체 build/source closure가 없던 한계를 명시하고, 최종 기록 빌드와 별도로 취급합니다. 초기 receiver/alias 단언 실패와 metadata 테스트 컴파일 오류도 보존합니다.

[최종 요약](reports/m2/stage24-final-summary.json)은 archive 16개·총 87,514,114 bytes를 기록합니다. [소스·gate 증거](reports/m2/source-stage24-final-evidence.tar.gz)는 소스 사본 6,707개·고유 프로젝트 경로 3,199개를 정확한 구현 커밋에 연결합니다. 탐색 archive 7개는 원본 경로 19,929개를 제외 없이 보존하며 partition별 metadata를 포함한 inventory는 19,957개입니다. [독립 최종 감사](reports/m2/stage24-post-seal-audit.json)는 archive inventory·Git 소스·리뷰·실제 포장 helper와 process·gate·역사적 artifact 식별자를 확인해 통과했습니다. 보존된 evaluator를 실행하지 않았습니다.

[완료 process 사본](reports/m2/stage24-completion-captures.json)은 archive 생성 뒤 완료된 포장·감사의 runner·invocation·process·stdout·stderr·helper 원본 12개를 바이트 그대로 보존합니다. 봉인된 archive 16개와 별도 기록입니다.

전체 gate의 실패 이유 1,950개, 요구사항 474개·부분 연결 16개와 accounting row 3,652개 중 ready 2,652개·미구현 960개·미연결 40개를 유지하며 완료율로 계산하지 않습니다. **Stage 24의 제한된 Collection 개발 계약을 마감했으며 M2 전체·제품 API·성능·메모리·릴리스 플랫폼 조건은 계속 미완료입니다.**

스물다섯 번째 단계는 깊게 작성된 구문의 제한된 프런트엔드 스택 안전성을 구현하고 로컬 검증·독립 감사를 완료했습니다. 핵심 파서·컴파일러·AST 구현은 `gpt-6-astra / xhigh`, Java 차등 증거와 resolver·작은 스택·플랫폼 작업은 각각 `gpt-6.1-sol / high`에 배정했습니다. root가 타입 파서와 통합을 맡고 각 작성자가 자기 변경을 제외해 교차 리뷰했습니다. 일반 식과 if·let·lambda·property-only 객체 경로를 명시적 작업 스택으로 연결하고, AST 복사·해제·상수 List 키·power assertion 순회를 바꿨습니다. 표현식 작업 예산 65,536은 interpolation과 공유하며, 범위와 타입의 논리 깊이·조합 높이 제한 32는 별도로 유지합니다.

최종 native 검사 5개는 256 KiB stack·검사당 60초에서 통과했습니다. 일반 식 깊이 768·독립 property 1,024개, if·let·lambda·Dynamic property-only 객체 깊이 24/31, 부분 실패의 해제·Host 재진입·재시도·취소·terminal 해제를 다룹니다. 타입 깊이 24/31은 parse→clone→drop만 검증합니다. 기존 깊이 64 Set 비교·Map key 입력을 포함한 새 선택 36개는 별도 Java 비교에서 같은 결과를 냈으며, 전체 선택은 2,688개·변형 관계 88개·순차 목록 150개입니다. Node·Chromium의 개발 시나리오 84개와 공통 primitive 결과 6,078개가 일치했고 기존 regex 차이 2개는 유지했습니다. 구현 커밋 `fc45bea329c4fdbc536eaef5070a0c03ed642d5a`를 origin/main에 푸시했습니다. 전체 단일 식 2,688개 중 2,686개 일치·기존 cycle oracle-error 1개·정수 `% 0` 진단 차이 1개, 변형 관계 88개 모두 일치, 순차 호출 2,592개 모두 일치를 확인했습니다. 필수 로컬 검사 7종은 Rust 고유 테스트 437개·Python 194개 성공과 기존 ignored probe 1개를 구분합니다. Rust 출력의 442회 성공에는 새 native child 5개의 부모/자식 중복이 포함되어 별도로 뺐습니다. 전체 gate의 실패 이유 1,950개와 요구사항 474개·ready 2,688개/미구현 960개/미연결 40개를 유지합니다. 새 입력으로 shared-engine family가 partial로 파생되지만 완료나 새 reviewed mapping으로 계산하지 않습니다. [최종 요약](reports/m2/stage25-final-summary.json)과 [독립 감사](reports/m2/stage25-post-seal-audit.json)로 이 제한된 단계를 마감했습니다.

중첩 lexical string 깊이 64는 같은 256 KiB stack에서 SIGABRT가 발생했으며 원본 실패·제외 결정을 보존합니다. 스택을 늘린 성공으로 대체하지 않았습니다. 깊은 타입의 전체 컴파일, 클래스·instance·혼합 collection·generator 컴파일, AST Debug/PartialEq는 전체 스택 안전성을 입증하지 않았습니다. 기존 테스트의 괄호 129개·멤버 600개 거부 단언은 같은 입력의 값·전체 span·멤버 순서를 검증하는 성공 단언으로 바꾸고 원본과 수정 이유를 남겼습니다. 독립 리뷰에서 찾은 Uninitialized resolver 누락은 수정과 회귀 검사로 해결했습니다. 앞선 native 실패·테스트 컴파일 실패·소스 변경을 가로지른 불확정 실행은 별도로 보존하며 어떤 것도 성공 분모에 포함하지 않습니다.

최종 archive 15개·73,849,365 bytes와 소스 사본 6,813개·고유 프로젝트 파일 3,245개를 정확한 구현 커밋에 연결했습니다. 탐색 archive 6개는 원본 경로 32,289개와 partition metadata를 포함한 inventory 32,313개를 보존합니다. 명시한 Cargo build cache 2개는 내부를 열거하지 않고 제외했으므로 payload 수·크기·해시를 주장하지 않으며, 열거된 일반 파일의 제외는 없습니다. 최종 suite 8개는 별도 archive로 보존하고 기존 증거는 정확한 해시 의존성으로 유지합니다. 탐색 설정·봉인의 완료된 외부 process 기록은 `target/m2`에 남고 설정·sealer·index·독립 감사는 봉인 증거에 포함됩니다. 독립 최종 감사는 archive inventory·Git 소스·리뷰·helper·gate·새 36개와 기존 실패 2개의 원시 결과·테스트 집계를 확인했으며 보존된 evaluator나 helper를 실행하지 않았습니다. [완료 process 사본](reports/m2/stage25-completion-captures.json)은 포장·감사 완료 후의 원본 13개를 별도로 보존합니다. **Stage 25 개발 계약은 마감했으며 M2 전체·제품 API·릴리스 플랫폼·속도·peak RSS 목표는 계속 미완료입니다.**

스물여섯 번째 단계는 중첩 lexical string과 interpolation의 lexer·parser 호출을 명시적 continuation으로 바꿨습니다. 핵심 구현은 `gpt-6-astra / xhigh`, Java 차등 사례와 native·순차·플랫폼 수명주기 검증은 각각 `gpt-6.1-sol / high`에 배정했고 독립 마감 검토를 수행했습니다. 구현 `51b132bba82587e3cb013c46f5c452c22fa01ccf`를 커밋·푸시했습니다. 문자열 오류 cache는 원래 입력 끝과 소비 범위를 보존하고 짧아진 interpolation 범위에서 안전하게 재사용합니다. 비교한 경로에서는 앞선 구문 오류가 뒤의 escape·comment·EOF·다중 행 오류보다 먼저 보고됩니다.

기존 256 KiB stack·검사당 60초 조건에서 허용 깊이 24/31의 parse·clone·resolve·compile·drop, 오류 후 부분 트리 해제, 공유 65,536 작업 예산과 Host 중단·재진입·실패 재시도·취소·terminal 해제를 검증했습니다. 원래 깊이 64 입력은 중단 없이 기존 한도 진단을 반환하지만 Java는 이를 허용하므로 호환성 불일치는 남습니다. 깊이 한도 32는 유지합니다. 잘못된 다중 행 입력 6개와 추가 escape 입력 2개의 작은 스택 재검사는 안전한 거부를 확인하며 Java 진단 바이트 일치를 뜻하지 않습니다. 보조 lexical 오류 검증의 재파싱은 일정한 작업량·지연을 보장하지 않습니다.

[최종 요약](reports/m2/stage26-final-summary.json)은 독립 식 2,724개 중 원시 바이트 일치 2,722개와 기존 순환 oracle 오류·정수 `% 0` 진단 차이를 기록합니다. 새 36개는 성공 23개·예상 오류 13개이며 모두 일치했고, 변형 관계 88개와 151개 목록을 두 번 실행한 순차 호출 2,622개도 통과했습니다. 탐색 62개는 일치 37개·불일치 23개·oracle 오류 2개를 유지하며 선택 사례와 중복됩니다. 최종 실행 파일로 원래 탐색 URI의 새 36개와 새 순차 호출 30개를 다시 확인했습니다. [필수 로컬 검사 7종](reports/m2/validation-stage26-final.json)은 Rust 고유 441개·Python 194개 성공과 기존 ignored 1개를 구분합니다. Rust harness 450회 중 child의 부모/자식 중복 9회를 제외하며 원시 출력을 유지합니다. Node·Chromium 각각 85개 개발 시나리오와 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 실패 2개를 기록합니다.

기존 2,688개 정의·150개 순차 목록·84개 플랫폼 시나리오·native 검사 파일을 보존했고 개발 예외는 추가하지 않았습니다. 요구사항 474개·부분 연결 16개와 accounting row 3,724개 중 ready 2,724개·미구현 960개·미연결 40개는 완료율로 계산하지 않습니다. 엄격한 전체 gate는 실패 이유 1,950개를 유지합니다. 새 archive 10개·58,909,100 bytes는 최종 suite 8개와 지원 자료·소스 증거를 보존하고, 소스 사본 6,911개·고유 프로젝트 파일 3,287개를 구현 커밋에 연결합니다. 명시한 Cargo build cache 5개는 내부를 열거하지 않고 제외하며 payload 수·크기·해시를 주장하지 않습니다. 기존 Stage 25 archive 15개는 직접 크기·해시 의존성으로 유지하고 재귀적인 역사 감사나 재실행을 주장하지 않습니다.

[독립 최종 감사](reports/m2/stage26-post-seal-audit.json)는 원본 파일 67,236개·archive·Git 소스·리뷰·gate·원시 결과·테스트 집계를 확인했습니다. 첫 봉인 도구의 메타데이터 필드 비교 오류와 첫 감사의 순차 native 출력 형식 가정 오류는 실패 기록을 보존하고 수정했습니다. 보충 감사는 원본 부모/자식 출력 블록을 검증하며 이미 봉인한 10개 archive를 변경하지 않았습니다. [완료 기록](reports/m2/stage26-completion-captures.json)은 실패·성공 감사 process와 보충 도구·준비 diff·리뷰의 원본 26개를 별도로 보존합니다. **Stage 26의 제한된 문자열 프런트엔드 개발 계약은 마감했으며 M2 전체·제품 API·릴리스 플랫폼·속도·peak RSS 목표는 계속 미완료입니다.**


스물일곱 번째 단계는 Stage 26에서 보존한 문자열 구문 진단 차이 15개를 해결했습니다. 구현 `5383d84c894bc19c90c5b5a3cf9bf54aeb3381d0`를 커밋·푸시했고, 핵심 lexer·parser는 `gpt-6-astra / xhigh`, Java 증거와 native·플랫폼 검증은 각각 `gpt-6.1-sol / high`에 배정했습니다. 문자열 token의 실제 span과 빈 text token, raw hash escape, EOF·continuation·다중 행 오류 메시지와 UTF-16 underline 폭을 보존합니다. 앞선 interpolation 오류의 우선순위와 cache의 소비 범위·원래 source 끝 조건도 유지합니다.

[최종 요약](reports/m2/stage27-final-summary.json)은 독립 식 2,831개 중 원시 바이트 일치 2,829개와 기존 순환 oracle 오류·정수 `% 0` 진단 차이를 기록합니다. 새 107개(성공 5개·예상 오류 102개)와 변형 관계 88개는 모두 일치했습니다. 기존 2,724개 정의·입력 바이트·151개 순차 목록·85개 플랫폼 시나리오·native 검사·참조 pin을 보존했습니다. 순차 계약 2,622개 호출도 모두 일치하며 새 개발 예외는 없습니다. 요구사항 474개·부분 연결 16개와 accounting row 3,831개(ready 2,831개·미구현 960개·미연결 40개)는 완료율로 계산하지 않습니다. 엄격한 전체 gate는 실패 이유 1,950개를 유지합니다.

[필수 로컬 검사 7종](reports/m2/validation-stage27-final.json)은 Rust 고유 443개·Python 194개 성공과 기존 ignored 1개를 구분합니다. 원시 Rust harness 454회에서 부모·자식 중복 11회를 제외하며 출력 바이트는 바꾸지 않습니다. 새 native 검사 2개는 기존 256 KiB stack·검사당 60초 조건을 유지합니다. 실제 Node·Chromium 각각 86개 시나리오와 공통 primitive 6,078개 일치·target별 Java 일치 6,076개·기존 regex 차이 2개를 확인했습니다. 새 플랫폼 시나리오는 target별 60회 호출이며 구문 오류 뒤에는 새 evaluator로 정상 입력을 실행합니다. 동일 evaluator의 Host·GC·재진입·취소 검증으로 확대하지 않습니다.

원래 URI에서 새로 수집한 Java 결과는 과거 진단 15개의 바이트를 그대로 재현합니다. 고정 release와 최종 언어 실행 파일의 탐색 158개는 각각 일치 144개·불일치 12개·Java process 오류 2개입니다. 남은 차이는 깊이 guard 7개·기존 Unicode 내부 오류 1개·새 Unicode 경계 4개이며 선택 사례와 중복됩니다. 원래 탐색 62개 중 나머지 47개는 보존한 Java 결과를 재사용했습니다. 깊이 한도 32·작업 예산 65,536과 보조 lexical 검증의 동기 재파싱은 유지하며 임의 깊이·균일한 지연·자원 오류 호환성을 주장하지 않습니다.

[독립 감사](reports/m2/stage27-post-seal-audit.json)는 새 archive 10개·38,213,047 bytes, 구현 커밋의 소스 사본 7,131개·고유 파일 3,396개와 원본 취득 파일 60,233개를 확인했습니다. 명시한 Cargo build cache 3개는 내부를 열거하지 않으며 Stage 26의 archive·요약·감사·완료 기록은 직접 불변 의존성으로 유지합니다. [완료 기록](reports/m2/stage27-completion-captures.json)은 봉인·감사의 원본 process 파일 12개를 보존합니다. 초기 실패·중간 비교와 의도적인 whitespace·CRLF 입력도 유지합니다. **Stage 27의 제한된 lexical 진단 개발 계약은 마감했으며 M2 전체·제품 API·릴리스 플랫폼·속도·peak RSS 목표는 미완료입니다.**

다음 구현은 남은 탐색 불일치의 의존성을 기준으로 진행합니다.

1. 남은 Reference metadata 작업·진단 확장을 continuation과 자원 계약으로 검증하고 Java stack-overflow 진단 및 취소 지연을 별도로 다룹니다. 전체 지연 타입 해석·alias-root 대입 identity와 wrapper 중복 제거의 실제 인자 구분을 검증합니다. 남은 nullable/default·inline Reference·local typed-property·outer 진단, generic constructor의 지연 멤버 frame 이름·builtin·generator 경계를 원래 실패 입력으로 보완합니다. Stage 24에서 보존한 String 선택 별칭 union 생성자의 `Cannot instantiate type` 진단·지연 시점·오류 우선순위도 여기에 포함합니다. wrapper identity를 바꾸기 전에는 Java에서 관찰 가능한 캐시·성공 predicate 호출 횟수 근거를 먼저 확보합니다. 이미 검증한 공유 타입 인자의 깊은 실행·동시 해제, inline·lambda·공유 let·분기 순서·재귀 깊이별 재시도, 타입 인자 캡처·기본값·Reference 검사 순서, Host/GC/취소/terminal 동작을 회귀 기준으로 유지합니다.
2. Stage 22·23에서 추가한 strict `Map`·`Set`의 생성·조회·기본값·타입 검사·동등성·해시·순회 회귀와 공통 VM·GC·Host 수명주기를 유지합니다. Stage 24의 `Collection<T>` 분류·eager/lazy 원소 검사·기본값·별칭·metadata와 `set-stage23-type-any`의 해결을 회귀 기준에 추가했습니다. 전체 collection API 완료로 확대하지 않습니다. Stage 25에서 `set-stage23-deep-64-equality`·`set-stage23-deep-64-map-key`의 작성된 구문 경로를 개선했고, 기존 768단계 runtime 검사와 별도로 authored source를 검증합니다. Stage 26은 중첩 lexical string의 원래 작은 스택 오류를 제한된 범위에서 개선했습니다. Stage 27은 비교한 문자열 진단 15개를 해결했습니다. 다음 문자열 작업은 빈·비16진수·여는 brace 누락 Unicode escape의 진단 3종과 지연 검증 순서를 우선 다루며, Java 범위 초과 내부 오류·미검증 surrogate 동작은 별도로 유지합니다. 후속 프런트엔드 작업은 남은 깊이 한도와 Unicode 진단 차이, 깊은 타입·클래스·혼합 body 컴파일과 Java 자원 오류의 진단입니다. JVM identity hash, 변경된 zero-hash 키의 순회와 중첩 Map 렌더링 중 재해시 효과도 남겨 둡니다. `Map.keys`·`entries`, Set·List/Listing의 고차 함수·변환·`map`·`join`, Pair·Duration/DataSize와 일반 compound 출력은 M3 의존성으로 추적합니다.
3. annotation과 남은 클래스 초기화·modifier·재귀 타입·reflection 규칙을 확장합니다. 실제 source의 초기화 중 Host 중단·재진입, 미게시 local 선언의 Java 내부 오류와 순환 계층의 비종료 관찰은 원래 실패·미실행 상태를 보존하며 별도로 검증합니다.
4. 모듈 클래스 상속·선언 출처·ModuleOutput override를 M2 의미론에 연결하고 실제 모듈 로딩·URI/source cache identity·Host는 M4 경로와 통합합니다. primitive missing-method 진단도 실제 멤버 목록을 사용하도록 보완합니다.

다음 기능도 한 evaluator의 상태·오류 순서·GC root·Host 재진입·terminal 해제를 함께 검증해야 합니다. 현재 선택된 2,831개 독립 식과 88개 변형 관계, 별도 클래스·별칭 순차 계약 2,622개 호출을 회귀 기준으로 유지하고 Java 원시 결과를 Rust 출력으로 대체하지 않습니다. 순차 계약의 성공은 전체 클래스·module API나 M2 완료, 메모리 목표 달성을 뜻하지 않습니다.

[원본 API 사례](external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/reference.pkl)는 `fixed` 이후에도 Map·`map`/`join`·`pkl:math`·renderer converter를 요구하며, [원본 basic 사례](external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/reference.pkl)는 module amendment·`pkl:test`·PCF 출력에 의존합니다. 이 M3·M4 연결을 별도 작업으로 추적하며 원래 snippet/API 실행 조건을 유지합니다. Stage 13의 Reference 탐색 재비교에서는 이전 불일치 4개가 해결되어 191개 중 173개 일치·18개 불일치가 남았습니다. 남은 불일치를 삭제하거나 개발 예외로 바꾸지 않고 단계별로 재검증합니다. 이는 Stage 12에서 수정한 순차 클래스 진단 불일치 22개와 별도 집계입니다.

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

`cargo xtask verify`는 참조 소스 고정을 확인합니다. 위 명령은 Java 차등 검증이나 제품 성능 측정을 수행하지 않습니다. 현재 Java 기록·재비교·초기 성능 측정 명령과 실행 범위는 [TESTING.md](TESTING.md)에 정리되어 있습니다. Rust 평가기 구현 후 실제 Java/Rust 비교를 연결합니다. 단순 문서 수정에는 링크·내용·형식 확인 등 변경에 맞는 검증을 수행합니다.

단계별 로드맵과 상세 실행 계획은 이 문서에서 함께 관리합니다. 증거가 생길 때 단계 상태와 다음 작업을 갱신하고, 단계의 범위나 완료 기준이 바뀌면 요약 표와 해당 상세 절을 함께 수정합니다. 설계 결정은 아키텍처에, 검증 계약 변경은 TESTING.md에 반영합니다. 기준 버전·필수 기능·성능 목표를 바꾸는 경우에는 INTENT.md까지 함께 갱신하고 목표 변경 이유를 명시합니다. 실패를 없애기 위한 범위 축소는 일반 구현 진척으로 기록하지 않습니다.
